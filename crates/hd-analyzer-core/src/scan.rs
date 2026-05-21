use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::driver::{
    DirectoryListing, DirectorySizeSummary, EntryKind, NodeState, PathNode, PreciseDirectoryScan,
    PreciseScanProfile, ReadIssueKind, ScanIssue,
};
use crate::jobs::CancelToken;
use crate::rules::ScanConfig;
use crate::safety::classify_path_safety;
use crate::size::{HardLinkDedupe, directory_measurement, measure_file_without_dedupe};

fn get_device_id(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).ok().map(|m| m.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

pub fn discover_directory(path: &Path, config: &ScanConfig) -> std::io::Result<DirectoryListing> {
    let path = path.canonicalize()?;
    let mut dedupe = HardLinkDedupe::new();
    discover_directory_inner(&path, &path, config, 0, &mut dedupe)
}

pub fn discover_directory_with_precise_sizes(
    path: &Path,
    config: &ScanConfig,
) -> std::io::Result<PreciseDirectoryScan> {
    discover_directory_with_precise_sizes_with_cancel(path, config, None)
}

pub fn discover_directory_with_precise_sizes_with_cancel(
    path: &Path,
    config: &ScanConfig,
    cancel_token: Option<&CancelToken>,
) -> std::io::Result<PreciseDirectoryScan> {
    discover_directory_with_precise_sizes_with_cancel_and_progress(path, config, cancel_token, None)
}

pub fn discover_directory_with_precise_sizes_with_cancel_and_progress(
    path: &Path,
    config: &ScanConfig,
    cancel_token: Option<&CancelToken>,
    progress: Option<&PreciseScanProgressCallback<'_>>,
) -> std::io::Result<PreciseDirectoryScan> {
    let path = path.canonicalize()?;
    let mut dedupe = HardLinkDedupe::new();
    let root_device = if config.stay_on_filesystem {
        get_device_id(&path)
    } else {
        None
    };
    discover_directory_precise_inner_with_progress(
        &path,
        config,
        &mut dedupe,
        root_device,
        cancel_token,
        progress,
    )
}

fn discover_directory_inner(
    root: &Path,
    path: &Path,
    config: &ScanConfig,
    depth: usize,
    dedupe: &mut HardLinkDedupe,
) -> std::io::Result<DirectoryListing> {
    let mut children = Vec::new();
    let mut issues = Vec::new();
    let mut total_visible_size = 0u64;
    let mut total_measured_size = 0u64;
    let mut total_logical_size = 0u64;
    let mut has_more_depth = false;

    let root_device = if config.stay_on_filesystem {
        get_device_id(root)
    } else {
        None
    };

    let entries = jwalk::WalkDir::new(path)
        .min_depth(1)
        .max_depth(1)
        .skip_hidden(false)
        .follow_links(config.follow_symlinks);

    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                issues.push(ScanIssue {
                    path: error
                        .path()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| path.to_path_buf()),
                    kind: if error.io_error().is_some_and(|io_error| {
                        io_error.kind() == std::io::ErrorKind::PermissionDenied
                    }) {
                        ReadIssueKind::PermissionDenied
                    } else {
                        ReadIssueKind::MetadataFailed
                    },
                    message: error.to_string(),
                });
                continue;
            }
        };

        let entry_path = entry.path();
        let name = entry.file_name.to_string_lossy().into_owned();
        let is_visible_by_config = !config.is_hidden_name(&name);

        let file_type = entry.file_type;

        if entry.path_is_symlink() && !config.follow_symlinks {
            match std::fs::symlink_metadata(&entry_path) {
                Ok(metadata) => {
                    let measurement = dedupe.measure_file_with_policy(
                        &entry_path,
                        &metadata,
                        config.dedupe_hard_links,
                        config.size_measurement_mode,
                    );
                    total_measured_size =
                        total_measured_size.saturating_add(measurement.allocated_bytes);
                    total_logical_size =
                        total_logical_size.saturating_add(measurement.logical_bytes);
                    if is_visible_by_config {
                        total_visible_size =
                            total_visible_size.saturating_add(measurement.allocated_bytes);
                        children.push(PathNode {
                            path: entry_path.clone(),
                            name,
                            kind: EntryKind::File,
                            parent_path: Some(path.to_path_buf()),
                            depth_from_request: depth + 1,
                            size: measurement.allocated_bytes,
                            logical_size: measurement.logical_bytes,
                            state: NodeState::Complete,
                            visible: true,
                            children_known: true,
                            active_job_id: None,
                            delete_safety: classify_path_safety(&entry_path),
                            issues: Vec::new(),
                        });
                    }
                }
                Err(error) => issues.push(ScanIssue {
                    path: entry_path,
                    kind: ReadIssueKind::MetadataFailed,
                    message: error.to_string(),
                }),
            }
            continue;
        }

        if file_type.is_dir() {
            if let Some(root_device) = root_device {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.dev() != root_device {
                            if is_visible_by_config {
                                issues.push(ScanIssue {
                                    path: entry_path,
                                    kind: ReadIssueKind::FilesystemBoundary,
                                    message: "Directory is on a different filesystem.".to_string(),
                                });
                            }
                            continue;
                        }
                    }
                }
            }

            let (size, logical_size, children_known, node_issues, node_state) =
                measure_directory_node(root, &entry_path, config, depth + 1, dedupe);
            issues.extend(node_issues.clone());
            total_measured_size = total_measured_size.saturating_add(size);
            total_logical_size = total_logical_size.saturating_add(logical_size);

            if is_visible_by_config && config.should_show_folder(size) {
                total_visible_size = total_visible_size.saturating_add(size);
                has_more_depth |= !children_known;
                children.push(PathNode {
                    path: entry_path.clone(),
                    name,
                    kind: EntryKind::Directory,
                    parent_path: Some(path.to_path_buf()),
                    depth_from_request: depth + 1,
                    size,
                    logical_size,
                    state: node_state,
                    visible: true,
                    children_known,
                    active_job_id: None,
                    delete_safety: classify_path_safety(&entry_path),
                    issues: node_issues,
                });
            }
        } else if file_type.is_file() {
            match entry.metadata() {
                Ok(metadata) => {
                    let measurement = dedupe.measure_file_with_policy(
                        &entry_path,
                        &metadata,
                        config.dedupe_hard_links,
                        config.size_measurement_mode,
                    );
                    total_measured_size =
                        total_measured_size.saturating_add(measurement.allocated_bytes);
                    total_logical_size =
                        total_logical_size.saturating_add(measurement.logical_bytes);
                    if is_visible_by_config {
                        total_visible_size =
                            total_visible_size.saturating_add(measurement.allocated_bytes);
                        children.push(PathNode {
                            path: entry_path.clone(),
                            name,
                            kind: EntryKind::File,
                            parent_path: Some(path.to_path_buf()),
                            depth_from_request: depth + 1,
                            size: measurement.allocated_bytes,
                            logical_size: measurement.logical_bytes,
                            state: NodeState::Complete,
                            visible: true,
                            children_known: true,
                            active_job_id: None,
                            delete_safety: classify_path_safety(&entry_path),
                            issues: Vec::new(),
                        });
                    }
                }
                Err(error) => issues.push(ScanIssue {
                    path: entry_path,
                    kind: ReadIssueKind::MetadataFailed,
                    message: error.to_string(),
                }),
            }
        }
    }

    children.sort_by(|left, right| {
        right
            .size
            .cmp(&left.size)
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(DirectoryListing {
        path: path.to_path_buf(),
        config_fingerprint: config.fingerprint(),
        children,
        total_visible_size,
        total_measured_size,
        total_logical_size,
        state: NodeState::Complete,
        loaded_depth: config.preload_depth.min(config.requested_depth),
        has_more_depth,
        issues,
        generation: 0,
    })
}

fn measure_directory_node(
    root: &Path,
    path: &Path,
    config: &ScanConfig,
    depth: usize,
    dedupe: &mut HardLinkDedupe,
) -> (u64, u64, bool, Vec<ScanIssue>, NodeState) {
    if depth > config.preload_depth {
        return (0, 0, false, Vec::new(), NodeState::Queued);
    }

    match discover_directory_inner(root, path, config, depth, dedupe) {
        Ok(listing) => {
            let measurement =
                directory_measurement(listing.total_measured_size, listing.total_logical_size);
            let should_expand = config.should_expand_folder(measurement.allocated_bytes, depth);
            let has_deferred_descendants = listing.has_more_depth;
            let children_known =
                !has_deferred_descendants && (depth >= config.requested_depth || !should_expand);
            let state = if has_deferred_descendants {
                NodeState::Partial
            } else {
                NodeState::Complete
            };
            (
                measurement.allocated_bytes,
                measurement.logical_bytes,
                children_known,
                listing.issues,
                state,
            )
        }
        Err(error) => (
            0,
            0,
            true,
            vec![ScanIssue {
                path: path.to_path_buf(),
                kind: if error.kind() == std::io::ErrorKind::PermissionDenied {
                    ReadIssueKind::PermissionDenied
                } else {
                    ReadIssueKind::MetadataFailed
                },
                message: error.to_string(),
            }],
            NodeState::Failed,
        ),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PreciseScanProgress {
    pub entries_visited: u64,
    pub files_visited: u64,
    pub directories_visited: u64,
    pub bytes_measured: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreciseDirectoryProgressUpdate {
    pub path: PathBuf,
    pub allocated_bytes: u64,
    pub logical_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct PreciseScanUpdate {
    pub progress: PreciseScanProgress,
    pub directory_updates: Vec<PreciseDirectoryProgressUpdate>,
}

pub type PreciseScanProgressCallback<'a> = dyn Fn(PreciseScanUpdate) + Send + Sync + 'a;

#[derive(Debug, Default)]
struct ScanProfileRecorder {
    entries_collected: AtomicU64,
    read_dir_nanos: AtomicU64,
    file_type_nanos: AtomicU64,
    metadata_nanos: AtomicU64,
    size_nanos: AtomicU64,
    live_update_nanos: AtomicU64,
    progress_emit_nanos: AtomicU64,
}

impl ScanProfileRecorder {
    fn add_duration(counter: &AtomicU64, duration: Duration) {
        let nanos = duration.as_nanos().min(u64::MAX as u128) as u64;
        counter.fetch_add(nanos, Ordering::Relaxed);
    }

    fn add_entries(&self, count: u64) {
        self.entries_collected.fetch_add(count, Ordering::Relaxed);
    }

    fn add_read_dir(&self, duration: Duration) {
        Self::add_duration(&self.read_dir_nanos, duration);
    }

    fn add_file_type(&self, duration: Duration) {
        Self::add_duration(&self.file_type_nanos, duration);
    }

    fn add_metadata(&self, duration: Duration) {
        Self::add_duration(&self.metadata_nanos, duration);
    }

    fn add_size(&self, duration: Duration) {
        Self::add_duration(&self.size_nanos, duration);
    }

    fn add_live_update(&self, duration: Duration) {
        Self::add_duration(&self.live_update_nanos, duration);
    }

    fn add_progress_emit(&self, duration: Duration) {
        Self::add_duration(&self.progress_emit_nanos, duration);
    }

    fn snapshot(&self) -> PreciseScanProfile {
        PreciseScanProfile {
            entries_collected: self.entries_collected.load(Ordering::Relaxed),
            read_dir_nanos: self.read_dir_nanos.load(Ordering::Relaxed),
            file_type_nanos: self.file_type_nanos.load(Ordering::Relaxed),
            metadata_nanos: self.metadata_nanos.load(Ordering::Relaxed),
            size_nanos: self.size_nanos.load(Ordering::Relaxed),
            live_update_nanos: self.live_update_nanos.load(Ordering::Relaxed),
            progress_emit_nanos: self.progress_emit_nanos.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
struct DirectChild {
    path: PathBuf,
    name: String,
    kind: EntryKind,
    allocated_bytes: u64,
    logical_bytes: u64,
    has_visible_children: bool,
    issues: Vec<ScanIssue>,
}

#[derive(Debug)]
struct EntryCandidate {
    path: PathBuf,
    name: String,
    visible: bool,
    is_dir: bool,
    is_file: bool,
    is_symlink: bool,
}

#[derive(Debug, Default)]
struct DirectoryAccumulator {
    allocated_bytes: u64,
    logical_bytes: u64,
    has_visible_children: bool,
    issues: Vec<ScanIssue>,
}

#[derive(Debug, Default)]
struct MeasuredDirectory {
    accumulator: DirectoryAccumulator,
    direct_children: Vec<DirectChild>,
    summaries: Vec<DirectorySizeSummary>,
}

fn discover_directory_precise_inner_with_progress(
    path: &Path,
    config: &ScanConfig,
    _dedupe: &mut HardLinkDedupe,
    root_device: Option<u64>,
    cancel_token: Option<&CancelToken>,
    progress: Option<&PreciseScanProgressCallback<'_>>,
) -> std::io::Result<PreciseDirectoryScan> {
    let config_fingerprint = config.fingerprint();
    let shared = ScanSharedState::new(
        config,
        root_device,
        cancel_token,
        progress,
        &config_fingerprint,
    );
    let MeasuredDirectory {
        accumulator: root_accumulator,
        direct_children,
        summaries,
    } = measure_directory_contents(path, &shared, None, true)?;
    if progress.is_some() {
        shared.flush_live_updates();
        shared.emit(PreciseScanUpdate {
            progress: shared.stats_snapshot(),
            directory_updates: Vec::new(),
        });
    }
    let mut children = Vec::new();
    let mut total_visible_size = 0u64;
    let mut has_more_depth = false;

    for child in direct_children {
        if child.kind == EntryKind::Directory && !config.should_show_folder(child.allocated_bytes) {
            continue;
        }

        total_visible_size = total_visible_size.saturating_add(child.allocated_bytes);
        has_more_depth |= child.kind == EntryKind::Directory && child.has_visible_children;
        children.push(PathNode {
            path: child.path.clone(),
            name: child.name,
            kind: child.kind,
            parent_path: Some(path.to_path_buf()),
            depth_from_request: 1,
            size: child.allocated_bytes,
            logical_size: child.logical_bytes,
            state: NodeState::Complete,
            visible: true,
            children_known: child.kind != EntryKind::Directory || !child.has_visible_children,
            active_job_id: None,
            delete_safety: classify_path_safety(&child.path),
            issues: child.issues,
        });
    }

    children.sort_by(|left, right| {
        right
            .size
            .cmp(&left.size)
            .then_with(|| left.name.cmp(&right.name))
    });

    let listing = DirectoryListing {
        path: path.to_path_buf(),
        config_fingerprint: config_fingerprint.clone(),
        children,
        total_visible_size,
        total_measured_size: root_accumulator.allocated_bytes,
        total_logical_size: root_accumulator.logical_bytes,
        state: NodeState::Complete,
        loaded_depth: 1.min(config.requested_depth),
        has_more_depth,
        issues: root_accumulator.issues,
        generation: 0,
    };

    Ok(PreciseDirectoryScan {
        listing,
        summaries,
        profile: shared.profile_snapshot(),
    })
}

fn is_canceled(cancel_token: Option<&CancelToken>) -> bool {
    cancel_token.is_some_and(CancelToken::is_canceled)
}

fn canceled_error() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Interrupted, "filesystem scan canceled")
}

#[derive(Debug, Default)]
struct LiveDirectoryTotals {
    allocated_bytes: u64,
    logical_bytes: u64,
}

#[derive(Debug)]
struct LiveUpdateCollector {
    enabled: bool,
    throttle: Duration,
    min_bytes_delta: u64,
    max_batch_size: usize,
    current: HashMap<PathBuf, LiveDirectoryTotals>,
    last_emitted: HashMap<PathBuf, LiveDirectoryTotals>,
    pending: HashMap<PathBuf, PreciseDirectoryProgressUpdate>,
    last_flush: Instant,
}

impl LiveUpdateCollector {
    fn new(config: &ScanConfig) -> Self {
        Self {
            enabled: config.live_updates.enabled,
            throttle: Duration::from_millis(config.live_updates.throttle_ms),
            min_bytes_delta: config.live_updates.min_bytes_delta,
            max_batch_size: config.live_updates.max_batch_size.max(1),
            current: HashMap::new(),
            last_emitted: HashMap::new(),
            pending: HashMap::new(),
            last_flush: Instant::now(),
        }
    }

    fn add_bytes(
        &mut self,
        path: Option<&Path>,
        allocated_bytes: u64,
        logical_bytes: u64,
        stats: PreciseScanProgress,
    ) -> Option<PreciseScanUpdate> {
        if !self.enabled {
            return None;
        }
        let Some(path) = path else {
            return None;
        };

        let path = path.to_path_buf();
        let totals = self.current.entry(path.clone()).or_default();
        totals.allocated_bytes = totals.allocated_bytes.saturating_add(allocated_bytes);
        totals.logical_bytes = totals.logical_bytes.saturating_add(logical_bytes);
        let queued_totals = LiveDirectoryTotals {
            allocated_bytes: totals.allocated_bytes,
            logical_bytes: totals.logical_bytes,
        };

        if self.should_queue(&path, &queued_totals) {
            self.pending.insert(
                path.clone(),
                PreciseDirectoryProgressUpdate {
                    path,
                    allocated_bytes: queued_totals.allocated_bytes,
                    logical_bytes: queued_totals.logical_bytes,
                },
            );
        }

        if self.pending.len() >= self.max_batch_size || self.last_flush.elapsed() >= self.throttle {
            return self.flush(stats);
        }
        None
    }

    fn flush_all(&mut self, stats: PreciseScanProgress) -> Option<PreciseScanUpdate> {
        if !self.enabled {
            return None;
        }

        for (path, totals) in &self.current {
            let last = self.last_emitted.get(path);
            if last.is_none_or(|last| {
                last.allocated_bytes != totals.allocated_bytes
                    || last.logical_bytes != totals.logical_bytes
            }) {
                self.pending.insert(
                    path.clone(),
                    PreciseDirectoryProgressUpdate {
                        path: path.clone(),
                        allocated_bytes: totals.allocated_bytes,
                        logical_bytes: totals.logical_bytes,
                    },
                );
            }
        }
        self.flush(stats)
    }

    fn should_queue(&self, path: &Path, totals: &LiveDirectoryTotals) -> bool {
        if self.min_bytes_delta == 0 {
            return true;
        }

        let Some(last) = self.last_emitted.get(path) else {
            return totals.allocated_bytes >= self.min_bytes_delta
                || totals.logical_bytes >= self.min_bytes_delta;
        };

        totals
            .allocated_bytes
            .saturating_sub(last.allocated_bytes)
            .max(totals.logical_bytes.saturating_sub(last.logical_bytes))
            >= self.min_bytes_delta
    }

    fn flush(&mut self, stats: PreciseScanProgress) -> Option<PreciseScanUpdate> {
        if self.pending.is_empty() {
            return None;
        }

        let updates = self
            .pending
            .drain()
            .map(|(_, update)| update)
            .collect::<Vec<_>>();
        for update in &updates {
            self.last_emitted.insert(
                update.path.clone(),
                LiveDirectoryTotals {
                    allocated_bytes: update.allocated_bytes,
                    logical_bytes: update.logical_bytes,
                },
            );
        }
        self.last_flush = Instant::now();
        Some(PreciseScanUpdate {
            progress: stats,
            directory_updates: updates,
        })
    }
}

struct ScanSharedState<'a> {
    config: &'a ScanConfig,
    root_device: Option<u64>,
    cancel_token: Option<&'a CancelToken>,
    progress: Option<&'a PreciseScanProgressCallback<'a>>,
    config_fingerprint: &'a str,
    dedupe: Mutex<HardLinkDedupe>,
    stats: Mutex<PreciseScanProgress>,
    last_progress: Mutex<Instant>,
    live_updates: Mutex<LiveUpdateCollector>,
    emit_lock: Mutex<()>,
    profile: ScanProfileRecorder,
}

impl<'a> ScanSharedState<'a> {
    fn new(
        config: &'a ScanConfig,
        root_device: Option<u64>,
        cancel_token: Option<&'a CancelToken>,
        progress: Option<&'a PreciseScanProgressCallback<'a>>,
        config_fingerprint: &'a str,
    ) -> Self {
        Self {
            config,
            root_device,
            cancel_token,
            progress,
            config_fingerprint,
            dedupe: Mutex::new(HardLinkDedupe::new()),
            stats: Mutex::new(PreciseScanProgress::default()),
            last_progress: Mutex::new(Instant::now()),
            live_updates: Mutex::new(LiveUpdateCollector::new(config)),
            emit_lock: Mutex::new(()),
            profile: ScanProfileRecorder::default(),
        }
    }

    fn stats_snapshot(&self) -> PreciseScanProgress {
        *self.stats.lock().expect("scan stats poisoned")
    }

    fn add_entries(&self, count: u64) -> PreciseScanProgress {
        let mut stats = self.stats.lock().expect("scan stats poisoned");
        stats.entries_visited = stats.entries_visited.saturating_add(count);
        *stats
    }

    fn add_directory(&self) -> PreciseScanProgress {
        let mut stats = self.stats.lock().expect("scan stats poisoned");
        stats.directories_visited = stats.directories_visited.saturating_add(1);
        *stats
    }

    fn add_file(&self, allocated_bytes: u64) -> PreciseScanProgress {
        let mut stats = self.stats.lock().expect("scan stats poisoned");
        stats.files_visited = stats.files_visited.saturating_add(1);
        stats.bytes_measured = stats.bytes_measured.saturating_add(allocated_bytes);
        *stats
    }

    fn measure_file(
        &self,
        path: &Path,
        metadata: &std::fs::Metadata,
    ) -> crate::size::SizeMeasurement {
        let started = Instant::now();
        let mut measurement =
            measure_file_without_dedupe(path, metadata, self.config.size_measurement_mode);
        if self.config.dedupe_hard_links
            && measurement.identity.as_ref().is_some_and(|identity| {
                self.dedupe
                    .lock()
                    .expect("hard-link dedupe state poisoned")
                    .has_seen(identity)
            })
        {
            measurement.allocated_bytes = 0;
            measurement.deduped = true;
        }
        self.profile.add_size(started.elapsed());
        measurement
    }

    fn add_live_bytes(
        &self,
        path: Option<&Path>,
        allocated_bytes: u64,
        logical_bytes: u64,
        stats: PreciseScanProgress,
    ) {
        let started = Instant::now();
        let update = self
            .live_updates
            .lock()
            .expect("live update state poisoned")
            .add_bytes(path, allocated_bytes, logical_bytes, stats);
        self.profile.add_live_update(started.elapsed());
        if let Some(update) = update {
            self.emit(update);
        }
    }

    fn flush_live_updates(&self) {
        let stats = self.stats_snapshot();
        let update = self
            .live_updates
            .lock()
            .expect("live update state poisoned")
            .flush_all(stats);
        if let Some(update) = update {
            self.emit(update);
        }
    }

    fn publish_progress_if_due(&self) {
        let Some(_progress) = self.progress else {
            return;
        };
        let mut last_progress = self.last_progress.lock().expect("progress state poisoned");
        if last_progress.elapsed() < Duration::from_millis(250) {
            return;
        }
        *last_progress = Instant::now();
        drop(last_progress);
        self.emit(PreciseScanUpdate {
            progress: self.stats_snapshot(),
            directory_updates: Vec::new(),
        });
    }

    fn emit(&self, update: PreciseScanUpdate) {
        let Some(progress) = self.progress else {
            return;
        };
        let started = Instant::now();
        let _guard = self.emit_lock.lock().expect("progress emit state poisoned");
        progress(update);
        self.profile.add_progress_emit(started.elapsed());
    }

    fn profile_snapshot(&self) -> PreciseScanProfile {
        self.profile.snapshot()
    }
}

fn measure_directory_contents(
    path: &Path,
    shared: &ScanSharedState<'_>,
    live_child_path: Option<&Path>,
    collect_direct_children: bool,
) -> std::io::Result<MeasuredDirectory> {
    if is_canceled(shared.cancel_token) {
        return Err(canceled_error());
    }

    let mut measured = MeasuredDirectory::default();
    let (candidates, issues) = collect_entry_candidates(path, shared)?;
    measured.accumulator.issues.extend(issues);

    let entries = candidates
        .into_par_iter()
        .map(|candidate| measure_entry(candidate, shared, live_child_path, collect_direct_children))
        .collect::<Vec<_>>();

    for entry in entries {
        let entry = entry?;
        measured.accumulator.allocated_bytes = measured
            .accumulator
            .allocated_bytes
            .saturating_add(entry.allocated_bytes);
        measured.accumulator.logical_bytes = measured
            .accumulator
            .logical_bytes
            .saturating_add(entry.logical_bytes);
        measured.accumulator.has_visible_children |= entry.visible;
        measured.accumulator.issues.extend(entry.issues);
        measured.summaries.extend(entry.summaries);
        if let Some(direct_child) = entry.direct_child {
            measured.direct_children.push(direct_child);
        }
    }

    Ok(measured)
}

fn collect_entry_candidates(
    path: &Path,
    shared: &ScanSharedState<'_>,
) -> std::io::Result<(Vec<EntryCandidate>, Vec<ScanIssue>)> {
    let read_started = Instant::now();
    let mut entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            if is_permission_denied(&error) || path.exists() {
                shared.profile.add_read_dir(read_started.elapsed());
                return Ok((
                    Vec::new(),
                    vec![filesystem_issue(path.to_path_buf(), error)],
                ));
            }
            shared.profile.add_read_dir(read_started.elapsed());
            return Err(error);
        }
    };
    shared.profile.add_read_dir(read_started.elapsed());

    let mut candidates = Vec::new();
    let mut issues = Vec::new();
    loop {
        if is_canceled(shared.cancel_token) {
            return Err(canceled_error());
        }

        let read_next_started = Instant::now();
        let entry_result = entries.next();
        shared.profile.add_read_dir(read_next_started.elapsed());
        let Some(entry_result) = entry_result else {
            break;
        };

        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                issues.push(filesystem_issue(path.to_path_buf(), error));
                continue;
            }
        };

        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_visible_by_config = !shared.config.is_hidden_name(&name);
        let file_type_started = Instant::now();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                shared.profile.add_file_type(file_type_started.elapsed());
                issues.push(filesystem_issue(entry_path, error));
                continue;
            }
        };
        shared.profile.add_file_type(file_type_started.elapsed());
        candidates.push(EntryCandidate {
            path: entry_path,
            name,
            visible: is_visible_by_config,
            is_dir: file_type.is_dir(),
            is_file: file_type.is_file(),
            is_symlink: file_type.is_symlink(),
        });
    }

    shared.profile.add_entries(candidates.len() as u64);
    shared.add_entries(candidates.len() as u64);
    shared.publish_progress_if_due();
    Ok((candidates, issues))
}

