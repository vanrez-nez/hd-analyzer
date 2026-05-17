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
    let path = path.canonicalize()?;
    let mut dedupe = HardLinkDedupe::new();
    let root_device = if config.stay_on_filesystem {
        get_device_id(&path)
    } else {
        None
    };
    discover_directory_precise_inner(&path, config, &mut dedupe, root_device)
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

#[derive(Debug, Default)]
struct DirectoryTreeMeasurement {
    allocated_bytes: u64,
    logical_bytes: u64,
    has_visible_children: bool,
    issues: Vec<ScanIssue>,
    summaries: Vec<DirectorySizeSummary>,
}

fn discover_directory_precise_inner(
    path: &Path,
    config: &ScanConfig,
    dedupe: &mut HardLinkDedupe,
    root_device: Option<u64>,
) -> std::io::Result<PreciseDirectoryScan> {
    let mut children = Vec::new();
    let mut issues = Vec::new();
    let mut summaries = Vec::new();
    let mut total_visible_size = 0u64;
    let mut total_measured_size = 0u64;
    let mut total_logical_size = 0u64;
    let mut has_more_depth = false;

    for entry_result in direct_entries(path, config) {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                issues.push(issue_from_walk_error(path, error));
                continue;
            }
        };

        let entry_path = entry.path();
        let name = entry.file_name.to_string_lossy().into_owned();
        let is_visible_by_config = !config.is_hidden_name(&name);

        if entry.path_is_symlink() && !config.follow_symlinks {
            match std::fs::symlink_metadata(&entry_path) {
                Ok(metadata) => {
                    let measurement = dedupe.measure_file_with_policy(
                        &entry_path,
                        &metadata,
                        config.dedupe_hard_links,
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
                            depth_from_request: 1,
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

        let file_type = entry.file_type;
        if file_type.is_dir() {
            if crosses_filesystem_boundary(&entry, root_device) {
                if is_visible_by_config {
                    issues.push(ScanIssue {
                        path: entry_path,
                        kind: ReadIssueKind::FilesystemBoundary,
                        message: "Directory is on a different filesystem.".to_string(),
                    });
                }
                continue;
            }

            let measurement = measure_directory_tree(&entry_path, config, dedupe, root_device);
            total_measured_size = total_measured_size.saturating_add(measurement.allocated_bytes);
            total_logical_size = total_logical_size.saturating_add(measurement.logical_bytes);
            issues.extend(measurement.issues.clone());
            summaries.extend(measurement.summaries.clone());

            if is_visible_by_config && config.should_show_folder(measurement.allocated_bytes) {
                total_visible_size = total_visible_size.saturating_add(measurement.allocated_bytes);
                has_more_depth |= measurement.has_visible_children;
                children.push(PathNode {
                    path: entry_path.clone(),
                    name,
                    kind: EntryKind::Directory,
                    parent_path: Some(path.to_path_buf()),
                    depth_from_request: 1,
                    size: measurement.allocated_bytes,
                    logical_size: measurement.logical_bytes,
                    state: NodeState::Complete,
                    visible: true,
                    children_known: !measurement.has_visible_children,
                    active_job_id: None,
                    delete_safety: classify_path_safety(&entry_path),
                    issues: measurement.issues,
                });
            }
        } else if file_type.is_file() {
            match entry.metadata() {
                Ok(metadata) => {
                    let measurement = dedupe.measure_file_with_policy(
                        &entry_path,
                        &metadata,
                        config.dedupe_hard_links,
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
                            depth_from_request: 1,
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

    let listing = DirectoryListing {
        path: path.to_path_buf(),
        config_fingerprint: config.fingerprint(),
        children,
        total_visible_size,
        total_measured_size,
        total_logical_size,
        state: NodeState::Complete,
        loaded_depth: 1.min(config.requested_depth),
        has_more_depth,
        issues,
        generation: 0,
    };

    Ok(PreciseDirectoryScan { listing, summaries })
}

fn measure_directory_tree(
    path: &Path,
    config: &ScanConfig,
    dedupe: &mut HardLinkDedupe,
    root_device: Option<u64>,
) -> DirectoryTreeMeasurement {
    let mut measurement = DirectoryTreeMeasurement::default();

    for entry_result in direct_entries(path, config) {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                measurement.issues.push(issue_from_walk_error(path, error));
                continue;
            }
        };

        let entry_path = entry.path();
        let name = entry.file_name.to_string_lossy().into_owned();
        let is_visible_by_config = !config.is_hidden_name(&name);

        if entry.path_is_symlink() && !config.follow_symlinks {
            match std::fs::symlink_metadata(&entry_path) {
                Ok(metadata) => {
                    if is_visible_by_config {
                        measurement.has_visible_children = true;
                    }
                    let file = dedupe.measure_file_with_policy(
                        &entry_path,
                        &metadata,
                        config.dedupe_hard_links,
                    );
                    measurement.allocated_bytes = measurement
                        .allocated_bytes
                        .saturating_add(file.allocated_bytes);
                    measurement.logical_bytes =
                        measurement.logical_bytes.saturating_add(file.logical_bytes);
                }
                Err(error) => measurement.issues.push(ScanIssue {
                    path: entry_path,
                    kind: ReadIssueKind::MetadataFailed,
                    message: error.to_string(),
                }),
            }
            continue;
        }

        let file_type = entry.file_type;
        if file_type.is_dir() {
            if crosses_filesystem_boundary(&entry, root_device) {
                if is_visible_by_config {
                    measurement.issues.push(ScanIssue {
                        path: entry_path,
                        kind: ReadIssueKind::FilesystemBoundary,
                        message: "Directory is on a different filesystem.".to_string(),
                    });
                }
                continue;
            }

            if is_visible_by_config {
                measurement.has_visible_children = true;
            }
            let child = measure_directory_tree(&entry_path, config, dedupe, root_device);
            measurement.allocated_bytes = measurement
                .allocated_bytes
                .saturating_add(child.allocated_bytes);
            measurement.logical_bytes = measurement
                .logical_bytes
                .saturating_add(child.logical_bytes);
            measurement.issues.extend(child.issues);
            measurement.summaries.extend(child.summaries);
        } else if file_type.is_file() {
            match entry.metadata() {
                Ok(metadata) => {
                    if is_visible_by_config {
                        measurement.has_visible_children = true;
                    }
                    let file = dedupe.measure_file_with_policy(
                        &entry_path,
                        &metadata,
                        config.dedupe_hard_links,
                    );
                    measurement.allocated_bytes = measurement
                        .allocated_bytes
                        .saturating_add(file.allocated_bytes);
                    measurement.logical_bytes =
                        measurement.logical_bytes.saturating_add(file.logical_bytes);
                }
                Err(error) => measurement.issues.push(ScanIssue {
                    path: entry_path,
                    kind: ReadIssueKind::MetadataFailed,
                    message: error.to_string(),
                }),
            }
        }
    }

    measurement.summaries.push(DirectorySizeSummary {
        path: path.to_path_buf(),
        config_fingerprint: config.fingerprint(),
        allocated_size: measurement.allocated_bytes,
        logical_size: measurement.logical_bytes,
        has_visible_children: measurement.has_visible_children,
        issues: measurement.issues.clone(),
    });

    measurement
}

fn direct_entries(
    path: &Path,
    config: &ScanConfig,
) -> impl Iterator<Item = Result<jwalk::DirEntry<((), ())>, jwalk::Error>> {
    jwalk::WalkDir::new(path)
        .min_depth(1)
        .max_depth(1)
        .skip_hidden(false)
        .follow_links(config.follow_symlinks)
        .into_iter()
}

fn issue_from_walk_error(path: &Path, error: jwalk::Error) -> ScanIssue {
    ScanIssue {
        path: error
            .path()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf()),
        kind: if error
            .io_error()
            .is_some_and(|io_error| io_error.kind() == std::io::ErrorKind::PermissionDenied)
        {
            ReadIssueKind::PermissionDenied
        } else {
            ReadIssueKind::MetadataFailed
        },
        message: error.to_string(),
    }
}

fn crosses_filesystem_boundary(
    entry: &jwalk::DirEntry<((), ())>,
    root_device: Option<u64>,
) -> bool {
    let Some(root_device) = root_device else {
        return false;
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        entry
            .metadata()
            .is_ok_and(|metadata| metadata.dev() != root_device)
    }

    #[cfg(not(unix))]
    {
        let _ = entry;
        false
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

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
}
