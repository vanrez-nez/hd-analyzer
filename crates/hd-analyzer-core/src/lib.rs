pub mod cache;
pub mod categories;
pub mod driver;
pub mod drives;
pub mod jobs;
pub mod paths;
pub mod rules;
pub mod safety;
pub mod scan;
pub mod size;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub use categories::{collect_categories, detect_file_kind};
pub use driver::{
    DeleteSafetyClassification, DeleteSafetyFlag, DirectoryListing, DriverError, DriverEvent,
    DriverResult, EntryKind, HdDriver, InvalidationReceipt, InvalidationScope, LocalHdDriver,
    NodeState, OpenPathRequest, PathDeleteSafety, PathNode, ReadIssueKind, ScanIssue,
    StartScanReceipt, StartScanRequest, Volume,
};
pub use drives::{Drive, list_drives};
pub use jobs::{JobState, ProgressSnapshot};
pub use paths::{compact_path, format_bytes, format_ratio};
pub use rules::ScanConfig;
pub use scan::{ScanUpdate, file_disk_usage, load_current_subdirectories, scan_drive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Video,
    Audio,
    Images,
    Archives,
    Apps,
    Documents,
    Code,
    Other,
}

impl FileKind {
    pub const ALL: [Self; 8] = [
        Self::Video,
        Self::Audio,
        Self::Images,
        Self::Archives,
        Self::Apps,
        Self::Documents,
        Self::Code,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Images => "Images",
            Self::Archives => "Archives",
            Self::Apps => "Apps",
            Self::Documents => "Docs",
            Self::Code => "Code",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SizedEntry {
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct CategoryUsage {
    pub kind: FileKind,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct ReadError {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub root: PathBuf,
    pub started_at: Instant,
    pub completed_at: Option<Instant>,
    pub bytes_scanned: u64,
    pub files_scanned: u64,
    pub directories_scanned: u64,
    pub directory_sizes: HashMap<PathBuf, u64>,
    pub categories: Vec<CategoryUsage>,
    pub read_errors: Vec<ReadError>,
}

impl ScanResult {
    pub fn total_bytes(&self) -> u64 {
        self.bytes_scanned
    }

    pub fn elapsed(&self) -> Duration {
        self.completed_at
            .unwrap_or_else(Instant::now)
            .saturating_duration_since(self.started_at)
    }
}

#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub bytes_scanned: u64,
    pub files_scanned: u64,
    pub directories_scanned: u64,
}
