use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::MutexGuard;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use hd_analyzer_core::{
    HdDriver, OpenPathRequest, ScanResult, ScanUpdate, StartScanRequest,
    list_drives as core_list_drives, scan_drive,
};
use tauri::{State, ipc::Channel};

use crate::dto::{
    CommandError, CommandErrorCode, DirectoryEntryDto, DirectoryListingDto, DriveDto,
    FsProgressEventDto, InvalidationReceiptDto, InvalidationScopeDto, PermissionCheckDto,
    ReadErrorDto, ScanConfigDto, ScanSessionDto, SessionStatusDto, StartScanReceiptDto,
};
use crate::state::{AppState, StoredSession};

#[tauri::command]
pub fn list_drives() -> Result<Vec<DriveDto>, CommandError> {
    log::info!("listing legacy drive scan roots");
    let drives = core_list_drives().map_err(|error| {
        log::warn!("legacy drive discovery failed: {error}");
        CommandError::new(CommandErrorCode::DriveDiscoveryFailed, error.to_string())
    })?;
    if drives.is_empty() {
        log::warn!("legacy drive discovery returned no mounted drives");
        return Err(CommandError::new(
            CommandErrorCode::NoDrives,
            "No mounted drives found.",
        ));
    }
    log::info!("legacy drive discovery returned {} drive(s)", drives.len());
    Ok(drives.iter().map(DriveDto::from).collect())
}

#[tauri::command]
pub fn fs_list_volumes(state: State<'_, AppState>) -> Result<Vec<DriveDto>, CommandError> {
    log::info!("listing filesystem explorer volumes");
    let volumes = state.fs_driver.list_volumes().map_err(|error| {
        log::warn!("filesystem explorer volume discovery failed: {error}");
        command_error(error)
    })?;
    if volumes.is_empty() {
        log::warn!("filesystem explorer volume discovery returned no mounted drives");
        return Err(CommandError::new(
            CommandErrorCode::NoDrives,
            "No mounted drives found.",
        ));
    }
    log::info!(
        "filesystem explorer volume discovery returned {} volume(s)",
        volumes.len()
    );
    Ok(volumes.iter().map(DriveDto::from).collect())
}

#[tauri::command]
pub fn fs_open_path(
    path: String,
    volume_root: String,
    config: Option<ScanConfigDto>,
    state: State<'_, AppState>,
) -> Result<DirectoryListingDto, CommandError> {
    let volume_root = PathBuf::from(volume_root);
    let path = PathBuf::from(path);
    log::info!(
        "opening filesystem path {} inside volume {}",
        path.display(),
        volume_root.display()
    );
    let request = OpenPathRequest {
        volume_root,
        path,
        config: config.unwrap_or_default().into(),
        request_id: chrono_like_timestamp(),
    };
    match state.fs_driver.open_path(request) {
        Ok(listing) => {
            log::info!(
                "opened filesystem path {} with {} child node(s), total measured size {}, total logical size {}",
                listing.path.display(),
                listing.children.len(),
                listing.total_measured_size,
                listing.total_logical_size
            );
            Ok(DirectoryListingDto::from(&listing))
        }
        Err(error) => {
            log::warn!("failed to open filesystem path: {error}");
            Err(command_error(error))
        }
    }
}

#[tauri::command]
pub fn fs_get_directory(
    path: String,
    volume_root: String,
    config: Option<ScanConfigDto>,
    state: State<'_, AppState>,
) -> Result<DirectoryListingDto, CommandError> {
    let config = config.unwrap_or_default().into();
    let volume_root = PathBuf::from(volume_root);
    let path = PathBuf::from(path);
    log::debug!(
        "getting cached filesystem directory {} inside volume {}",
        path.display(),
        volume_root.display()
    );
    match state.fs_driver.get_directory(&volume_root, &path, &config) {
        Ok(listing) => Ok(DirectoryListingDto::from(&listing)),
        Err(error) => {
            log::warn!("failed to get filesystem directory: {error}");
            Err(command_error(error))
        }
    }
}

