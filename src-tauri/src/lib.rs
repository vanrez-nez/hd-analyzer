mod commands;
mod dto;
mod state;

use commands::{
    check_permissions, get_read_errors, get_scan_session, list_directory_entries, list_drives,
    rescan_subtree, start_scan,
};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_macos_permissions::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_drives,
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
