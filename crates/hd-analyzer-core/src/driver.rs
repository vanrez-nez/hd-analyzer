use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cache::{CacheFreshness, DirectoryCache};
use crate::drives;
use crate::jobs::{JobRegistry, JobState, ProgressSnapshot};
use crate::rules::ScanConfig;

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("path is outside the selected volume: {0}")]
    PathOutsideVolume(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("driver unavailable: {0}")]
    DriverUnavailable(String),
    #[error("scan canceled")]
    ScanCanceled,
    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub type DriverResult<T> = Result<T, DriverError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Volume,
    Directory,
    File,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    Queued,
    Working,
    Partial,
    Complete,
    Skipped,
    Failed,
    Stale,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadIssueKind {
    PermissionDenied,
    NotFound,
    NotDirectory,
    HiddenSkipped,
    FilesystemBoundary,
    SymlinkSkipped,
    MetadataFailed,
    SizeUnavailable,
    VolumeUnmounted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanIssue {
    pub path: PathBuf,
    pub kind: ReadIssueKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteSafetyClassification {
    UserContent,
    SafeJunk,
    ReviewRequired,
    ProtectedSystem,
    NotDeletableNow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteSafetyFlag {
    SystemOwned,
    SipProtected,
    Immutable,
    AppendOnly,
    ParentNotWritable,
    NotDeletableNow,
    SafeJunkRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathDeleteSafety {
    pub classification: DeleteSafetyClassification,
    pub can_delete_now: bool,
    pub flags: Vec<DeleteSafetyFlag>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub id: String,
    pub display_name: String,
    pub root_path: PathBuf,
    pub mount_point: PathBuf,
    pub filesystem: String,
    pub storage_kind: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub is_removable: bool,
    pub is_read_only: bool,
    pub is_accessible: bool,
    pub access_issue: Option<String>,
}

impl From<drives::Drive> for Volume {
    fn from(drive: drives::Drive) -> Self {
        let is_accessible = std::fs::read_dir(&drive.mount_point).is_ok();
        Self {
            id: drive.mount_point.display().to_string(),
            display_name: drive.label,
            root_path: drive.mount_point.clone(),
            mount_point: drive.mount_point,
            filesystem: drive.file_system,
            storage_kind: drive.storage_kind,
            total_bytes: drive.total_space,
            available_bytes: drive.available_space,
            is_removable: drive.is_removable,
            is_read_only: drive.is_read_only,
            is_accessible,
            access_issue: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathNode {
    pub path: PathBuf,
    pub name: String,
    pub kind: EntryKind,
    pub parent_path: Option<PathBuf>,
    pub depth_from_request: usize,
    pub size: u64,
    pub logical_size: u64,
    pub state: NodeState,
    pub visible: bool,
    pub children_known: bool,
    pub active_job_id: Option<String>,
    pub delete_safety: PathDeleteSafety,
    pub issues: Vec<ScanIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    pub path: PathBuf,
    pub config_fingerprint: String,
    pub children: Vec<PathNode>,
    pub total_visible_size: u64,
    pub total_measured_size: u64,
    pub total_logical_size: u64,
    pub state: NodeState,
    pub loaded_depth: usize,
    pub has_more_depth: bool,
    pub issues: Vec<ScanIssue>,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySizeSummary {
    pub path: PathBuf,
    pub config_fingerprint: String,
    pub allocated_size: u64,
    pub logical_size: u64,
    pub has_visible_children: bool,
    pub issues: Vec<ScanIssue>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreciseScanProfile {
    pub entries_collected: u64,
    pub read_dir_nanos: u64,
    pub file_type_nanos: u64,
    pub metadata_nanos: u64,
    pub size_nanos: u64,
    pub live_update_nanos: u64,
    pub progress_emit_nanos: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreciseDirectoryScan {
    pub listing: DirectoryListing,
    pub summaries: Vec<DirectorySizeSummary>,
    pub profile: PreciseScanProfile,
}

#[derive(Debug, Clone)]
pub struct OpenPathRequest {
    pub volume_root: PathBuf,
    pub path: PathBuf,
    pub config: ScanConfig,
    pub request_id: String,
}

#[derive(Debug, Clone)]
pub struct StartScanRequest {
    pub volume_root: PathBuf,
    pub path: PathBuf,
    pub config: ScanConfig,
    pub replace_existing: bool,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidationScope {
    PathOnly,
    PathAndDescendants,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidationReceipt {
    pub invalidated_paths: Vec<PathBuf>,
    pub superseded_job_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartScanReceipt {
    pub job_id: String,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryProgressUpdate {
    pub path: PathBuf,
    pub size: u64,
    pub logical_size: u64,
    pub state: NodeState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum DriverEvent {
    JobQueued {
        job_id: String,
        request_id: String,
        path: PathBuf,
    },
    JobStarted {
        job_id: String,
        request_id: String,
        path: PathBuf,
    },
    DirectoryReady {
        job_id: String,
        request_id: String,
        path: PathBuf,
        listing: DirectoryListing,
    },
    ProgressSnapshot {
        job_id: String,
        request_id: String,
        snapshot: ProgressSnapshot,
    },
    DirectoryProgress {
        job_id: String,
        request_id: String,
        path: PathBuf,
        updates: Vec<DirectoryProgressUpdate>,
        snapshot: ProgressSnapshot,
    },
    JobFinished {
        job_id: String,
        request_id: String,
        path: PathBuf,
    },
    JobFailed {
        job_id: String,
        request_id: String,
        path: PathBuf,
        message: String,
    },
    PathInvalidated {
        path: PathBuf,
        scope: InvalidationScope,
        generation: u64,
    },
}

pub trait HdDriver: Send + Sync {
    fn list_volumes(&self) -> DriverResult<Vec<Volume>>;
    fn open_path(&self, request: OpenPathRequest) -> DriverResult<DirectoryListing>;
    fn get_directory(
        &self,
        volume_root: &Path,
        path: &Path,
        config: &ScanConfig,
    ) -> DriverResult<DirectoryListing>;
    fn start_scan(
        &self,
        request: StartScanRequest,
        sink: Option<Arc<dyn Fn(DriverEvent) + Send + Sync>>,
    ) -> DriverResult<StartScanReceipt>;
    fn invalidate_path(
        &self,
        volume_root: &Path,
        path: &Path,
        scope: InvalidationScope,
    ) -> DriverResult<InvalidationReceipt>;
    fn cancel_job(&self, job_id: &str) -> DriverResult<()>;
}

#[derive(Debug, Clone, Default)]
pub struct LocalHdDriver {
    cache: Arc<Mutex<DirectoryCache>>,
    jobs: Arc<JobRegistry>,
    scan_gate: Arc<Mutex<()>>,
}

impl LocalHdDriver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_path_with_progress(
        &self,
        request: OpenPathRequest,
        progress: Option<&crate::scan::DirectoryOpenProgressCallback<'_>>,
    ) -> DriverResult<DirectoryListing> {
        self.get_directory_with_open_progress(
            &request.volume_root,
            &request.path,
            &request.config,
            progress,
        )
    }

    fn discover_and_cache_with_progress(
        &self,
        path: &Path,
        config: &ScanConfig,
        progress: Option<&crate::scan::DirectoryOpenProgressCallback<'_>>,
    ) -> DriverResult<DirectoryListing> {
        let mut listing = crate::scan::discover_directory_with_progress(path, config, progress)
            .map_err(|source| DriverError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        self.enrich_listing_from_cached_summaries(&mut listing);
        let entry = self
            .cache
            .lock()
            .expect("directory cache poisoned")
            .upsert(listing, CacheFreshness::Fresh);
        Ok(entry.listing)
    }

    fn discover_precise_and_cache(
        &self,
        path: &Path,
        config: &ScanConfig,
        cancel_token: Option<&crate::jobs::CancelToken>,
        progress: Option<&crate::scan::PreciseScanProgressCallback<'_>>,
    ) -> DriverResult<DirectoryListing> {
        let started = Instant::now();
        let precise = crate::scan::discover_directory_with_precise_sizes_with_cancel_and_progress(
            path,
            config,
            cancel_token,
            progress,
        )
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::Interrupted {
                DriverError::ScanCanceled
            } else {
                DriverError::Io {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })?;
        let scan_elapsed = started.elapsed();
        log::info!(
            "precise filesystem scan completed for {} in {:?} with {} child node(s), {} cached summaries, total measured size {}, total logical size {}",
            path.display(),
            scan_elapsed,
            precise.listing.children.len(),
            precise.summaries.len(),
            precise.listing.total_measured_size,
            precise.listing.total_logical_size
        );
        log::info!(
            "precise filesystem scan profile for {}: entries collected {}, worker-time read_dir {:.3}s, file_type {:.3}s, metadata {:.3}s, size {:.3}s, live updates {:.3}s, progress emit {:.3}s",
            path.display(),
            precise.profile.entries_collected,
            nanos_to_seconds(precise.profile.read_dir_nanos),
            nanos_to_seconds(precise.profile.file_type_nanos),
            nanos_to_seconds(precise.profile.metadata_nanos),
            nanos_to_seconds(precise.profile.size_nanos),
            nanos_to_seconds(precise.profile.live_update_nanos),
            nanos_to_seconds(precise.profile.progress_emit_nanos)
        );
        let cache_started = Instant::now();
        let entry = self
            .cache
            .lock()
            .expect("directory cache poisoned")
            .upsert_precise(precise, CacheFreshness::Fresh);
        log::info!(
            "precise filesystem scan cache update for {} completed in {:?}",
            path.display(),
            cache_started.elapsed()
        );
        Ok(entry.listing)
    }

    fn enrich_listing_from_cached_summaries(&self, listing: &mut DirectoryListing) {
        let mut total_visible_size = listing.total_visible_size;
        let mut total_measured_size = listing.total_measured_size;
        let mut total_logical_size = listing.total_logical_size;
        let mut has_more_depth = false;
        let cache = self.cache.lock().expect("directory cache poisoned");

        for child in &mut listing.children {
            if child.kind == EntryKind::Directory {
                if let Some(summary) = cache.get_summary(&child.path, &listing.config_fingerprint) {
                    let previous_size = child.size;
                    let previous_logical_size = child.logical_size;
                    child.size = summary.allocated_size;
                    child.logical_size = summary.logical_size;
                    child.state = NodeState::Complete;
                    child.children_known = !summary.has_visible_children;
                    child.issues = summary.issues;
                    total_measured_size =
                        adjusted_total(total_measured_size, previous_size, child.size);
                    total_logical_size = adjusted_total(
                        total_logical_size,
                        previous_logical_size,
                        child.logical_size,
                    );
                    if child.visible {
                        total_visible_size =
                            adjusted_total(total_visible_size, previous_size, child.size);
                    }
                }
            }

            has_more_depth |= !child.children_known;
        }

        listing.total_visible_size = total_visible_size;
        listing.total_measured_size = total_measured_size;
        listing.total_logical_size = total_logical_size;
        listing.has_more_depth = has_more_depth;
    }

    fn scoped_existing_path(&self, volume_root: &Path, path: &Path) -> DriverResult<PathBuf> {
        let canonical_root = canonicalize_volume_root(volume_root)?;
        let canonical_path = path.canonicalize().map_err(|source| DriverError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        ensure_inside_volume(&canonical_root, &canonical_path)?;
        Ok(canonical_path)
    }

    fn scoped_invalidation_path(&self, volume_root: &Path, path: &Path) -> DriverResult<PathBuf> {
        let canonical_root = canonicalize_volume_root(volume_root)?;
        if path.exists() {
            return self.scoped_existing_path(&canonical_root, path);
        }

        let absolute_path = if path.is_absolute() {
            path.strip_prefix(volume_root)
                .map(|relative| canonical_root.join(relative))
                .unwrap_or_else(|_| path.to_path_buf())
        } else {
            canonical_root.join(path)
        };
        let normalized_root = normalize_path_lexically(&canonical_root)?;
        let normalized_path = normalize_path_lexically(&absolute_path)?;
        ensure_inside_volume(&normalized_root, &normalized_path)?;
        Ok(normalized_path)
    }

    fn estimate_scan_total_bytes(
        &self,
        volume_root: &Path,
        path: &Path,
        config: &ScanConfig,
    ) -> Option<u64> {
        let canonical_root = canonicalize_volume_root(volume_root).ok()?;
        if let Some(used_space) = estimate_volume_root_used_space(&canonical_root, path) {
            return Some(used_space);
        }

        let fingerprint = config.fingerprint();
        let summary_estimate = self
            .cache
            .lock()
            .expect("directory cache poisoned")
            .get_summary(path, &fingerprint)
            .map(|summary| summary.allocated_size);

        summary_estimate.filter(|estimate| *estimate > 0)
    }

    fn get_directory_with_open_progress(
        &self,
        volume_root: &Path,
        path: &Path,
        config: &ScanConfig,
        progress: Option<&crate::scan::DirectoryOpenProgressCallback<'_>>,
    ) -> DriverResult<DirectoryListing> {
        let config = config.clone().normalized();
        let scoped_path = self.scoped_existing_path(volume_root, path)?;
        let fingerprint = config.fingerprint();
        let cached_entry = {
            self.cache
                .lock()
                .expect("directory cache poisoned")
                .get(&scoped_path, &fingerprint)
        };
        if let Some(entry) = cached_entry {
            if entry.freshness != CacheFreshness::Stale {
                let mut listing = entry.listing;
                self.enrich_listing_from_cached_summaries(&mut listing);
                return Ok(listing);
            }
        }
        self.discover_and_cache_with_progress(&scoped_path, &config, progress)
    }
}

impl HdDriver for LocalHdDriver {
    fn list_volumes(&self) -> DriverResult<Vec<Volume>> {
        drives::list_drives()
            .map(|drives| drives.into_iter().map(Volume::from).collect())
            .map_err(|error| DriverError::DriverUnavailable(error.to_string()))
    }

    fn open_path(&self, request: OpenPathRequest) -> DriverResult<DirectoryListing> {
        self.get_directory(&request.volume_root, &request.path, &request.config)
    }

    fn get_directory(
        &self,
        volume_root: &Path,
        path: &Path,
        config: &ScanConfig,
    ) -> DriverResult<DirectoryListing> {
        self.get_directory_with_open_progress(volume_root, path, config, None)
    }

    fn start_scan(
        &self,
        request: StartScanRequest,
        sink: Option<Arc<dyn Fn(DriverEvent) + Send + Sync>>,
    ) -> DriverResult<StartScanReceipt> {
        let scoped_path = self.scoped_existing_path(&request.volume_root, &request.path)?;
        if request.replace_existing {
            self.jobs.supersede_path(&scoped_path);
        }
        let config = request.config.normalized();
        let estimated_total_bytes =
            self.estimate_scan_total_bytes(&request.volume_root, &scoped_path, &config);
        let config_fingerprint = config.fingerprint();
        if !request.replace_existing {
            if let Some(existing) = self.jobs.active_exact(&scoped_path, &config_fingerprint) {
                log::info!(
                    "coalescing filesystem scan for {} into active job {}",
                    scoped_path.display(),
                    existing.job_id
                );
                return Ok(StartScanReceipt {
                    job_id: existing.job_id,
                    request_id: existing.request_id,
                });
            }
        }
        let handle = self.jobs.create(scoped_path.clone(), config_fingerprint);
        let receipt = StartScanReceipt {
            job_id: handle.job_id.clone(),
            request_id: handle.request_id.clone(),
        };
        if let Some(sink) = &sink {
            sink(DriverEvent::JobQueued {
                job_id: handle.job_id.clone(),
                request_id: handle.request_id.clone(),
                path: scoped_path,
            });
        }

        let driver = self.clone();
        let sink_for_worker = sink.clone();
        thread::spawn(move || {
            if handle.cancel_token.is_canceled() {
                driver.jobs.set_state(&handle.job_id, JobState::Canceled);
                return;
            }
            let scan_gate = Arc::clone(&driver.scan_gate);
            let _scan_guard = scan_gate.lock().expect("scan gate poisoned");
            if handle.cancel_token.is_canceled() {
                driver.jobs.set_state(&handle.job_id, JobState::Canceled);
                return;
            }
            driver.jobs.set_state(&handle.job_id, JobState::Running);
            if let Some(sink) = &sink_for_worker {
                sink(DriverEvent::JobStarted {
                    job_id: handle.job_id.clone(),
                    request_id: handle.request_id.clone(),
                    path: handle.root_path.clone(),
                });
            }

            let latest_scan_progress =
                Arc::new(Mutex::new(crate::scan::PreciseScanProgress::default()));
            let progress_callback = sink_for_worker.as_ref().map(|sink| {
                let sink = Arc::clone(sink);
                let job_id = handle.job_id.clone();
                let request_id = handle.request_id.clone();
                let root_path = handle.root_path.clone();
                let latest_scan_progress = Arc::clone(&latest_scan_progress);
                move |scan_update: crate::scan::PreciseScanUpdate| {
                    let scan_progress = scan_update.progress;
                    *latest_scan_progress
                        .lock()
                        .expect("scan progress state poisoned") = scan_progress;
                    let mut snapshot = ProgressSnapshot::new(job_id.clone(), request_id.clone());
                    snapshot.state = JobState::Running;
                    snapshot.estimated_total_bytes = estimated_total_bytes;
                    snapshot.scheduled_units = estimated_total_bytes.unwrap_or(0);
                    snapshot.discovered_units = scan_progress.entries_visited;
                    snapshot.completed_units = scan_progress.entries_visited;
                    snapshot.active_units = 1;
                    snapshot.bytes_measured = scan_progress.bytes_measured;
                    if scan_update.directory_updates.is_empty() {
                        sink(DriverEvent::ProgressSnapshot {
                            job_id: job_id.clone(),
                            request_id: request_id.clone(),
                            snapshot,
                        });
                    } else {
                        sink(DriverEvent::DirectoryProgress {
                            job_id: job_id.clone(),
                            request_id: request_id.clone(),
                            path: root_path.clone(),
                            updates: scan_update
                                .directory_updates
                                .into_iter()
                                .map(|update| DirectoryProgressUpdate {
                                    path: update.path,
                                    size: update.allocated_bytes,
                                    logical_size: update.logical_bytes,
                                    state: NodeState::Working,
                                })
                                .collect(),
                            snapshot,
                        });
                    }
                }
            });
            let progress_ref = progress_callback
                .as_ref()
                .map(|callback| callback as &crate::scan::PreciseScanProgressCallback<'_>);
            let result = driver.discover_precise_and_cache(
                &handle.root_path,
                &config,
                Some(&handle.cancel_token),
                progress_ref,
            );
            match result {
                Ok(listing) => {
                    let scan_progress = *latest_scan_progress
                        .lock()
                        .expect("scan progress state poisoned");
                    log::info!(
                        "filesystem scan job {} visited {} entries ({} files, {} directories), measured {} bytes",
                        handle.job_id,
                        scan_progress.entries_visited,
                        scan_progress.files_visited,
                        scan_progress.directories_visited,
                        scan_progress.bytes_measured
                    );
                    if handle.cancel_token.is_canceled() {
                        driver.jobs.set_state(&handle.job_id, JobState::Canceled);
                        return;
                    }
                    driver.jobs.set_state(&handle.job_id, JobState::Completed);
                    if let Some(sink) = &sink_for_worker {
                        let emit_started = Instant::now();
                        let mut progress =
                            ProgressSnapshot::new(handle.job_id.clone(), handle.request_id.clone());
                        progress.state = JobState::Completed;
                        progress.estimated_total_bytes = Some(listing.total_measured_size);
                        progress.scheduled_units = listing.total_measured_size;
                        progress.discovered_units = listing.children.len() as u64;
                        progress.completed_units = listing.children.len() as u64;
                        progress.bytes_measured = listing.total_measured_size;
                        sink(DriverEvent::DirectoryReady {
                            job_id: handle.job_id.clone(),
                            request_id: handle.request_id.clone(),
                            path: listing.path.clone(),
                            listing,
                        });
                        sink(DriverEvent::ProgressSnapshot {
                            job_id: handle.job_id.clone(),
                            request_id: handle.request_id.clone(),
                            snapshot: progress,
                        });
                        sink(DriverEvent::JobFinished {
                            job_id: handle.job_id.clone(),
                            request_id: handle.request_id.clone(),
                            path: handle.root_path.clone(),
                        });
                        log::info!(
                            "filesystem scan job {} emitted final events in {:?}",
                            handle.job_id,
                            emit_started.elapsed()
                        );
                    }
                }
                Err(error) => {
                    if matches!(error, DriverError::ScanCanceled)
                        || handle.cancel_token.is_canceled()
                        || matches!(
                            driver.jobs.state(&handle.job_id),
                            Some(JobState::Superseded | JobState::Canceled)
                        )
                    {
                        if !matches!(
                            driver.jobs.state(&handle.job_id),
                            Some(JobState::Superseded)
                        ) {
                            driver.jobs.set_state(&handle.job_id, JobState::Canceled);
                        }
                        return;
                    }
                    driver.jobs.set_state(&handle.job_id, JobState::Failed);
                    if let Some(sink) = &sink_for_worker {
                        sink(DriverEvent::JobFailed {
                            job_id: handle.job_id.clone(),
                            request_id: handle.request_id.clone(),
                            path: handle.root_path.clone(),
                            message: error.to_string(),
                        });
                    }
                }
            }
        });

        Ok(receipt)
    }

    fn invalidate_path(
        &self,
        volume_root: &Path,
        path: &Path,
        scope: InvalidationScope,
    ) -> DriverResult<InvalidationReceipt> {
        let scoped_path = self.scoped_invalidation_path(volume_root, path)?;
        let descendants = scope == InvalidationScope::PathAndDescendants;
        let invalidated_paths = self
            .cache
            .lock()
            .expect("directory cache poisoned")
            .mark_stale(&scoped_path, descendants);
        let superseded_job_ids = self.jobs.supersede_path(&scoped_path);
        Ok(InvalidationReceipt {
            invalidated_paths,
            superseded_job_ids,
        })
    }

    fn cancel_job(&self, job_id: &str) -> DriverResult<()> {
        if self.jobs.cancel(job_id) {
            Ok(())
        } else {
            Err(DriverError::JobNotFound(job_id.to_string()))
        }
    }
}

fn adjusted_total(total: u64, previous_value: u64, next_value: u64) -> u64 {
    if next_value >= previous_value {
        total.saturating_add(next_value - previous_value)
    } else {
        total.saturating_sub(previous_value - next_value)
    }
}

fn nanos_to_seconds(nanos: u64) -> f64 {
    nanos as f64 / 1_000_000_000.0
}

fn estimate_volume_root_used_space(canonical_root: &Path, path: &Path) -> Option<u64> {
    if !equivalent_paths(path, canonical_root) {
        return None;
    }

    drives::list_drives()
        .ok()?
        .into_iter()
        .find_map(|drive| {
            if drive_mount_matches_root(&drive.mount_point, canonical_root) {
                Some(drive.used_space())
            } else {
                None
            }
        })
        .filter(|used_space| *used_space > 0)
}

fn drive_mount_matches_root(mount_point: &Path, canonical_root: &Path) -> bool {
    mount_point
        .canonicalize()
        .is_ok_and(|drive_root| equivalent_paths(&drive_root, canonical_root))
        || equivalent_paths(mount_point, canonical_root)
}

fn equivalent_paths(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }

    match (
        normalize_path_lexically(left),
        normalize_path_lexically(right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn canonicalize_volume_root(volume_root: &Path) -> DriverResult<PathBuf> {
    volume_root
        .canonicalize()
        .map_err(|source| DriverError::Io {
            path: volume_root.to_path_buf(),
            source,
        })
}

fn ensure_inside_volume(volume_root: &Path, path: &Path) -> DriverResult<()> {
    if path == volume_root || path.starts_with(volume_root) {
        Ok(())
    } else {
        Err(DriverError::PathOutsideVolume(format!(
            "{} is outside {}",
            path.display(),
            volume_root.display()
        )))
    }
}

fn normalize_path_lexically(path: &Path) -> DriverResult<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(DriverError::InvalidPath(format!(
                        "Path escapes its root: {}",
                        path.display()
                    )));
                }
            }
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn open_path_returns_immediate_listing() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        let driver = LocalHdDriver::new();

        let listing = driver
            .open_path(OpenPathRequest {
                volume_root: dir.path().to_path_buf(),
                path: dir.path().to_path_buf(),
                config: ScanConfig {
                    min_visible_folder_bytes: None,
                    ..ScanConfig::default()
                },
                request_id: "test".to_string(),
            })
            .unwrap();

        assert_eq!(listing.children.len(), 1);
    }

    #[test]
    fn open_path_preserves_hidden_direct_totals_when_rows_are_omitted() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".hidden"), b"abc").unwrap();
        std::fs::write(dir.path().join("visible"), b"abc").unwrap();
        let driver = LocalHdDriver::new();

        let listing = driver
            .open_path(OpenPathRequest {
                volume_root: dir.path().to_path_buf(),
                path: dir.path().to_path_buf(),
                config: ScanConfig {
                    min_visible_folder_bytes: None,
                    show_hidden: false,
                    ..ScanConfig::default()
                },
                request_id: "test".to_string(),
            })
            .unwrap();

        assert_eq!(listing.children.len(), 1);
        assert_eq!(listing.children[0].name, "visible");
        assert!(listing.total_measured_size > listing.total_visible_size);
        assert!(listing.total_logical_size > listing.children[0].logical_size);
    }

    #[test]
    fn rejects_parent_path_outside_volume_root() {
        let volume = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let driver = LocalHdDriver::new();

        let error = driver
            .get_directory(volume.path(), outside.path(), &ScanConfig::default())
            .unwrap_err();

        assert!(matches!(error, DriverError::PathOutsideVolume(_)));
    }

    #[test]
    fn accepts_child_path_inside_volume_root() {
        let volume = tempdir().unwrap();
        let child = volume.path().join("child");
        std::fs::create_dir(&child).unwrap();
        let driver = LocalHdDriver::new();

        let listing = driver
            .get_directory(volume.path(), &child, &ScanConfig::default())
            .unwrap();

        assert_eq!(listing.path, child.canonicalize().unwrap());
    }

    #[test]
    fn equivalent_paths_accepts_lexically_equal_paths() {
        let left = Path::new("volume").join("folder").join("..").join("root");
        let right = Path::new("volume").join("root");

        assert!(equivalent_paths(&left, &right));
    }

    #[test]
    fn child_listing_reuses_precise_descendant_size_summaries() {
        let volume = tempdir().unwrap();
        let top = volume.path().join("top");
        let nested = top.join("nested");
        let deep = nested.join("deep");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("file.txt"), b"abc").unwrap();
        let driver = LocalHdDriver::new();
        let config = ScanConfig {
            requested_depth: 2,
            preload_depth: 1,
            min_visible_folder_bytes: None,
            ..ScanConfig::default()
        };

        driver
            .discover_precise_and_cache(volume.path(), &config, None, None)
            .unwrap();
        let listing = driver.get_directory(volume.path(), &top, &config).unwrap();

        let nested_node = listing
            .children
            .iter()
            .find(|node| node.path == nested.canonicalize().unwrap())
            .unwrap();
        assert_eq!(nested_node.state, NodeState::Complete);
        assert!(nested_node.size > 0);
    }

    #[test]
    fn child_listing_reuses_hidden_inclusive_precise_descendant_size_summaries() {
        let volume = tempdir().unwrap();
        let top = volume.path().join("top");
        let nested = top.join("nested");
        let hidden = nested.join(".hidden");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(hidden.join("file.txt"), b"abc").unwrap();
        let driver = LocalHdDriver::new();
        let config = ScanConfig {
            requested_depth: 2,
            preload_depth: 1,
            min_visible_folder_bytes: None,
            show_hidden: false,
            ..ScanConfig::default()
        };

        driver
            .discover_precise_and_cache(volume.path(), &config, None, None)
            .unwrap();
        let listing = driver.get_directory(volume.path(), &top, &config).unwrap();

        let nested_node = listing
            .children
            .iter()
            .find(|node| node.path == nested.canonicalize().unwrap())
            .unwrap();
        assert_eq!(nested_node.state, NodeState::Complete);
        assert!(nested_node.size > 0);
        assert!(!listing.children.iter().any(|node| node.name == ".hidden"));
    }

    #[test]
    fn parent_listing_reuses_precise_scan_root_size_summary() {
        let volume = tempdir().unwrap();
        let parent = volume.path().join("parent");
        let child = parent.join("child");
        let deep = child.join("deep");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("file.txt"), b"abc").unwrap();
        let driver = LocalHdDriver::new();
        let config = ScanConfig {
            requested_depth: 2,
            preload_depth: 1,
            min_visible_folder_bytes: None,
            ..ScanConfig::default()
        };

        let initial_parent = driver
            .get_directory(volume.path(), &parent, &config)
            .unwrap();
        let initial_child = initial_parent
            .children
            .iter()
            .find(|node| node.path == child.canonicalize().unwrap())
            .unwrap();
        assert_eq!(initial_child.state, NodeState::Partial);
        assert_eq!(initial_child.size, 0);

        driver
            .discover_precise_and_cache(&child, &config, None, None)
            .unwrap();
        let listing = driver
            .get_directory(volume.path(), &parent, &config)
            .unwrap();

        let child_node = listing
            .children
            .iter()
            .find(|node| node.path == child.canonicalize().unwrap())
            .unwrap();
        assert_eq!(child_node.state, NodeState::Complete);
        assert!(child_node.size > 0);
        assert_eq!(listing.total_measured_size, child_node.size);
    }

    #[test]
    fn invalidation_rejects_lexical_escape_outside_volume_root() {
        let volume = tempdir().unwrap();
        let driver = LocalHdDriver::new();
        let escape = volume.path().join("..").join("outside");

        let error = driver
            .invalidate_path(
                volume.path(),
                &escape,
                InvalidationScope::PathAndDescendants,
            )
            .unwrap_err();

        assert!(matches!(error, DriverError::PathOutsideVolume(_)));
    }

    #[test]
    fn invalidation_allows_missing_child_inside_volume_root() {
        let volume = tempdir().unwrap();
        let driver = LocalHdDriver::new();
        let missing_child = volume.path().join("missing");

        let receipt = driver
            .invalidate_path(
                volume.path(),
                &missing_child,
                InvalidationScope::PathAndDescendants,
            )
            .unwrap();

        assert!(receipt.invalidated_paths.is_empty());
    }
}
