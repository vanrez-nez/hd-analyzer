use std::path::PathBuf;

use hd_analyzer_core::{
    DeleteSafetyClassification, DeleteSafetyFlag, DirectoryListing, DirectoryOpenProgress,
    DirectoryProgressUpdate, DriverEvent, EntryKind, InvalidationReceipt, InvalidationScope,
    LiveUpdateConfig, NodeState, PathDeleteSafety, PathNode, ProgressSnapshot, ScanConfig,
    ScanIssue, SizeMeasurementMode, StartScanReceipt, Volume,
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
    pub storage_kind: String,
    pub is_removable: bool,
    pub is_read_only: bool,
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
            storage_kind: volume.storage_kind.clone(),
            is_removable: volume.is_removable,
            is_read_only: volume.is_read_only,
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
    pub size_measurement_mode: Option<SizeMeasurementMode>,
    pub live_updates: Option<LiveUpdateConfig>,
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
            size_measurement_mode: dto
                .size_measurement_mode
                .unwrap_or(default.size_measurement_mode),
            live_updates: dto.live_updates.unwrap_or(default.live_updates),
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
            size_measurement_mode: Some(config.size_measurement_mode),
            live_updates: Some(config.live_updates),
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PathDeleteSafetyDto {
    pub classification: DeleteSafetyClassification,
    pub can_delete_now: bool,
    pub flags: Vec<DeleteSafetyFlag>,
    pub reason: String,
}

impl From<&PathDeleteSafety> for PathDeleteSafetyDto {
    fn from(safety: &PathDeleteSafety) -> Self {
        Self {
            classification: safety.classification,
            can_delete_now: safety.can_delete_now,
            flags: safety.flags.clone(),
            reason: safety.reason.clone(),
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
    pub delete_safety: PathDeleteSafetyDto,
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
            delete_safety: PathDeleteSafetyDto::from(&node.delete_safety),
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
pub struct FsOpenProgressEventDto {
    pub path: String,
    pub entries_processed: u64,
}

impl FsOpenProgressEventDto {
    pub fn new(path: String, progress: DirectoryOpenProgress) -> Self {
        Self {
            path,
            entries_processed: progress.entries_processed,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryProgressUpdateDto {
    pub path: String,
    pub size: u64,
    pub logical_size: u64,
    pub state: NodeState,
}

impl From<&DirectoryProgressUpdate> for DirectoryProgressUpdateDto {
    fn from(update: &DirectoryProgressUpdate) -> Self {
        Self {
            path: display_path(&update.path),
            size: update.size,
            logical_size: update.logical_size,
            state: update.state,
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
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
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
    DirectoryProgress {
        job_id: String,
        request_id: String,
        path: String,
        updates: Vec<DirectoryProgressUpdateDto>,
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
            DriverEvent::DirectoryProgress {
                job_id,
                request_id,
                path,
                updates,
                snapshot,
            } => Self::DirectoryProgress {
                job_id,
                request_id,
                path: display_path(&path),
                updates: updates
                    .iter()
                    .map(DirectoryProgressUpdateDto::from)
                    .collect(),
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
#[serde(rename_all = "camelCase")]
pub struct DeleteReceiptDto {
    pub deleted_paths: Vec<String>,
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
    InvalidRoot,
    ScanStartFailed,
    PathOutsideRoot,
    PathOutsideVolume,
    DriverUnavailable,
    JobNotFound,
    OpenItemFailed,
    DeleteFailed,
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
    fn maps_volume_to_camel_case_dto_shape() {
        let volume = Volume {
            id: "/data".to_string(),
            display_name: "Data".to_string(),
            root_path: PathBuf::from("/data"),
            mount_point: PathBuf::from("/data"),
            total_bytes: 100,
            available_bytes: 40,
            filesystem: "apfs".to_string(),
            storage_kind: "SSD".to_string(),
            is_removable: false,
            is_read_only: true,
            is_accessible: true,
            access_issue: None,
        };

        let dto = DriveDto::from(&volume);

        assert_eq!(dto.id, "/data");
        assert_eq!(dto.used_space, 60);
        assert_eq!(dto.storage_kind, "SSD");
        assert!(dto.is_read_only);
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
        assert!(json.contains("jobId"));
        assert!(json.contains("requestId"));
        assert!(!json.contains("job_id"));
    }

    #[test]
    fn supplied_scan_config_can_disable_min_visible_folder_filter() {
        let dto: ScanConfigDto = serde_json::from_value(serde_json::json!({
            "requestedDepth": 2,
            "preloadDepth": 1,
            "showHidden": false,
            "expandAboveBytes": 1_000_000_000u64,
            "stayOnFilesystem": true,
            "followSymlinks": false,
            "sizeMeasurementMode": "logicalOnly"
        }))
        .unwrap();

        let config = ScanConfig::from(dto);

        assert_eq!(config.min_visible_folder_bytes, None);
        assert_eq!(
            config.size_measurement_mode,
            SizeMeasurementMode::LogicalOnly
        );
    }
}