#[derive(Debug, Default)]
struct MeasuredEntry {
    allocated_bytes: u64,
    logical_bytes: u64,
    visible: bool,
    issues: Vec<ScanIssue>,
    summaries: Vec<DirectorySizeSummary>,
    direct_child: Option<DirectChild>,
}

fn measure_entry(
    candidate: EntryCandidate,
    shared: &ScanSharedState<'_>,
    live_child_path: Option<&Path>,
    collect_direct_children: bool,
) -> std::io::Result<MeasuredEntry> {
    if is_canceled(shared.cancel_token) {
        return Err(canceled_error());
    }

    if candidate.is_symlink && !shared.config.follow_symlinks {
        return Ok(measure_file_entry(
            candidate,
            shared,
            live_child_path,
            collect_direct_children,
            true,
        ));
    }

    if candidate.is_dir {
        return measure_directory_entry(
            candidate,
            shared,
            live_child_path,
            collect_direct_children,
        );
    }

    if candidate.is_file {
        return Ok(measure_file_entry(
            candidate,
            shared,
            live_child_path,
            collect_direct_children,
            false,
        ));
    }

    Ok(MeasuredEntry::default())
}

fn measure_file_entry(
    candidate: EntryCandidate,
    shared: &ScanSharedState<'_>,
    live_child_path: Option<&Path>,
    collect_direct_children: bool,
    symlink_metadata: bool,
) -> MeasuredEntry {
    let metadata_started = Instant::now();
    let metadata = if symlink_metadata {
        std::fs::symlink_metadata(&candidate.path)
    } else {
        std::fs::metadata(&candidate.path)
    };
    shared.profile.add_metadata(metadata_started.elapsed());

    let metadata = match metadata {
        Ok(metadata) => metadata,
        Err(error) => {
            return MeasuredEntry {
                issues: vec![filesystem_issue(candidate.path, error)],
                ..MeasuredEntry::default()
            };
        }
    };

    let file = shared.measure_file(&candidate.path, &metadata);
    let stats = shared.add_file(file.allocated_bytes);
    shared.add_live_bytes(
        live_child_path,
        file.allocated_bytes,
        file.logical_bytes,
        stats,
    );
    shared.publish_progress_if_due();

    let direct_child = (candidate.visible && collect_direct_children).then(|| DirectChild {
        path: candidate.path,
        name: candidate.name,
        kind: EntryKind::File,
        allocated_bytes: file.allocated_bytes,
        logical_bytes: file.logical_bytes,
        has_visible_children: false,
        issues: Vec::new(),
    });

    MeasuredEntry {
        allocated_bytes: file.allocated_bytes,
        logical_bytes: file.logical_bytes,
        visible: candidate.visible,
        direct_child,
        ..MeasuredEntry::default()
    }
}

