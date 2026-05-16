use std::fs;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use hd_analyzer_core::{ScanResult, ScanUpdate, list_drives as core_list_drives, scan_drive};
use tauri::State;

use crate::dto::{
    CommandError, CommandErrorCode, DirectoryEntryDto, DriveDto, PermissionCheckDto, ReadErrorDto,
    ScanSessionDto, SessionStatusDto,
};
use crate::state::{AppState, StoredSession};

#[tauri::command]
pub fn list_drives() -> Result<Vec<DriveDto>, CommandError> {
    let drives = core_list_drives().map_err(|error| {
        CommandError::new(CommandErrorCode::DriveDiscoveryFailed, error.to_string())
    })?;
    if drives.is_empty() {
        return Err(CommandError::new(
            CommandErrorCode::NoDrives,
            "No mounted drives found.",
        ));
    }
    Ok(drives.iter().map(DriveDto::from).collect())
}

#[tauri::command]
pub fn check_permissions(root: Option<String>) -> PermissionCheckDto {
    let Some(root) = root.filter(|value| !value.trim().is_empty()) else {
        return PermissionCheckDto {
            granted: false,
            message: "Select a volume before scanning.".to_string(),
        };
    };
    let root_path = PathBuf::from(&root);

    match fs::read_dir(&root_path) {
        Ok(_) => PermissionCheckDto {
            granted: true,
            message: format!("File permissions verified for {}.", root_path.display()),
        },
        Err(error) => PermissionCheckDto {
            granted: false,
            message: format!("HD Analyzer cannot read {}: {error}", root_path.display()),
        },
    }
}

#[tauri::command]
pub fn start_scan(
    root: String,
    state: State<'_, AppState>,
) -> Result<ScanSessionDto, CommandError> {
    if root.trim().is_empty() {
        return Err(CommandError::new(
            CommandErrorCode::InvalidRoot,
            "Root path cannot be empty.",
        ));
    }

    let mut active_scan = state.active_scan.lock().unwrap();
    if active_scan.is_some() {
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
                if let Some(session) = sessions.lock().unwrap().get_mut(&session_id_for_worker) {
                    session.result = Some(result);
                    session.status = SessionStatusDto::Complete;
                }
            }
            Ok(Err(error)) => {
                if let Some(session) = sessions.lock().unwrap().get_mut(&session_id_for_worker) {
                    session.status = SessionStatusDto::Failed;
                    session.error_message = Some(error.to_string());
                }
            }
            Err(_) => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_outside_root() {
        let error =
            ensure_inside_root(Path::new("/tmp/other"), Path::new("/tmp/root")).unwrap_err();
        assert_eq!(error.code, CommandErrorCode::PathOutsideRoot);
    }
}
