mod commands;
mod dto;
mod state;

use commands::{
    check_permissions, fs_cancel_job, fs_get_directory, fs_invalidate_path, fs_list_volumes,
    fs_open_path, fs_open_path_with_progress, fs_preview_item, fs_reveal_items, fs_start_scan,
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
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|_app| {
            log::info!("HD Analyzer Tauri runtime initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            fs_list_volumes,
            fs_open_path,
            fs_open_path_with_progress,
            fs_get_directory,
            fs_start_scan,
            fs_cancel_job,
            fs_invalidate_path,
            fs_reveal_items,
            fs_preview_item,
            check_permissions
        ])
        .run(tauri::generate_context!())
        .expect("error while running HD Analyzer Tauri application");
}
