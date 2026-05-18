use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rayon::prelude::*;

use crate::categories::{collect_categories, detect_file_kind};
use crate::driver::{
    DirectoryListing, DirectorySizeSummary, EntryKind, NodeState, PathNode, PreciseDirectoryScan,
    ReadIssueKind, ScanIssue,
};
use crate::jobs::CancelToken;
use crate::rules::ScanConfig;
use crate::safety::classify_path_safety;
use crate::size::{HardLinkDedupe, directory_measurement};
use crate::{FileKind, ReadError, ScanProgress, ScanResult};

#[derive(Debug)]
pub enum ScanUpdate {
    Snapshot {
        progress: ScanProgress,
        directory_sizes: HashMap<PathBuf, u64>,
        categories_map: HashMap<FileKind, u64>,
    },
    Finished(Result<ScanResult>),
    PartialFinished(Result<ScanResult>),
}

pub fn load_current_subdirectories(path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let file_name = entry.file_name();
                    if let Some(name) = file_name.to_str() {
                        if name.starts_with('.') && name != "." && name != ".." {
                            continue;
                        }
                    }
                    dirs.push(entry.path());
                }
            }
        }
    }
    dirs
}

pub fn scan_drive(
    root: &Path,
    started_at: Instant,
    tx: &mpsc::Sender<ScanUpdate>,
    is_partial: bool,
) -> Result<ScanResult> {
    let root = root
        .canonicalize()
        .with_context(|| format!("Unable to access {}", root.display()))?;

    scan_drive_parallel(&root, started_at, tx, is_partial)
}

struct PartialScanResult {
    bytes_scanned: u64,
    files_scanned: u64,
    directories_scanned: u64,
    directory_sizes: HashMap<PathBuf, u64>,
    categories: HashMap<FileKind, u64>,
}

fn scan_drive_parallel(
    root: &Path,
    started_at: Instant,
    tx: &mpsc::Sender<ScanUpdate>,
    is_partial: bool,
) -> Result<ScanResult> {
    let (local_tx, local_rx) = mpsc::channel::<(PathBuf, u64, Option<FileKind>)>();

    let errors = Arc::new(Mutex::new(Vec::<ReadError>::new()));
    let is_done = Arc::new(AtomicBool::new(false));

    let root_dev = get_device_id(root);

    let tx_clone = tx.clone();
    let root_clone = root.to_path_buf();
    let is_done_clone = Arc::clone(&is_done);
    let aggregator_thread = thread::spawn(move || {
        let mut state = PartialScanResult {
            bytes_scanned: 0,
            files_scanned: 0,
            directories_scanned: 0,
            directory_sizes: HashMap::new(),
            categories: FileKind::ALL.into_iter().map(|kind| (kind, 0)).collect(),
        };

        let mut last_update = Instant::now();
        let update_interval = Duration::from_millis(250);

        for (path, size, kind) in local_rx {
            if let Some(kind) = kind {
                state.files_scanned += 1;
                state.bytes_scanned = state.bytes_scanned.saturating_add(size);
                *state.categories.entry(kind).or_default() += size;
            } else {
                state.directories_scanned += 1;
                state.directory_sizes.entry(path.clone()).or_insert(0);
            }

            if size > 0 {
                let mut current = path.as_path();
                while current != root_clone {
                    if let Some(parent) = current.parent() {
                        *state
                            .directory_sizes
                            .entry(parent.to_path_buf())
                            .or_default() += size;
                        current = parent;
                    } else {
                        break;
                    }
                }
            }

            if !is_partial && last_update.elapsed() >= update_interval {
                let progress = ScanProgress {
                    bytes_scanned: state.bytes_scanned,
                    files_scanned: state.files_scanned,
                    directories_scanned: state.directories_scanned,
                };
                let _ = tx_clone.send(ScanUpdate::Snapshot {
                    progress,
                    directory_sizes: state.directory_sizes.clone(),
                    categories_map: state.categories.clone(),
                });
                last_update = Instant::now();
            }
        }

        is_done_clone.store(true, Ordering::Release);
        state
    });

    walk_dir_parallel(root, &local_tx, &errors, root_dev);
    drop(local_tx);

    let mut final_state = aggregator_thread
        .join()
        .expect("Aggregator thread panicked");

    let categories = collect_categories(final_state.categories);
    if !final_state.directory_sizes.contains_key(root) {
        final_state
            .directory_sizes
            .insert(root.to_path_buf(), final_state.bytes_scanned);
    }
    let read_errors = errors.lock().unwrap().clone();
    Ok(ScanResult {
        root: root.to_path_buf(),
        started_at,
        completed_at: Some(Instant::now()),
        bytes_scanned: final_state.bytes_scanned,
        files_scanned: final_state.files_scanned,
        directories_scanned: final_state.directories_scanned,
        directory_sizes: final_state.directory_sizes,
        categories,
        read_errors,
    })
}

