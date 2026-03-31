use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use humansize::{DECIMAL, format_size};
use rayon::prelude::*;
use sysinfo::Disks;


#[derive(Debug, Clone)]
pub struct Drive {
    pub label: String,
    pub mount_point: PathBuf,
    pub total_space: u64,
    pub available_space: u64,
    pub file_system: String,
}

impl Drive {
    pub fn used_space(&self) -> u64 {
        self.total_space.saturating_sub(self.available_space)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
        self.completed_at.unwrap_or_else(Instant::now).saturating_duration_since(self.started_at)
    }
}

#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub bytes_scanned: u64,
    pub files_scanned: u64,
    pub directories_scanned: u64,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    SelectDrive,
    Results,
    ErrorLog,
}

pub struct App {
    pub drives: Vec<Drive>,
    pub selected_drive: usize,
    pub screen: Screen,
    pub progress: Option<ScanProgress>,
    pub result: Option<ScanResult>,
    pub result_path: Option<PathBuf>,
    pub current_subdirs: Option<Vec<PathBuf>>,
    pub selected_result_entry: usize,
    pub error_scroll: usize,
    pub rescan_focused_path: Option<PathBuf>,
    pub scanner: Option<mpsc::Receiver<ScanUpdate>>,
    pub should_quit: bool,
    pub error_message: Option<String>,
}

impl App {
    pub fn new() -> Result<Self> {
        let drives = list_drives()?;
        Ok(Self {
            drives,
            selected_drive: 0,
            screen: Screen::SelectDrive,
            progress: None,
            result: None,
            result_path: None,
            current_subdirs: None,
            selected_result_entry: 0,
            error_scroll: 0,
            rescan_focused_path: None,
            scanner: None,
            should_quit: false,
            error_message: None,
        })
    }

    pub fn next_drive(&mut self) {
        if !self.drives.is_empty() {
            self.selected_drive = (self.selected_drive + 1) % self.drives.len();
        }
    }

    pub fn previous_drive(&mut self) {
        if !self.drives.is_empty() {
            if self.selected_drive == 0 {
                self.selected_drive = self.drives.len() - 1;
            } else {
                self.selected_drive -= 1;
            }
        }
    }

    pub fn selected_drive(&self) -> Option<&Drive> {
        self.drives.get(self.selected_drive)
    }

