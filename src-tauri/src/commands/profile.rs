use crate::models::Profile;
use crate::services::{profile_service, settings_service};
use std::path::Path;

#[tauri::command]
pub fn list_profiles() -> Result<Vec<Profile>, String> {
    profile_service::list_profiles()
}

#[tauri::command]
pub fn get_profile(id: String) -> Result<Profile, String> {
    profile_service::get_profile(&id)
}

#[tauri::command]
pub fn save_profile(profile: Profile) -> Result<(), String> {
    profile_service::save_profile(&profile)
}

#[tauri::command]
pub fn delete_profile(id: String) -> Result<(), String> {
    // Clear the last-used setting if this profile was selected.
    if let Ok(mut settings) = settings_service::get_settings() {
        if settings.last_used_profile_id.as_deref() == Some(&id) {
            settings.last_used_profile_id = None;
            let _ = settings_service::save_settings(&settings);
        }
    }
    profile_service::delete_profile(&id)
}

#[tauri::command]
pub fn check_path_exists(path: String) -> bool {
    Path::new(&path).exists()
}
