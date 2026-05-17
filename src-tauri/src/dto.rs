use std::collections::HashMap;
use std::path::{Path, PathBuf};

use hd_analyzer_core::{
    CategoryUsage, DirectoryListing, Drive, DriverEvent, EntryKind, FileKind, InvalidationReceipt,
    InvalidationScope, NodeState, PathNode, ProgressSnapshot, ReadError, ScanConfig, ScanIssue,
    ScanProgress, ScanResult, StartScanReceipt, Volume, paths,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DriveDto {
    pub id: String,
    pub label: String,
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
    pub used_space: u64,
    pub file_system: String,
}

impl From<&Drive> for DriveDto {
    fn from(drive: &Drive) -> Self {
        let mount_point = drive.mount_point.display().to_string();
        Self {
            id: mount_point.clone(),
            label: drive.label.clone(),
            mount_point,
            total_space: drive.total_space,
            available_space: drive.available_space,
            used_space: drive.used_space(),
            file_system: drive.file_system.clone(),
        }
    }
}

impl From<&Volume> for DriveDto {
    fn from(volume: &Volume) -> Self {
        Self {
            id: volume.id.clone(),
            label: volume.display_name.clone(),
            mount_point: volume.mount_point.display().to_string(),
            total_space: volume.total_bytes,
            available_space: volume.available_bytes,
            used_space: volume.total_bytes.saturating_sub(volume.available_bytes),
            file_system: volume.filesystem.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanConfigDto {
    pub requested_depth: Option<usize>,
    pub preload_depth: Option<usize>,
    pub show_hidden: Option<bool>,
    pub expand_above_bytes: Option<u64>,
    pub min_visible_folder_bytes: Option<u64>,
    pub stay_on_filesystem: Option<bool>,
    pub follow_symlinks: Option<bool>,
    pub dedupe_hard_links: Option<bool>,
}

impl From<ScanConfigDto> for ScanConfig {
    fn from(dto: ScanConfigDto) -> Self {
        let default = ScanConfig::default();
        ScanConfig {
            requested_depth: dto.requested_depth.unwrap_or(default.requested_depth),
            preload_depth: dto.preload_depth.unwrap_or(default.preload_depth),
            show_hidden: dto.show_hidden.unwrap_or(default.show_hidden),
            expand_above_bytes: dto.expand_above_bytes.or(default.expand_above_bytes),
            min_visible_folder_bytes: dto.min_visible_folder_bytes,
            stay_on_filesystem: dto.stay_on_filesystem.unwrap_or(default.stay_on_filesystem),
            follow_symlinks: dto.follow_symlinks.unwrap_or(default.follow_symlinks),
            dedupe_hard_links: dto.dedupe_hard_links.unwrap_or(default.dedupe_hard_links),
        }
        .normalized()
    }
}

impl Default for ScanConfigDto {
    fn default() -> Self {
        let config = ScanConfig::default();
        Self {
            requested_depth: Some(config.requested_depth),
            preload_depth: Some(config.preload_depth),
            show_hidden: Some(config.show_hidden),
            expand_above_bytes: config.expand_above_bytes,
            min_visible_folder_bytes: config.min_visible_folder_bytes,
            stay_on_filesystem: Some(config.stay_on_filesystem),
            follow_symlinks: Some(config.follow_symlinks),
            dedupe_hard_links: Some(config.dedupe_hard_links),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanIssueDto {
    pub path: String,
    pub kind: String,
    pub message: String,
}

impl From<&ScanIssue> for ScanIssueDto {
    fn from(issue: &ScanIssue) -> Self {
        Self {
            path: issue.path.display().to_string(),
            kind: format!("{:?}", issue.kind),
            message: issue.message.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PathNodeDto {
    pub path: String,
    pub name: String,
    pub kind: EntryKind,
    pub parent_path: Option<String>,
    pub depth_from_request: usize,
    pub size: u64,
    pub logical_size: u64,
    pub state: NodeState,
    pub visible: bool,
    pub children_known: bool,
    pub active_job_id: Option<String>,
    pub issues: Vec<ScanIssueDto>,
}

impl From<&PathNode> for PathNodeDto {
    fn from(node: &PathNode) -> Self {
        Self {
            path: node.path.display().to_string(),
            name: node.name.clone(),
            kind: node.kind,
            parent_path: node.parent_path.as_ref().map(display_path),
            depth_from_request: node.depth_from_request,
            size: node.size,
            logical_size: node.logical_size,
            state: node.state,
            visible: node.visible,
            children_known: node.children_known,
            active_job_id: node.active_job_id.clone(),
            issues: node.issues.iter().map(ScanIssueDto::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListingDto {
    pub path: String,
    pub config_fingerprint: String,
    pub children: Vec<PathNodeDto>,
    pub total_visible_size: u64,
    pub total_measured_size: u64,
    pub total_logical_size: u64,
    pub state: NodeState,
    pub loaded_depth: usize,
    pub has_more_depth: bool,
    pub issues: Vec<ScanIssueDto>,
    pub generation: u64,
}

impl From<&DirectoryListing> for DirectoryListingDto {
    fn from(listing: &DirectoryListing) -> Self {
        Self {
            path: listing.path.display().to_string(),
            config_fingerprint: listing.config_fingerprint.clone(),
            children: listing.children.iter().map(PathNodeDto::from).collect(),
            total_visible_size: listing.total_visible_size,
            total_measured_size: listing.total_measured_size,
            total_logical_size: listing.total_logical_size,
            state: listing.state,
            loaded_depth: listing.loaded_depth,
            has_more_depth: listing.has_more_depth,
            issues: listing.issues.iter().map(ScanIssueDto::from).collect(),
            generation: listing.generation,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartScanReceiptDto {
    pub job_id: String,
    pub request_id: String,
}

impl From<StartScanReceipt> for StartScanReceiptDto {
    fn from(receipt: StartScanReceipt) -> Self {
        Self {
            job_id: receipt.job_id,
            request_id: receipt.request_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvalidationReceiptDto {
    pub invalidated_paths: Vec<String>,
    pub superseded_job_ids: Vec<String>,
}

impl From<InvalidationReceipt> for InvalidationReceiptDto {
    fn from(receipt: InvalidationReceipt) -> Self {
        Self {
            invalidated_paths: receipt.invalidated_paths.iter().map(display_path).collect(),
            superseded_job_ids: receipt.superseded_job_ids,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvalidationScopeDto {
    PathOnly,
    PathAndDescendants,
}

impl From<InvalidationScopeDto> for InvalidationScope {
    fn from(scope: InvalidationScopeDto) -> Self {
        match scope {
            InvalidationScopeDto::PathOnly => InvalidationScope::PathOnly,
            InvalidationScopeDto::PathAndDescendants => InvalidationScope::PathAndDescendants,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum FsProgressEventDto {
    JobQueued {
        job_id: String,
        request_id: String,
        path: String,
    },
    JobStarted {
        job_id: String,
        request_id: String,
        path: String,
    },
    DirectoryReady {
        job_id: String,
        request_id: String,
        path: String,
        listing: DirectoryListingDto,
    },
    ProgressSnapshot {
        job_id: String,
        request_id: String,
        snapshot: ProgressSnapshot,
    },
    JobFinished {
        job_id: String,
        request_id: String,
        path: String,
    },
    JobFailed {
        job_id: String,
        request_id: String,
        path: String,
        message: String,
    },
    PathInvalidated {
        path: String,
        scope: InvalidationScope,
        generation: u64,
    },
}

impl From<DriverEvent> for FsProgressEventDto {
    fn from(event: DriverEvent) -> Self {
        match event {
            DriverEvent::JobQueued {
                job_id,
                request_id,
                path,
            } => Self::JobQueued {
                job_id,
                request_id,
                path: display_path(&path),
            },
            DriverEvent::JobStarted {
                job_id,
                request_id,
                path,
            } => Self::JobStarted {
                job_id,
                request_id,
                path: display_path(&path),
            },
            DriverEvent::DirectoryReady {
                job_id,
                request_id,
                path,
                listing,
            } => Self::DirectoryReady {
                job_id,
                request_id,
                path: display_path(&path),
                listing: DirectoryListingDto::from(&listing),
            },
            DriverEvent::ProgressSnapshot {
                job_id,
                request_id,
                snapshot,
            } => Self::ProgressSnapshot {
                job_id,
                request_id,
                snapshot,
            },
            DriverEvent::JobFinished {
                job_id,
                request_id,
                path,
            } => Self::JobFinished {
                job_id,
                request_id,
                path: display_path(&path),
            },
            DriverEvent::JobFailed {
                job_id,
                request_id,
                path,
                message,
            } => Self::JobFailed {
                job_id,
                request_id,
                path: display_path(&path),
                message,
            },
            DriverEvent::PathInvalidated {
                path,
                scope,
                generation,
            } => Self::PathInvalidated {
                path: display_path(&path),
                scope,
                generation,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatusDto {
    Idle,
    Scanning,
    Complete,
    Failed,
    PartialRescan,
    CancelRequested,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanSessionDto {
    pub session_id: String,
    pub root: String,
    pub status: SessionStatusDto,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressDto {
    pub session_id: String,
    pub root: String,
    pub bytes_scanned: u64,
    pub files_scanned: u64,
    pub directories_scanned: u64,
    pub directory_sizes: HashMap<String, u64>,
    pub categories: Vec<CategoryUsageDto>,
}

impl ScanProgressDto {
    pub fn from_progress(session_id: &str, root: &Path, progress: &ScanProgress) -> Self {
        Self {
            session_id: session_id.to_string(),
            root: root.display().to_string(),
            bytes_scanned: progress.bytes_scanned,
            files_scanned: progress.files_scanned,
            directories_scanned: progress.directories_scanned,
            directory_sizes: HashMap::new(),
            categories: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanResultDto {
    pub root: String,
    pub bytes_scanned: u64,
    pub files_scanned: u64,
    pub directories_scanned: u64,
    pub directory_sizes: HashMap<String, u64>,
    pub categories: Vec<CategoryUsageDto>,
    pub read_errors: Vec<ReadErrorDto>,
    pub hidden_unscanned_bytes: Option<u64>,
}

impl ScanResultDto {
    pub fn from_result(result: &ScanResult, hidden_unscanned_bytes: Option<u64>) -> Self {
        Self {
            root: result.root.display().to_string(),
            bytes_scanned: result.bytes_scanned,
            files_scanned: result.files_scanned,
            directories_scanned: result.directories_scanned,
            directory_sizes: result
                .directory_sizes
                .iter()
                .map(|(path, size)| (path.display().to_string(), *size))
                .collect(),
            categories: result
                .categories
                .iter()
                .map(|entry| CategoryUsageDto::from_usage(entry, result.bytes_scanned))
                .collect(),
            read_errors: result
                .read_errors
                .iter()
                .map(|error| ReadErrorDto::from_error(error, &result.root))
                .collect(),
            hidden_unscanned_bytes,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntryDto {
    pub path: String,
    pub display_name: String,
    pub size: u64,
    pub share: f64,
    pub is_directory: bool,
    pub is_virtual: bool,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CategoryUsageDto {
    pub kind: FileKind,
    pub label: String,
    pub size: u64,
    pub share: f64,
}

impl CategoryUsageDto {
    pub fn from_usage(entry: &CategoryUsage, total: u64) -> Self {
        let share = if total == 0 {
            0.0
        } else {
            entry.size as f64 / total as f64
        };
        Self {
            kind: entry.kind,
            label: entry.kind.label().to_string(),
            size: entry.size,
            share,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadErrorDto {
    pub path: String,
    pub display_path: String,
    pub error: String,
}

impl ReadErrorDto {
    pub fn from_error(error: &ReadError, root: &Path) -> Self {
        Self {
            path: error.path.display().to_string(),
            display_path: paths::compact_path(&error.path, root),
            error: error.error.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionCheckDto {
    pub granted: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandErrorCode {
    NoDrives,
    DriveDiscoveryFailed,
    InvalidRoot,
    ScanAlreadyRunning,
    ScanStartFailed,
    UnknownSession,
    PathOutsideRoot,
    PathOutsideVolume,
    DriverUnavailable,
    JobNotFound,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: CommandErrorCode,
    pub message: String,
}

impl CommandError {
    pub fn new(code: CommandErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub fn display_path(path: &PathBuf) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn maps_drive_to_camel_case_dto_shape() {
        let drive = Drive {
            label: "Data".to_string(),
            mount_point: PathBuf::from("/data"),
            total_space: 100,
            available_space: 40,
            file_system: "apfs".to_string(),
        };

        let dto = DriveDto::from(&drive);

        assert_eq!(dto.id, "/data");
        assert_eq!(dto.used_space, 60);
    }

    #[test]
    fn serializes_progress_event_with_tagged_shape() {
        let event = FsProgressEventDto::JobQueued {
            job_id: "job".to_string(),
            request_id: "req".to_string(),
            path: "/tmp".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("jobQueued"));
    }

    #[test]
    fn supplied_scan_config_can_disable_min_visible_folder_filter() {
        let dto: ScanConfigDto = serde_json::from_value(serde_json::json!({
            "requestedDepth": 2,
            "preloadDepth": 1,
            "showHidden": false,
            "expandAboveBytes": 1_000_000_000u64,
            "stayOnFilesystem": true,
            "followSymlinks": false
        }))
        .unwrap();

        let config = ScanConfig::from(dto);

        assert_eq!(config.min_visible_folder_bytes, None);
    }
}
