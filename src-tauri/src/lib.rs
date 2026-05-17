mod commands;
mod dto;
mod state;

use commands::{
    check_permissions, fs_cancel_job, fs_get_directory, fs_invalidate_path, fs_list_volumes,
    fs_open_path, fs_start_scan, get_read_errors, get_scan_session, list_directory_entries,
    list_drives, rescan_subtree, start_scan,
};
use state::AppState;
use tauri_plugin_log::{Target, TargetKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log_level)
                .target(Target::new(TargetKind::Webview))
                .build(),
        )
        .plugin(tauri_plugin_macos_permissions::init())
        .manage(AppState::default())
        .setup(|_app| {
            log::info!("HD Analyzer Tauri runtime initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_drives,
            fs_list_volumes,
            fs_open_path,
            fs_get_directory,
            fs_start_scan,
            fs_cancel_job,
            fs_invalidate_path,
            check_permissions,
            start_scan,
            get_scan_session,
            list_directory_entries,
            rescan_subtree,
            get_read_errors
        ])
        .run(tauri::generate_context!())
        .expect("error while running HD Analyzer Tauri application");
}