fn measure_directory_entry(
    candidate: EntryCandidate,
    shared: &ScanSharedState<'_>,
    live_child_path: Option<&Path>,
    collect_direct_children: bool,
) -> std::io::Result<MeasuredEntry> {
    shared.add_directory();
    if crosses_filesystem_boundary_path(&candidate.path, shared.root_device) {
        if candidate.visible {
            return Ok(MeasuredEntry {
                issues: vec![ScanIssue {
                    path: candidate.path,
                    kind: ReadIssueKind::FilesystemBoundary,
                    message: "Directory is on a different filesystem.".to_string(),
                }],
                ..MeasuredEntry::default()
            });
        }
        return Ok(MeasuredEntry::default());
    }

    let next_live_child_path = if collect_direct_children && candidate.visible {
        Some(candidate.path.as_path())
    } else {
        live_child_path
    };
    let child = measure_directory_contents(&candidate.path, shared, next_live_child_path, false)?;
    let mut summaries = child.summaries;
    summaries.push(DirectorySizeSummary {
        path: candidate.path.clone(),
        config_fingerprint: shared.config_fingerprint.to_string(),
        allocated_size: child.accumulator.allocated_bytes,
        logical_size: child.accumulator.logical_bytes,
        has_visible_children: child.accumulator.has_visible_children,
        issues: child.accumulator.issues.clone(),
    });

    let direct_child = (candidate.visible && collect_direct_children).then(|| DirectChild {
        path: candidate.path,
        name: candidate.name,
        kind: EntryKind::Directory,
        allocated_bytes: child.accumulator.allocated_bytes,
        logical_bytes: child.accumulator.logical_bytes,
        has_visible_children: child.accumulator.has_visible_children,
        issues: child.accumulator.issues.clone(),
    });

    Ok(MeasuredEntry {
        allocated_bytes: child.accumulator.allocated_bytes,
        logical_bytes: child.accumulator.logical_bytes,
        visible: candidate.visible,
        issues: child.accumulator.issues,
        summaries,
        direct_child,
    })
}

