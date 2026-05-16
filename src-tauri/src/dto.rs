use std::collections::HashMap;
use std::path::{Path, PathBuf};

use hd_analyzer_core::{
    CategoryUsage, Drive, FileKind, ReadError, ScanProgress, ScanResult, paths,
};
use serde::Serialize;

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
}