    pub fn start_scan(&mut self) {
        let Some(drive) = self.selected_drive().cloned() else {
            self.error_message = Some("No drives were detected on this machine.".to_string());
            return;
        };

        let root = drive.mount_point.clone();

        if let Some(existing) = &self.result {
            if existing.root == root {
                self.screen = Screen::Results;
                return;
            }
        }

        let scan_root = root.clone();
        let started_at = Instant::now();
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let outcome = scan_drive(&scan_root, started_at, &tx, false);
            let _ = tx.send(ScanUpdate::Finished(outcome));
        });

        self.progress = Some(ScanProgress {
            bytes_scanned: 0,
            files_scanned: 0,
            directories_scanned: 0,
        });

        let partial_result = ScanResult {
            root: root.clone(),
            started_at,
            completed_at: None,
            bytes_scanned: 0,
            files_scanned: 0,
            directories_scanned: 0,
            directory_sizes: HashMap::new(),
            categories: Vec::new(),
            read_errors: Vec::new(),
        };

        self.result = Some(partial_result);
        self.result_path = Some(root.clone());
        self.current_subdirs = Some(load_current_subdirectories(&root));
        self.selected_result_entry = 0;
        self.error_message = None;
        self.scanner = Some(rx);
        self.screen = Screen::Results;
    }

    pub fn rescan(&mut self) {
        let entries = self.current_directory_entries();
        if self.selected_result_entry >= entries.len() { return; }
        
        let entry = &entries[self.selected_result_entry];
        let path_to_scan = entry.path.clone();

        self.rescan_focused_path = Some(path_to_scan.clone());
        
        let started_at = Instant::now();
        let (tx, rx) = mpsc::channel();
        
        thread::spawn(move || {
            let outcome = scan_drive(&path_to_scan, started_at, &tx, true);
            let _ = tx.send(ScanUpdate::PartialFinished(outcome));
        });
        
        self.scanner = Some(rx);
    }

    pub fn back_to_selection(&mut self) -> Result<()> {
        self.drives = list_drives()?;
        self.selected_drive = self.selected_drive.min(self.drives.len().saturating_sub(1));
        self.error_scroll = 0;
        self.error_message = None;
        self.screen = Screen::SelectDrive;
        Ok(())
    }

    pub fn poll_scanner(&mut self) {
        let Some(receiver) = self.scanner.take() else {
            return;
        };

        let mut keep_receiver = true;
        let mut latest_snapshot = None;
        let mut finished = None;
        let mut is_partial_finished = false;

        while let Ok(update) = receiver.try_recv() {
            match update {
                ScanUpdate::Snapshot { progress, directory_sizes, categories_map } => {
                    latest_snapshot = Some((progress, directory_sizes, categories_map));
                }
                ScanUpdate::Finished(scan_result) => {
                    finished = Some(scan_result);
                    keep_receiver = false;
                }
                ScanUpdate::PartialFinished(scan_result) => {
                    finished = Some(scan_result);
                    keep_receiver = false;
                    is_partial_finished = true;
                }
            }
        }

        if let Some((progress, directory_sizes, categories_map)) = latest_snapshot {
            self.progress = Some(progress.clone());
            if let Some(result) = &mut self.result {
                let categories = collect_categories(categories_map);
                result.bytes_scanned = progress.bytes_scanned;
                result.files_scanned = progress.files_scanned;
                result.directories_scanned = progress.directories_scanned;
                result.directory_sizes = directory_sizes;
                result.categories = categories;
            }
        }

        if let Some(scan_result) = finished {
            match scan_result {
                Ok(mut final_result) => {
                    if is_partial_finished {
                        if let Some(existing_result) = &mut self.result {
                            let target_path = final_result.root.clone();
                            let old_size = existing_result.directory_sizes.get(&target_path).copied().unwrap_or(0);
                            let new_size = final_result.bytes_scanned;
                            let delta = new_size as i64 - old_size as i64;

                            existing_result.directory_sizes.retain(|k, _| !k.starts_with(&target_path));
                            existing_result.directory_sizes.extend(final_result.directory_sizes);

                            let mut current = target_path.parent();
                            while let Some(p) = current {
                                if p.starts_with(&existing_result.root) {
                                    let s = existing_result.directory_sizes.entry(p.to_path_buf()).or_insert(0);
                                    *s = (*s as i64 + delta).max(0) as u64;
                                }
                                current = p.parent();
                            }
                            existing_result.bytes_scanned = (existing_result.bytes_scanned as i64 + delta).max(0) as u64;
                        }
                    } else {
                        final_result.completed_at = Some(Instant::now());
                        if self.result_path.is_none() {
                            self.result_path = Some(final_result.root.clone());
                            self.current_subdirs = Some(load_current_subdirectories(&final_result.root));
                            self.selected_result_entry = 0;
                        }
                        self.result = Some(final_result);
                        self.progress = None;
                        self.screen = Screen::Results;
                    }
                }
                Err(error) => {
                    self.error_message = Some(error.to_string());
                    if !is_partial_finished {
                        self.screen = Screen::SelectDrive;
                        self.progress = None;
                    }
                }
            }
            
            if let Some(focused) = self.rescan_focused_path.take() {
                let entries = self.current_directory_entries();
                if let Some(pos) = entries.iter().position(|e| e.path == focused) {
                    self.selected_result_entry = pos;
                }
            }
        }

        if keep_receiver {
            self.scanner = Some(receiver);
        }
    }

    pub fn current_result_path(&self) -> Option<&Path> {
        self.result_path.as_deref()
    }

    pub fn current_directory_entries(&self) -> Vec<SizedEntry> {
        let Some(result) = &self.result else {
            return Vec::new();
        };
        let Some(subdirs) = &self.current_subdirs else {
            return Vec::new();
        };

        let mut entries: Vec<_> = subdirs
            .iter()
            .map(|path| SizedEntry {
                path: path.clone(),
                size: result.directory_sizes.get(path).copied().unwrap_or(0),
                is_dir: true,
            })
            .collect();
        entries.sort_by(|left, right| {
            right
                .size
                .cmp(&left.size)
                .then_with(|| left.path.cmp(&right.path))
        });

        // Add virtual Hidden Space row if we are at root and there is an un-scanned gap
        let is_root = self.result_path.as_ref() == Some(&result.root) || self.result_path.is_none();
        if is_root && !result.read_errors.is_empty() {
            if let Some(drive) = self.drives.iter().find(|d| d.mount_point == result.root) {
                let drive_used = drive.used_space();
                let total_size = result.total_bytes();
                if drive_used > total_size + 1_000_000 {
                    entries.push(SizedEntry {
                        path: PathBuf::from("HIDDEN_SPACE_VIRTUAL_NODE"),
                        size: drive_used - total_size,
                        is_dir: true,
                    });
                }
            }
        }

        entries
    }

    pub fn next_result_entry(&mut self) {
        let entries = self.current_directory_entries();
        if entries.is_empty() {
            self.selected_result_entry = 0;
            return;
        }
        self.selected_result_entry = (self.selected_result_entry + 1) % entries.len();
    }

    pub fn previous_result_entry(&mut self) {
        let entries = self.current_directory_entries();
        if entries.is_empty() {
            self.selected_result_entry = 0;
            return;
        }
        if self.selected_result_entry == 0 {
            self.selected_result_entry = entries.len() - 1;
        } else {
            self.selected_result_entry -= 1;
        }
    }

    pub fn enter_selected_directory(&mut self) {
        let entries = self.current_directory_entries();
        if self.selected_result_entry < entries.len() {
            let entry = &entries[self.selected_result_entry];
            
            if entry.path.to_str() == Some("HIDDEN_SPACE_VIRTUAL_NODE") {
                self.screen = Screen::ErrorLog;
                self.error_scroll = 0;
                return;
            }

            let subdirs = load_current_subdirectories(&entry.path);
            if subdirs.is_empty() {
                return; // Do not enter empty folders
            }
            self.result_path = Some(entry.path.clone());
            self.current_subdirs = Some(subdirs);
            self.selected_result_entry = 0;
        }
    }

    pub fn go_to_parent_directory(&mut self) {
        let Some(result) = &self.result else {
            return;
        };
        let Some(current_path) = self.result_path.as_ref() else {
            return;
        };
        if current_path == &result.root {
            return;
        }
        let parent = current_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| result.root.clone());
        self.result_path = Some(parent.clone());
        self.current_subdirs = Some(load_current_subdirectories(&parent));
        self.selected_result_entry = 0;
    }
}

