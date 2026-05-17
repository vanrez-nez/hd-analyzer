use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub id: String,
    pub display_name: String,
    pub root_path: PathBuf,
    pub mount_point: PathBuf,
    pub filesystem: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub is_removable: bool,
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
            total_bytes: drive.total_space,
            available_bytes: drive.available_space,
            is_removable: false,
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
    pub state: NodeState,
    pub loaded_depth: usize,
    pub has_more_depth: bool,
    pub issues: Vec<ScanIssue>,
    pub generation: u64,
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
}

impl LocalHdDriver {
    pub fn new() -> Self {
        Self::default()
    }

    fn discover_and_cache(
        &self,
        path: &Path,
        config: &ScanConfig,
    ) -> DriverResult<DirectoryListing> {
        let listing =
            crate::scan::discover_directory(path, config).map_err(|source| DriverError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        let entry = self
            .cache
            .lock()
            .expect("directory cache poisoned")
            .upsert(listing, CacheFreshness::Fresh);
        Ok(entry.listing)
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
        let config = config.clone().normalized();
        let scoped_path = self.scoped_existing_path(volume_root, path)?;
        let fingerprint = config.fingerprint();
        if let Some(entry) = self
            .cache
            .lock()
            .expect("directory cache poisoned")
            .get(&scoped_path, &fingerprint)
        {
            if entry.freshness != CacheFreshness::Stale {
                return Ok(entry.listing);
            }
        }
        self.discover_and_cache(&scoped_path, &config)
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
        let handle = self.jobs.create(scoped_path.clone());
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
            driver.jobs.set_state(&handle.job_id, JobState::Running);
            if let Some(sink) = &sink_for_worker {
                sink(DriverEvent::JobStarted {
                    job_id: handle.job_id.clone(),
                    request_id: handle.request_id.clone(),
                    path: handle.root_path.clone(),
                });
            }

            let result = driver.discover_and_cache(&handle.root_path, &config);
            match result {
                Ok(listing) => {
                    driver.jobs.set_state(&handle.job_id, JobState::Completed);
                    if let Some(sink) = &sink_for_worker {
                        let mut progress =
                            ProgressSnapshot::new(handle.job_id.clone(), handle.request_id.clone());
                        progress.state = JobState::Completed;
                        progress.scheduled_units = listing.children.len() as u64;
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
                    }
                }
                Err(error) => {
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
