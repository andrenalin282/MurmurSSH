use crate::models::{AuthType, Profile};
use crate::services::{credentials_store, profile_service, secrets_service, settings_service};
use std::path::Path;

#[tauri::command]
pub fn list_profiles() -> Result<Vec<Profile>, String> {
    profile_service::list_profiles()
}

#[tauri::command]
pub fn get_profile(id: String) -> Result<Profile, String> {
    profile_service::get_profile(&id)
}

/// Save a profile. If the authentication type is switching away from password auth,
/// any previously stored password credential is cleared automatically.
#[tauri::command]
pub fn save_profile(profile: Profile) -> Result<(), String> {
    // Detect auth-type switch away from password. If the user changes a profile
    // from password auth to SSH key or agent, remove any saved password credential
    // so stale secret data does not accumulate.
    if let Ok(existing) = profile_service::get_profile(&profile.id) {
        if existing.auth_type == AuthType::Password && profile.auth_type != AuthType::Password {
            secrets_service::delete(&profile.id);
            credentials_store::clear(&profile.id);
            // The incoming profile should already have credential fields cleared by the frontend,
            // but we ensure it here too by relying on what is actually being saved.
        }
    }

    profile_service::save_profile(&profile)
}

/// Delete a profile and clean up all associated data:
/// - removes the profile JSON file
/// - deletes any local machine secret file for this profile
/// - clears the session store entry
/// - clears the last-used-profile setting if it pointed here
#[tauri::command]
pub fn delete_profile(id: String) -> Result<(), String> {
    // Clear the last-used setting if this profile was selected
    if let Ok(mut settings) = settings_service::get_settings() {
        if settings.last_used_profile_id.as_deref() == Some(&id) {
            settings.last_used_profile_id = None;
            let _ = settings_service::save_settings(&settings);
        }
    }

    // Remove any local machine credential file. The portable credential field
    // lives in the profile JSON itself, so it disappears with the file below.
    secrets_service::delete(&id);

    // Clear session store so no stale credential lingers after deletion
    credentials_store::clear(&id);

    profile_service::delete_profile(&id)
}

#[tauri::command]
pub fn check_path_exists(path: String) -> bool {
    Path::new(&path).exists()
}