#[tauri::command]
pub fn fs_start_scan(
    path: String,
    volume_root: String,
    config: Option<ScanConfigDto>,
    replace_existing: Option<bool>,
    progress_channel: Channel<FsProgressEventDto>,
    state: State<'_, AppState>,
) -> Result<StartScanReceiptDto, CommandError> {
    log::info!(
        "starting filesystem scan for {} inside volume {}",
        path,
        volume_root
    );
    let channel = progress_channel.clone();
    let sink = Arc::new(move |event| {
        match &event {
            hd_analyzer_core::DriverEvent::DirectoryReady { path, listing, .. } => {
                log::info!(
                    "filesystem scan produced listing for {} with {} child node(s), total measured size {}, total logical size {}",
                    path.display(),
                    listing.children.len(),
                    listing.total_measured_size,
                    listing.total_logical_size
                );
            }
            hd_analyzer_core::DriverEvent::JobFinished { job_id, path, .. } => {
                log::info!(
                    "filesystem scan job {job_id} finished for {}",
                    path.display()
                );
            }
            hd_analyzer_core::DriverEvent::JobFailed {
                job_id,
                path,
                message,
                ..
            } => {
                log::warn!(
                    "filesystem scan job {job_id} failed for {}: {message}",
                    path.display()
                );
            }
            _ => {}
        }
        let _ = channel.send(FsProgressEventDto::from(event));
    });
    match state.fs_driver.start_scan(
        StartScanRequest {
            volume_root: PathBuf::from(volume_root),
            path: PathBuf::from(path),
            config: config.unwrap_or_default().into(),
            replace_existing: replace_existing.unwrap_or(true),
            request_id: chrono_like_timestamp(),
        },
        Some(sink),
    ) {
        Ok(receipt) => {
            log::info!("filesystem scan queued as job {}", receipt.job_id);
            Ok(StartScanReceiptDto::from(receipt))
        }
        Err(error) => {
            log::warn!("failed to start filesystem scan: {error}");
            Err(command_error(error))
        }
    }
}

#[tauri::command]
pub fn fs_cancel_job(job_id: String, state: State<'_, AppState>) -> Result<(), CommandError> {
    log::info!("canceling filesystem scan job {job_id}");
    state.fs_driver.cancel_job(&job_id).map_err(|error| {
        log::warn!("failed to cancel filesystem scan job {job_id}: {error}");
        command_error(error)
    })
}

#[tauri::command]
pub fn fs_invalidate_path(
    path: String,
    volume_root: String,
    scope: InvalidationScopeDto,
    state: State<'_, AppState>,
) -> Result<InvalidationReceiptDto, CommandError> {
    let volume_root = PathBuf::from(volume_root);
    let path = PathBuf::from(path);
    log::info!(
        "invalidating filesystem path {} inside volume {}",
        path.display(),
        volume_root.display()
    );
    state
        .fs_driver
        .invalidate_path(&volume_root, &path, scope.into())
        .map(InvalidationReceiptDto::from)
        .map_err(|error| {
            log::warn!("failed to invalidate filesystem path: {error}");
            command_error(error)
        })
}

#[tauri::command]
pub fn check_permissions(root: Option<String>) -> PermissionCheckDto {
    let Some(root) = root.filter(|value| !value.trim().is_empty()) else {
        log::warn!("permission check requested without a selected volume");
        return PermissionCheckDto {
            granted: false,
            message: "Select a volume before scanning.".to_string(),
        };
    };
    let root_path = PathBuf::from(&root);
    log::info!(
        "checking filesystem permissions for {}",
        root_path.display()
    );

    match fs::read_dir(&root_path) {
        Ok(_) => {
            log::info!(
                "filesystem permissions verified for {}",
                root_path.display()
            );
            PermissionCheckDto {
                granted: true,
                message: format!("File permissions verified for {}.", root_path.display()),
            }
        }
        Err(error) => {
            log::warn!(
                "filesystem permission check failed for {}: {error}",
                root_path.display()
            );
            PermissionCheckDto {
                granted: false,
                message: format!("HD Analyzer cannot read {}: {error}", root_path.display()),
            }
        }
    }
}