fn filesystem_issue(path: PathBuf, error: std::io::Error) -> ScanIssue {
    let kind = if is_permission_denied(&error) {
        ReadIssueKind::PermissionDenied
    } else {
        ReadIssueKind::MetadataFailed
    };
    ScanIssue {
        path,
        kind,
        message: error.to_string(),
    }
}

fn is_permission_denied(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::PermissionDenied || error.raw_os_error() == Some(1)
}

fn crosses_filesystem_boundary_path(path: &Path, root_device: Option<u64>) -> bool {
    let Some(root_device) = root_device else {
        return false;
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).is_ok_and(|metadata| metadata.dev() != root_device)
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanBenchmarkMode {
    FileTypes,
    Metadata,
    SizeMeasurement,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanBenchmarkReport {
    pub entries_visited: u64,
    pub files_visited: u64,
    pub directories_visited: u64,
    pub bytes_measured: u64,
    pub elapsed_nanos: u64,
    pub read_dir_nanos: u64,
    pub file_type_nanos: u64,
    pub metadata_nanos: u64,
    pub size_nanos: u64,
}

pub fn benchmark_directory_walk(
    path: &Path,
    config: &ScanConfig,
    mode: ScanBenchmarkMode,
) -> std::io::Result<ScanBenchmarkReport> {
    let started = Instant::now();
    let path = path.canonicalize()?;
    let root_device = if config.stay_on_filesystem {
        get_device_id(&path)
    } else {
        None
    };
    let mut report = ScanBenchmarkReport::default();
    let mut dedupe = HardLinkDedupe::new();
    benchmark_directory_walk_inner(&path, config, mode, root_device, &mut dedupe, &mut report)?;
    report.elapsed_nanos = duration_nanos(started.elapsed());
    Ok(report)
}

fn benchmark_directory_walk_inner(
    path: &Path,
    config: &ScanConfig,
    mode: ScanBenchmarkMode,
    root_device: Option<u64>,
    dedupe: &mut HardLinkDedupe,
    report: &mut ScanBenchmarkReport,
) -> std::io::Result<()> {
    let read_started = Instant::now();
    let mut entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            report.read_dir_nanos = report
                .read_dir_nanos
                .saturating_add(duration_nanos(read_started.elapsed()));
            if is_permission_denied(&error) || path.exists() {
                return Ok(());
            }
            return Err(error);
        }
    };
    report.read_dir_nanos = report
        .read_dir_nanos
        .saturating_add(duration_nanos(read_started.elapsed()));

    loop {
        let read_next_started = Instant::now();
        let entry_result = entries.next();
        report.read_dir_nanos = report
            .read_dir_nanos
            .saturating_add(duration_nanos(read_next_started.elapsed()));
        let Some(entry_result) = entry_result else {
            break;
        };
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let entry_path = entry.path();
        let file_type_started = Instant::now();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                report.file_type_nanos = report
                    .file_type_nanos
                    .saturating_add(duration_nanos(file_type_started.elapsed()));
                continue;
            }
        };
        report.file_type_nanos = report
            .file_type_nanos
            .saturating_add(duration_nanos(file_type_started.elapsed()));
        report.entries_visited = report.entries_visited.saturating_add(1);

        if file_type.is_dir() {
            report.directories_visited = report.directories_visited.saturating_add(1);
            if crosses_filesystem_boundary_path(&entry_path, root_device) {
                continue;
            }
            benchmark_directory_walk_inner(&entry_path, config, mode, root_device, dedupe, report)?;
            continue;
        }

        if file_type.is_file() || (file_type.is_symlink() && !config.follow_symlinks) {
            report.files_visited = report.files_visited.saturating_add(1);
            if mode == ScanBenchmarkMode::FileTypes {
                continue;
            }
            let metadata_started = Instant::now();
            let metadata = if file_type.is_symlink() && !config.follow_symlinks {
                std::fs::symlink_metadata(&entry_path)
            } else {
                std::fs::metadata(&entry_path)
            };
            report.metadata_nanos = report
                .metadata_nanos
                .saturating_add(duration_nanos(metadata_started.elapsed()));
            let Ok(metadata) = metadata else {
                continue;
            };
            if mode == ScanBenchmarkMode::SizeMeasurement {
                let size_started = Instant::now();
                let measurement = dedupe.measure_file_with_policy(
                    &entry_path,
                    &metadata,
                    config.dedupe_hard_links,
                    config.size_measurement_mode,
                );
                report.size_nanos = report
                    .size_nanos
                    .saturating_add(duration_nanos(size_started.elapsed()));
                report.bytes_measured = report
                    .bytes_measured
                    .saturating_add(measurement.allocated_bytes);
            }
        }
    }

    Ok(())
}

