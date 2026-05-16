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
    fn skips_symlinked_files() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.txt");
        std::fs::write(&target, b"abc").unwrap();
        symlink(&target, dir.path().join("link.txt")).unwrap();

        let (tx, _rx) = mpsc::channel();
        let result = scan_drive(dir.path(), Instant::now(), &tx, false).unwrap();

        assert_eq!(result.files_scanned, 1);
    }
}