#[tauri::command]
pub fn start_scan(
    root: String,
    state: State<'_, AppState>,
) -> Result<ScanSessionDto, CommandError> {
    log::info!("starting legacy full scan for {root}");
    if root.trim().is_empty() {
        log::warn!("legacy full scan rejected because root path is empty");
        return Err(CommandError::new(
            CommandErrorCode::InvalidRoot,
            "Root path cannot be empty.",
        ));
    }

    let mut active_scan = state.active_scan.lock().unwrap();
    if active_scan.is_some() {
        log::warn!("legacy full scan rejected because another scan is running");
        return Err(CommandError::new(
            CommandErrorCode::ScanAlreadyRunning,
            "A scan is already running.",
        ));
    }

    let scan_root = PathBuf::from(&root);
    let scan_root = scan_root.canonicalize().map_err(|error| {
        CommandError::new(
            CommandErrorCode::ScanStartFailed,
            format!("Unable to access {}: {error}", scan_root.display()),
        )
    })?;
    let root = scan_root.display().to_string();
    let session_id = format!("scan-{}", chrono_like_timestamp());
    log::info!("legacy full scan session {session_id} started for {root}");
    *active_scan = Some(session_id.clone());
    state.sessions.lock().unwrap().insert(
        session_id.clone(),
        StoredSession {
            root: scan_root.clone(),
            result: None,
            status: SessionStatusDto::Scanning,
            error_message: None,
        },
    );
    let sessions = state.sessions.clone();
    let active_scan_state = state.active_scan.clone();
    let session_id_for_worker = session_id.clone();
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<ScanUpdate>();
        let worker_root = scan_root.clone();
        let started_at = Instant::now();
        let scan_thread = thread::spawn(move || scan_drive(&worker_root, started_at, &tx, false));

        while let Ok(_update) = rx.try_recv() {}

        match scan_thread.join() {
            Ok(Ok(result)) => {
                log::info!("legacy full scan session {session_id_for_worker} completed");
                if let Some(session) = sessions.lock().unwrap().get_mut(&session_id_for_worker) {
                    session.result = Some(result);
                    session.status = SessionStatusDto::Complete;
                }
            }
            Ok(Err(error)) => {
                log::error!("legacy full scan session {session_id_for_worker} failed: {error}");
                if let Some(session) = sessions.lock().unwrap().get_mut(&session_id_for_worker) {
                    session.status = SessionStatusDto::Failed;
                    session.error_message = Some(error.to_string());
                }
            }
            Err(_) => {
                log::error!("legacy full scan session {session_id_for_worker} worker panicked");
                if let Some(session) = sessions.lock().unwrap().get_mut(&session_id_for_worker) {
                    session.status = SessionStatusDto::Failed;
                    session.error_message = Some("Scan worker panicked.".to_string());
                }
            }
        }
        *active_scan_state.lock().unwrap() = None;
    });

    Ok(ScanSessionDto {
        session_id,
        root,
        status: SessionStatusDto::Scanning,
        error_message: None,
    })
}

#[tauri::command]
pub fn get_scan_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<ScanSessionDto, CommandError> {
    let sessions = state.sessions.lock().unwrap();
    let session = get_session(&sessions, &session_id)?;
    Ok(ScanSessionDto {
        session_id,
        root: session.root.display().to_string(),
        status: session.status.clone(),
        error_message: session.error_message.clone(),
    })
}

