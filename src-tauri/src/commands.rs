use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use hd_analyzer_core::{HdDriver, OpenPathRequest, StartScanRequest};
use tauri::{State, ipc::Channel};

use crate::dto::{
    CommandError, CommandErrorCode, DirectoryListingDto, DriveDto, FsProgressEventDto,
    InvalidationReceiptDto, InvalidationScopeDto, PermissionCheckDto, ScanConfigDto,
    StartScanReceiptDto,
};
use crate::state::AppState;

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
        hd_analyzer_core::DriverError::ScanCanceled => CommandErrorCode::ScanStartFailed,
        hd_analyzer_core::DriverError::Io { .. } => CommandErrorCode::DriverUnavailable,
    };
    CommandError::new(code, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_outside_volume_driver_error_to_outside_volume_code() {
        let error = command_error(hd_analyzer_core::DriverError::PathOutsideVolume(
            "outside".to_string(),
        ));

        assert_eq!(error.code, CommandErrorCode::PathOutsideVolume);
    }
}