fn duration_nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn discover_directory_counts_hidden_entries_without_emitting_rows_by_default() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".hidden"), b"abc").unwrap();
        std::fs::write(dir.path().join("visible"), b"abc").unwrap();

        let listing = discover_directory(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: None,
                ..ScanConfig::default()
            },
        )
        .unwrap();

        assert_eq!(listing.children.len(), 1);
        assert_eq!(listing.children[0].name, "visible");
        assert!(listing.total_measured_size > listing.total_visible_size);
        assert!(listing.total_logical_size > listing.children[0].logical_size);
        assert!(listing.issues.is_empty());
    }

    #[test]
    fn discover_directory_filters_small_folders() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("small")).unwrap();
        std::fs::write(dir.path().join("small").join("file.txt"), b"abc").unwrap();

        let listing = discover_directory(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: Some(u64::MAX),
                ..ScanConfig::default()
            },
        )
        .unwrap();

        assert!(listing.children.is_empty());
    }

    #[test]
    fn discover_directory_marks_deferred_directory_size_as_partial() {
        let dir = tempdir().unwrap();
        let top = dir.path().join("top");
        let nested = top.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("file.txt"), b"abc").unwrap();

        let listing = discover_directory(
            dir.path(),
            &ScanConfig {
                requested_depth: 2,
                preload_depth: 1,
                min_visible_folder_bytes: None,
                ..ScanConfig::default()
            },
        )
        .unwrap();

        let top_node = listing
            .children
            .iter()
            .find(|node| node.name == "top")
            .unwrap();
        assert_eq!(top_node.state, NodeState::Partial);
        assert_eq!(top_node.size, 0);
    }

    #[test]
    fn precise_directory_discovery_reports_nested_folder_size() {
        let dir = tempdir().unwrap();
        let top = dir.path().join("top");
        let nested = top.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("file.txt"), b"abc").unwrap();

        let precise = discover_directory_with_precise_sizes(
            dir.path(),
            &ScanConfig {
                requested_depth: 2,
                preload_depth: 1,
                min_visible_folder_bytes: None,
                ..ScanConfig::default()
            },
        )
        .unwrap();
        let listing = precise.listing;

        let top_node = listing
            .children
            .iter()
            .find(|node| node.name == "top")
            .unwrap();
        assert_eq!(top_node.state, NodeState::Complete);
        assert!(top_node.size > 0);
    }

    #[test]
    fn precise_directory_discovery_emits_live_updates_for_visible_direct_child() {
        let dir = tempdir().unwrap();
        let top = dir.path().join("top");
        std::fs::create_dir_all(&top).unwrap();
        std::fs::write(top.join("file.txt"), b"abc").unwrap();
        let canonical_top = top.canonicalize().unwrap();
        let updates = Mutex::new(Vec::new());

        let callback = |update: PreciseScanUpdate| {
            updates
                .lock()
                .unwrap()
                .extend(update.directory_updates.into_iter());
        };
        discover_directory_with_precise_sizes_with_cancel_and_progress(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: None,
                live_updates: crate::rules::LiveUpdateConfig {
                    throttle_ms: 0,
                    min_bytes_delta: 0,
                    ..crate::rules::LiveUpdateConfig::default()
                },
                ..ScanConfig::default()
            },
            None,
            Some(&callback),
        )
        .unwrap();

        assert!(
            updates
                .lock()
                .unwrap()
                .iter()
                .any(|update| update.path == canonical_top && update.logical_bytes > 0)
        );
    }

    #[test]
    fn precise_directory_discovery_suppresses_live_updates_when_disabled() {
        let dir = tempdir().unwrap();
        let top = dir.path().join("top");
        std::fs::create_dir_all(&top).unwrap();
        std::fs::write(top.join("file.txt"), b"abc").unwrap();
        let updates = Mutex::new(0usize);

        let callback = |update: PreciseScanUpdate| {
            *updates.lock().unwrap() += update.directory_updates.len();
        };
        discover_directory_with_precise_sizes_with_cancel_and_progress(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: None,
                live_updates: crate::rules::LiveUpdateConfig {
                    enabled: false,
                    ..crate::rules::LiveUpdateConfig::default()
                },
                ..ScanConfig::default()
            },
            None,
            Some(&callback),
        )
        .unwrap();

        assert_eq!(*updates.lock().unwrap(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn precise_directory_discovery_counts_symlink_entry_without_following_target() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("target.txt"), b"abc").unwrap();
        symlink("target.txt", dir.path().join("link.txt")).unwrap();

        let listing = discover_directory_with_precise_sizes(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: None,
                ..ScanConfig::default()
            },
        )
        .unwrap()
        .listing;
        let link_node = listing
            .children
            .iter()
            .find(|node| node.name == "link.txt")
            .unwrap();

        assert_eq!(link_node.kind, EntryKind::File);
        assert_eq!(link_node.logical_size, "target.txt".len() as u64);
        assert!(listing.total_logical_size >= 3 + "target.txt".len() as u64);
        assert!(listing.issues.is_empty());
    }

    #[test]
    fn precise_directory_discovery_counts_hidden_subtrees_without_emitting_rows() {
        let dir = tempdir().unwrap();
        let visible = dir.path().join("visible");
        let hidden = visible.join(".cache");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(hidden.join("data.bin"), b"hidden data").unwrap();

        let config = ScanConfig {
            min_visible_folder_bytes: None,
            ..ScanConfig::default()
        };
        let root_listing = discover_directory_with_precise_sizes(dir.path(), &config)
            .unwrap()
            .listing;
        let visible_node = root_listing
            .children
            .iter()
            .find(|node| node.name == "visible")
            .unwrap();

        assert!(visible_node.size > 0);
        assert!(visible_node.logical_size > 0);

        let visible_listing = discover_directory_with_precise_sizes(&visible, &config)
            .unwrap()
            .listing;

        assert!(visible_listing.children.is_empty());
        assert!(visible_listing.total_measured_size > 0);
        assert!(visible_listing.total_logical_size > 0);
        assert_eq!(visible_listing.total_visible_size, 0);
    }

    #[test]
    fn precise_directory_discovery_keeps_totals_stable_when_hidden_rows_are_shown() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".hidden"), b"abc").unwrap();
        std::fs::write(dir.path().join("visible"), b"abc").unwrap();

        let hidden_rows_omitted = discover_directory_with_precise_sizes(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: None,
                show_hidden: false,
                ..ScanConfig::default()
            },
        )
        .unwrap()
        .listing;
        let hidden_rows_shown = discover_directory_with_precise_sizes(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: None,
                show_hidden: true,
                ..ScanConfig::default()
            },
        )
        .unwrap()
        .listing;

        assert_eq!(
            hidden_rows_omitted.total_measured_size,
            hidden_rows_shown.total_measured_size
        );
        assert_eq!(
            hidden_rows_omitted.total_logical_size,
            hidden_rows_shown.total_logical_size
        );
        assert!(
            !hidden_rows_omitted
                .children
                .iter()
                .any(|node| node.name == ".hidden")
        );
        assert!(
            hidden_rows_shown
                .children
                .iter()
                .any(|node| node.name == ".hidden")
        );
    }

    #[cfg(unix)]
    #[test]
    fn precise_directory_discovery_counts_hard_links_per_path_when_configured() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first.bin");
        let second = dir.path().join("second.bin");
        std::fs::write(&first, vec![1u8; 8192]).unwrap();
        std::fs::hard_link(&first, &second).unwrap();

        let unique_listing = discover_directory_with_precise_sizes(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: None,
                dedupe_hard_links: true,
                ..ScanConfig::default()
            },
        )
        .unwrap()
        .listing;
        let per_path_listing = discover_directory_with_precise_sizes(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: None,
                dedupe_hard_links: false,
                ..ScanConfig::default()
            },
        )
        .unwrap()
        .listing;

        assert!(unique_listing.total_measured_size > 0);
        assert_eq!(
            per_path_listing.total_measured_size,
            unique_listing.total_measured_size * 2
        );
    }

    #[test]
    fn precise_directory_discovery_reports_descendant_summaries() {
        let dir = tempdir().unwrap();
        let top = dir.path().join("top");
        let nested = top.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("file.txt"), b"abc").unwrap();

        let precise = discover_directory_with_precise_sizes(
            dir.path(),
            &ScanConfig {
                requested_depth: 2,
                preload_depth: 1,
                min_visible_folder_bytes: None,
                ..ScanConfig::default()
            },
        )
        .unwrap();
        let canonical_nested = nested.canonicalize().unwrap();

        let nested_summary = precise
            .summaries
            .iter()
            .find(|summary| summary.path == canonical_nested)
            .unwrap();
        assert!(nested_summary.allocated_size > 0);
    }

    #[test]
    fn precise_directory_discovery_stops_when_cancel_token_is_set() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("file.txt"), b"abc").unwrap();
        let cancel_token = CancelToken::new();
        cancel_token.cancel();

        let error = discover_directory_with_precise_sizes_with_cancel(
            dir.path(),
            &ScanConfig {
                min_visible_folder_bytes: None,
                ..ScanConfig::default()
            },
            Some(&cancel_token),
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    }

    #[test]
    fn filesystem_issue_classifies_raw_eperm_as_permission_denied() {
        let issue = filesystem_issue(
            PathBuf::from("/protected"),
            std::io::Error::from_raw_os_error(1),
        );

        assert_eq!(issue.kind, ReadIssueKind::PermissionDenied);
    }
}