#[tauri::command]
pub fn list_directory_entries(
    session_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<DirectoryEntryDto>, CommandError> {
    let sessions = state.sessions.lock().unwrap();
    let session = get_session(&sessions, &session_id)?;
    let requested = PathBuf::from(path);
    ensure_inside_root(&requested, &session.root)?;

    let Some(result) = &session.result else {
        return Ok(Vec::new());
    };
    Ok(directory_entries(result, &requested))
}

#[tauri::command]
pub fn rescan_subtree(
    session_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<ScanSessionDto, CommandError> {
    let sessions = state.sessions.lock().unwrap();
    let session = get_session(&sessions, &session_id)?;
    let requested = PathBuf::from(&path);
    ensure_inside_root(&requested, &session.root)?;
    drop(sessions);

    Ok(ScanSessionDto {
        session_id,
        root: path,
        status: SessionStatusDto::PartialRescan,
        error_message: None,
    })
}

#[tauri::command]
pub fn get_read_errors(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ReadErrorDto>, CommandError> {
    let sessions = state.sessions.lock().unwrap();
    let session = get_session(&sessions, &session_id)?;
    let Some(result) = &session.result else {
        return Ok(Vec::new());
    };
    Ok(result
        .read_errors
        .iter()
        .map(|error| ReadErrorDto::from_error(error, &result.root))
        .collect())
}

fn get_session<'a>(
    sessions: &'a MutexGuard<'_, std::collections::HashMap<String, StoredSession>>,
    session_id: &str,
) -> Result<&'a StoredSession, CommandError> {
    sessions.get(session_id).ok_or_else(|| {
        CommandError::new(
            CommandErrorCode::UnknownSession,
            format!("Unknown scan session: {session_id}"),
        )
    })
}

fn ensure_inside_root(path: &Path, root: &Path) -> Result<(), CommandError> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(CommandError::new(
            CommandErrorCode::PathOutsideRoot,
            format!("Path {} is outside {}", path.display(), root.display()),
        ))
    }
}

fn directory_entries(result: &ScanResult, path: &Path) -> Vec<DirectoryEntryDto> {
    let mut entries: Vec<_> = result
        .directory_sizes
        .iter()
        .filter(|(entry_path, _)| entry_path.parent() == Some(path))
        .map(|(entry_path, size)| {
            let display_name = entry_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("..")
                .to_string();
            DirectoryEntryDto {
                path: entry_path.display().to_string(),
                display_name,
                size: *size,
                share: if result.bytes_scanned == 0 {
                    0.0
                } else {
                    *size as f64 / result.bytes_scanned as f64
                },
                is_directory: true,
                is_virtual: false,
                kind: "directory".to_string(),
            }
        })
        .collect();
    entries.sort_by(|left, right| {
        right
            .size
            .cmp(&left.size)
            .then_with(|| left.path.cmp(&right.path))
    });
    entries
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn command_error(error: hd_analyzer_core::DriverError) -> CommandError {
    let code = match error {
        hd_analyzer_core::DriverError::JobNotFound(_) => CommandErrorCode::JobNotFound,
        hd_analyzer_core::DriverError::DriverUnavailable(_) => CommandErrorCode::DriverUnavailable,
        hd_analyzer_core::DriverError::InvalidPath(_) => CommandErrorCode::InvalidRoot,
        hd_analyzer_core::DriverError::PathOutsideVolume(_) => CommandErrorCode::PathOutsideVolume,
        hd_analyzer_core::DriverError::PermissionDenied(_) => CommandErrorCode::PathOutsideRoot,
        hd_analyzer_core::DriverError::Io { .. } => CommandErrorCode::DriverUnavailable,
    };
    CommandError::new(code, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_outside_root() {
        let error =
            ensure_inside_root(Path::new("/tmp/other"), Path::new("/tmp/root")).unwrap_err();
        assert_eq!(error.code, CommandErrorCode::PathOutsideRoot);
    }

    #[test]
    fn maps_outside_volume_driver_error_to_outside_volume_code() {
        let error = command_error(hd_analyzer_core::DriverError::PathOutsideVolume(
            "outside".to_string(),
        ));

        assert_eq!(error.code, CommandErrorCode::PathOutsideVolume);
    }
}
