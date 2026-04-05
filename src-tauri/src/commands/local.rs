use crate::models::FileEntry;
use crate::services::local_service;

/// List the contents of a local directory.
///
/// Rejects paths with null bytes; fails if the path is not a readable directory.
#[tauri::command]
pub fn list_local_directory(path: String) -> Result<Vec<FileEntry>, String> {
    local_service::list_local_directory(&path)
}

/// Return the current user's home directory ($HOME).
#[tauri::command]
pub fn get_home_dir() -> String {
    local_service::get_home_dir()
}

/// Return the current OS username ($USER / $LOGNAME).
#[tauri::command]
pub fn get_current_user() -> String {
    local_service::get_current_user()
}

/// Return the saved local browser path for a profile + current user.
///
/// Resolution: portable profile → per-user map; local profile → local_path field; fallback → $HOME.
#[tauri::command]
pub fn get_local_browser_path(profile_id: String) -> Result<String, String> {
    local_service::get_local_browser_path(&profile_id)
}

/// Persist the current local browser path for the profile + current OS user.
///
/// Validates the path (absolute, no null bytes, existing directory) before saving.
#[tauri::command]
pub fn save_local_browser_path(profile_id: String, path: String) -> Result<(), String> {
    local_service::save_local_browser_path(&profile_id, &path)
}