fn walk_dir_parallel(
    dir: &Path,
    tx: &mpsc::Sender<(PathBuf, u64, Option<FileKind>)>,
    errors: &Arc<Mutex<Vec<ReadError>>>,
    root_dev: Option<u64>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            let mut errors_guard = errors.lock().unwrap();
            errors_guard.push(ReadError {
                path: dir.to_path_buf(),
                error: e.to_string(),
            });
            return;
        }
    };

    entries.par_bridge().for_each(|entry_res| {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => return,
        };

        let path = entry.path();
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with('.') && file_name != "." && file_name != ".." {
                return;
            }
        }

        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => return,
        };

        if file_type.is_symlink() {
            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                let size = metadata.len();
                let kind = detect_file_kind(&path);
                let _ = tx.send((path.clone(), size, Some(kind)));
            }
            return;
        }

        if file_type.is_dir() {
            if let Some(r_dev) = root_dev {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(meta) = entry.metadata() {
                        if meta.dev() != r_dev {
                            return;
                        }
                    }
                }
            }

            let _ = tx.send((path.clone(), 0, None));
            walk_dir_parallel(&path, tx, errors, root_dev);
        } else if file_type.is_file() {
            if let Ok(metadata) = entry.metadata() {
                let size = file_disk_usage(&metadata);
                let kind = detect_file_kind(&path);
                let _ = tx.send((path.clone(), size, Some(kind)));
            }
        }
    });
}

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