pub fn list_drives() -> Result<Vec<Drive>> {
    let disks = Disks::new_with_refreshed_list();
    let mut seen = HashMap::<PathBuf, Drive>::new();

    for disk in disks.list() {
        let mount_point = disk.mount_point().to_path_buf();
        if disk.total_space() == 0 || mount_point.as_os_str().is_empty() {
            continue;
        }

        let file_system = disk.file_system().to_string_lossy().into_owned();
        let disk_name = disk.name().to_string_lossy();
        let label = if disk_name.is_empty() {
            mount_point.display().to_string()
        } else {
            format!("{disk_name} ({})", mount_point.display())
        };

        seen.entry(mount_point.clone()).or_insert(Drive {
            label,
            mount_point,
            total_space: disk.total_space(),
            available_space: disk.available_space(),
            file_system,
        });
    }

    let mut drives: Vec<_> = seen.into_values().collect();
    #[cfg(target_os = "macos")]
    {
        drives = collapse_macos_volume_group_duplicates(drives);
    }
    drives.sort_by(|left, right| left.mount_point.cmp(&right.mount_point));
    if drives.is_empty() {
        anyhow::bail!("No mounted drives found.");
    }
    Ok(drives)
}

fn load_current_subdirectories(path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let file_name = entry.file_name();
                    if let Some(name) = file_name.to_str() {
                        if name.starts_with('.') && name != "." && name != ".." {
                            continue; // Match earlier skipped hidden dirs logic over in scanner
                        }
                    }
                    dirs.push(entry.path());
                }
            }
        }
    }
    dirs
}

#[cfg(target_os = "macos")]
fn collapse_macos_volume_group_duplicates(drives: Vec<Drive>) -> Vec<Drive> {
    let data_mount = Path::new("/System/Volumes/Data");
    let root_mount = Path::new("/");

    let has_data_volume = drives.iter().any(|drive| drive.mount_point == data_mount);
    if !has_data_volume {
        return drives;
    }

    drives
        .into_iter()
        .filter(|drive| drive.mount_point != root_mount)
        .collect()
}

