use crate::services::launch_service;

/// Canonical id of the profile this instance was launched to connect to, if any.
#[tauri::command]
pub fn get_launch_profile() -> Option<String> {
    launch_service::launch_profile_id()
}

/// Spawn a second, independent MurmurSSH window connecting to `profile_id`.
#[tauri::command]
pub fn open_profile_in_new_window(profile_id: String) -> Result<(), String> {
    launch_service::open_in_new_window(&profile_id)
}

/// Write a `.desktop` launcher for `profile_id` to a user-chosen path; returns it.
#[tauri::command]
pub fn create_desktop_shortcut(profile_id: String, target_path: String) -> Result<String, String> {
    launch_service::create_desktop_shortcut(&profile_id, &target_path)
}

/// Locale-aware default directory (Desktop) for the shortcut save dialog.
#[tauri::command]
pub fn get_desktop_dir() -> String {
    launch_service::desktop_dir()
}