pub fn file_disk_usage(metadata: &Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let allocated = metadata.blocks().saturating_mul(512);
        if allocated > 0 {
            return allocated;
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return metadata.file_size();
    }

    metadata.len()
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
    dedupe: &mut HardLinkDedupe,
    root_device: Option<u64>,
    cancel_token: Option<&CancelToken>,
    progress: Option<&PreciseScanProgressCallback<'_>>,
) -> std::io::Result<PreciseDirectoryScan> {
    let config_fingerprint = config.fingerprint();
    let mut stats = PreciseScanProgress::default();
    let mut last_progress = Instant::now();
    let mut live_updates = LiveUpdateCollector::new(config);
    let MeasuredDirectory {
        accumulator: root_accumulator,
        direct_children,
        summaries,
    } = measure_directory_contents(
        path,
        config,
        dedupe,
        root_device,
        cancel_token,
        progress,
        &mut stats,
        &mut last_progress,
        None,
        &mut live_updates,
        true,
        &config_fingerprint,
    )?;
    if let Some(progress) = progress {
        live_updates.flush_all(progress, &stats);
        progress(PreciseScanUpdate {
            progress: stats,
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
        config_fingerprint,
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

    Ok(PreciseDirectoryScan { listing, summaries })
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
        progress: Option<&PreciseScanProgressCallback<'_>>,
        stats: &PreciseScanProgress,
    ) {
        if !self.enabled {
            return;
        }
        let Some(path) = path else {
            return;
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
            self.flush(progress, stats);
        }
    }

    fn flush_all(
        &mut self,
        progress: &PreciseScanProgressCallback<'_>,
        stats: &PreciseScanProgress,
    ) {
        if !self.enabled {
            return;
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
        self.flush(Some(progress), stats);
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

    fn flush(
        &mut self,
        progress: Option<&PreciseScanProgressCallback<'_>>,
        stats: &PreciseScanProgress,
    ) {
        let Some(progress) = progress else {
            return;
        };
        if self.pending.is_empty() {
            return;
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
        progress(PreciseScanUpdate {
            progress: *stats,
            directory_updates: updates,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn measure_directory_contents(
    path: &Path,
    config: &ScanConfig,
    dedupe: &mut HardLinkDedupe,
    root_device: Option<u64>,
    cancel_token: Option<&CancelToken>,
    progress: Option<&PreciseScanProgressCallback<'_>>,
    stats: &mut PreciseScanProgress,
    last_progress: &mut Instant,
    live_child_path: Option<&Path>,
    live_updates: &mut LiveUpdateCollector,
    collect_direct_children: bool,
    config_fingerprint: &str,
) -> std::io::Result<MeasuredDirectory> {
    if is_canceled(cancel_token) {
        return Err(canceled_error());
    }

    let mut measured = MeasuredDirectory::default();
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            if is_permission_denied(&error) || path.exists() {
                measured
                    .accumulator
                    .issues
                    .push(filesystem_issue(path.to_path_buf(), error));
                return Ok(measured);
            }
            return Err(error);
        }
    };

    for entry_result in entries {
        if is_canceled(cancel_token) {
            return Err(canceled_error());
        }

        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                measured
                    .accumulator
                    .issues
                    .push(filesystem_issue(path.to_path_buf(), error));
                continue;
            }
        };

        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_visible_by_config = !config.is_hidden_name(&name);
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                measured
                    .accumulator
                    .issues
                    .push(filesystem_issue(entry_path, error));
                continue;
            }
        };

        stats.entries_visited = stats.entries_visited.saturating_add(1);
        if file_type.is_symlink() && !config.follow_symlinks {
            match std::fs::symlink_metadata(&entry_path) {
                Ok(metadata) => {
                    let file = dedupe.measure_file_with_policy(
                        &entry_path,
                        &metadata,
                        config.dedupe_hard_links,
                        config.size_measurement_mode,
                    );
                    stats.files_visited = stats.files_visited.saturating_add(1);
                    stats.bytes_measured =
                        stats.bytes_measured.saturating_add(file.allocated_bytes);
                    live_updates.add_bytes(
                        live_child_path,
                        file.allocated_bytes,
                        file.logical_bytes,
                        progress,
                        stats,
                    );
                    measured.accumulator.allocated_bytes = measured
                        .accumulator
                        .allocated_bytes
                        .saturating_add(file.allocated_bytes);
                    measured.accumulator.logical_bytes = measured
                        .accumulator
                        .logical_bytes
                        .saturating_add(file.logical_bytes);
                    if is_visible_by_config {
                        measured.accumulator.has_visible_children = true;
                        if collect_direct_children {
                            measured.direct_children.push(DirectChild {
                                path: entry_path,
                                name,
                                kind: EntryKind::File,
                                allocated_bytes: file.allocated_bytes,
                                logical_bytes: file.logical_bytes,
                                has_visible_children: false,
                                issues: Vec::new(),
                            });
                        }
                    }
                }
                Err(error) => measured
                    .accumulator
                    .issues
                    .push(filesystem_issue(entry_path, error)),
            }
            publish_precise_progress(progress, stats, last_progress);
            continue;
        }

        if file_type.is_dir() {
            stats.directories_visited = stats.directories_visited.saturating_add(1);
            if crosses_filesystem_boundary_path(&entry_path, root_device) {
                if is_visible_by_config {
                    measured.accumulator.issues.push(ScanIssue {
                        path: entry_path,
                        kind: ReadIssueKind::FilesystemBoundary,
                        message: "Directory is on a different filesystem.".to_string(),
                    });
                }
                continue;
            }

            let child = measure_directory_contents(
                &entry_path,
                config,
                dedupe,
                root_device,
                cancel_token,
                progress,
                stats,
                last_progress,
                if collect_direct_children && is_visible_by_config {
                    Some(entry_path.as_path())
                } else {
                    live_child_path
                },
                live_updates,
                false,
                config_fingerprint,
            )?;
            measured.accumulator.allocated_bytes = measured
                .accumulator
                .allocated_bytes
                .saturating_add(child.accumulator.allocated_bytes);
            measured.accumulator.logical_bytes = measured
                .accumulator
                .logical_bytes
                .saturating_add(child.accumulator.logical_bytes);
            measured
                .accumulator
                .issues
                .extend(child.accumulator.issues.clone());
            measured.summaries.extend(child.summaries);
            measured.summaries.push(DirectorySizeSummary {
                path: entry_path.clone(),
                config_fingerprint: config_fingerprint.to_string(),
                allocated_size: child.accumulator.allocated_bytes,
                logical_size: child.accumulator.logical_bytes,
                has_visible_children: child.accumulator.has_visible_children,
                issues: child.accumulator.issues.clone(),
            });

            if is_visible_by_config {
                measured.accumulator.has_visible_children = true;
                if collect_direct_children {
                    measured.direct_children.push(DirectChild {
                        path: entry_path,
                        name,
                        kind: EntryKind::Directory,
                        allocated_bytes: child.accumulator.allocated_bytes,
                        logical_bytes: child.accumulator.logical_bytes,
                        has_visible_children: child.accumulator.has_visible_children,
                        issues: child.accumulator.issues,
                    });
                }
            }
        } else if file_type.is_file() {
            match entry.metadata() {
                Ok(metadata) => {
                    let file = dedupe.measure_file_with_policy(
                        &entry_path,
                        &metadata,
                        config.dedupe_hard_links,
                        config.size_measurement_mode,
                    );
                    stats.files_visited = stats.files_visited.saturating_add(1);
                    stats.bytes_measured =
                        stats.bytes_measured.saturating_add(file.allocated_bytes);
                    live_updates.add_bytes(
                        live_child_path,
                        file.allocated_bytes,
                        file.logical_bytes,
                        progress,
                        stats,
                    );
                    measured.accumulator.allocated_bytes = measured
                        .accumulator
                        .allocated_bytes
                        .saturating_add(file.allocated_bytes);
                    measured.accumulator.logical_bytes = measured
                        .accumulator
                        .logical_bytes
                        .saturating_add(file.logical_bytes);
                    if is_visible_by_config {
                        measured.accumulator.has_visible_children = true;
                        if collect_direct_children {
                            measured.direct_children.push(DirectChild {
                                path: entry_path,
                                name,
                                kind: EntryKind::File,
                                allocated_bytes: file.allocated_bytes,
                                logical_bytes: file.logical_bytes,
                                has_visible_children: false,
                                issues: Vec::new(),
                            });
                        }
                    }
                }
                Err(error) => measured
                    .accumulator
                    .issues
                    .push(filesystem_issue(entry_path, error)),
            }
        }

        publish_precise_progress(progress, stats, last_progress);
    }

    Ok(measured)
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

fn publish_precise_progress(
    progress: Option<&PreciseScanProgressCallback<'_>>,
    stats: &PreciseScanProgress,
    last_progress: &mut Instant,
) {
    let Some(progress) = progress else {
        return;
    };

    if last_progress.elapsed() >= Duration::from_millis(250) {
        progress(PreciseScanUpdate {
            progress: *stats,
            directory_updates: Vec::new(),
        });
        *last_progress = Instant::now();
    }
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

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, mpsc};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn scans_temp_directory_and_records_root_total() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub").join("b.rs"), b"fn main() {}").unwrap();

        let (tx, _rx) = mpsc::channel();
        let result = scan_drive(dir.path(), Instant::now(), &tx, false).unwrap();

        assert!(result.files_scanned >= 2);
        assert!(result.directory_sizes.contains_key(&result.root));
        assert!(result.bytes_scanned > 0);
    }

    #[cfg(unix)]
    #[test]
    fn scan_drive_counts_symlink_entries_without_following_targets() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.txt");
        std::fs::write(&target, b"abc").unwrap();
        symlink("target.txt", dir.path().join("link.txt")).unwrap();

        let (tx, _rx) = mpsc::channel();
        let result = scan_drive(dir.path(), Instant::now(), &tx, false).unwrap();

        assert_eq!(result.files_scanned, 2);
    }

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