fn scan_drive(
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

    // Spawn the aggregator thread
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
                // Pre-populate directory so it shows up even if empty
                state.directory_sizes.entry(path.clone()).or_insert(0);
            }

            // Bubble up sizes
            if size > 0 {
                let mut current = path.as_path();
                while current != root_clone {
                    if let Some(parent) = current.parent() {
                        *state.directory_sizes.entry(parent.to_path_buf()).or_default() += size;
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

    // Recursive parallel walk
    walk_dir_parallel(root, &local_tx, &errors, root_dev);

    // Drop the sender so the aggregator thread knows we are done sending.
    drop(local_tx);

    let mut final_state = aggregator_thread.join().expect("Aggregator thread panicked");

    let categories = collect_categories(final_state.categories);
    if !final_state.directory_sizes.contains_key(root) {
        final_state.directory_sizes.insert(root.to_path_buf(), final_state.bytes_scanned);
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
        
        // Skip hidden files to maintain expected UI tidiness (Optional, but good practice for speed)
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with('.') && file_name != "." && file_name != ".." {
                // Ignore hidden files / dotfiles to speed it up and match Space behavior.
                return;
            }
        }

        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => return,
        };

        if file_type.is_symlink() {
            return; // Skip symlinks
        }

        if file_type.is_dir() {
            if let Some(r_dev) = root_dev {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(meta) = entry.metadata() {
                        if meta.dev() != r_dev {
                            return; // Crosses filesystem boundary, ignore!
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

fn collect_categories(categories: HashMap<FileKind, u64>) -> Vec<CategoryUsage> {
    let mut categories: Vec<_> = categories
        .into_iter()
        .map(|(kind, size)| CategoryUsage { kind, size })
        .collect();
    categories.sort_by(|left, right| {
        right
            .size
            .cmp(&left.size)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    categories
}

fn detect_file_kind(path: &Path) -> FileKind {
    let Some(extension) = normalized_extension(path) else {
        return if is_probably_executable(path) {
            FileKind::Apps
        } else {
            FileKind::Other
        };
    };

    match extension.as_str() {
        "mp4" | "mkv" | "mov" | "avi" | "m4v" | "webm" | "flv" | "mpg" | "mpeg" | "wmv" | "ts" => {
            FileKind::Video
        }
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" | "opus" | "alac" | "aiff" | "wma" => {
            FileKind::Audio
        }
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "heic" | "tif" | "tiff" | "raw"
        | "svg" | "psd" => FileKind::Images,
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "tbz" | "iso" | "dmg" => {
            FileKind::Archives
        }
        "app" | "pkg" | "exe" | "msi" | "appimage" | "dylib" | "dll" | "so" | "deb" | "rpm" => {
            FileKind::Apps
        }
        "pdf" | "doc" | "docx" | "ppt" | "pptx" | "xls" | "xlsx" | "pages" | "numbers" | "key"
        | "txt" | "rtf" | "md" | "odt" | "epub" => FileKind::Documents,
        "rs" | "go" | "py" | "js" | "tsx" | "jsx" | "java" | "kt" | "swift" | "c" | "cc"
        | "cpp" | "h" | "hpp" | "cs" | "php" | "rb" | "lua" | "toml" | "yaml" | "yml" | "json"
        | "xml" | "sql" | "sh" | "zsh" | "bash" => FileKind::Code,
        _ => FileKind::Other,
    }
}

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn get_device_id(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).ok().map(|m| m.dev())
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn is_probably_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            return metadata.permissions().mode() & 0o111 != 0;
        }
    }

    #[cfg(windows)]
    {
        let executable_exts = ["exe", "bat", "cmd", "com"];
        if let Some(ext) = normalized_extension(path) {
            return executable_exts.contains(&ext.as_str());
        }
    }

    false
}

fn file_disk_usage(metadata: &Metadata) -> u64 {
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

pub fn format_bytes(size: u64) -> String {
    format_size(size, DECIMAL)
}

pub fn format_ratio(size: u64, total: u64) -> String {
    if total == 0 {
        return "0.0%".to_string();
    }

    let ratio = size as f64 / total as f64 * 100.0;
    format!("{ratio:.1}%")
}

pub fn compact_path(path: &Path, root: &Path) -> String {
    if path == root {
        return root.display().to_string();
    }

    match path.strip_prefix(root) {
        Ok(stripped) => {
            let mut components = Vec::new();
            for component in stripped.components() {
                match component {
                    Component::Normal(value) => {
                        components.push(value.to_string_lossy().into_owned())
                    }
                    Component::RootDir => components.push(std::path::MAIN_SEPARATOR.to_string()),
                    _ => {}
                }
            }

            if components.is_empty() {
                root.display().to_string()
            } else {
                let prefix = root.display().to_string();
                format!(
                    "{prefix}{}{}",
                    std::path::MAIN_SEPARATOR,
                    components.join(std::path::MAIN_SEPARATOR_STR)
                )
            }
        }
        Err(_) => path.display().to_string(),
    }
}
