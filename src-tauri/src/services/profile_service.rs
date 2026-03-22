use std::fs;
use std::path::PathBuf;

use crate::models::Profile;
use crate::services::settings_service;

fn config_base() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    PathBuf::from(home).join(".config").join("murmurssh")
}

fn profiles_dir() -> PathBuf {
    // Use custom profiles directory from settings if configured.
    if let Ok(settings) = settings_service::get_settings() {
        if let Some(custom) = settings.profiles_path {
            if !custom.is_empty() {
                return PathBuf::from(custom);
            }
        }
    }
    config_base().join("profiles")
}

/// Returns the current profiles directory (default or custom).
/// Used by commands that need to open or display the path.
pub fn get_profiles_dir() -> PathBuf {
    profiles_dir()
}

pub fn ensure_dirs() -> Result<(), String> {
    let base = config_base();
    // Always create workspace and logs in the default config base
    for dir in [base.join("workspace"), base.join("logs")] {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    }
    // Profiles directory may be a custom path
    let pdir = profiles_dir();
    fs::create_dir_all(&pdir)
        .map_err(|e| format!("Failed to create profiles dir {}: {}", pdir.display(), e))?;
    Ok(())
}


pub fn list_profiles() -> Result<Vec<Profile>, String> {
    ensure_dirs()?;
    let mut profiles = Vec::new();

    let entries = fs::read_dir(profiles_dir())
        .map_err(|e| format!("Failed to read profiles directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let contents = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let profile: Profile = serde_json::from_str(&contents)
                .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
            profiles.push(profile);
        }
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(profiles)
}

pub fn get_profile(id: &str) -> Result<Profile, String> {
    let path = profiles_dir().join(format!("{}.json", id));
    let contents =
        fs::read_to_string(&path).map_err(|_| format!("Profile '{}' not found", id))?;
    serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse profile '{}': {}", id, e))
}

pub fn save_profile(profile: &Profile) -> Result<(), String> {
    ensure_dirs()?;
    let path = profiles_dir().join(format!("{}.json", profile.id));
    // Create a backup of the existing file before overwriting
    if path.exists() {
        let bkp = profiles_dir().join(format!("{}.json.bkp", profile.id));
        fs::copy(&path, &bkp).map_err(|e| format!("Failed to create profile backup: {}", e))?;
    }
    let json = serde_json::to_string_pretty(profile)
        .map_err(|e| format!("Failed to serialize profile: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write profile: {}", e))
}

pub fn delete_profile(id: &str) -> Result<(), String> {
    let path = profiles_dir().join(format!("{}.json", id));
    if !path.exists() {
        return Err(format!("Profile '{}' not found", id));
    }
    fs::remove_file(&path).map_err(|e| format!("Failed to delete profile: {}", e))?;

    // Also remove the backup file created by save_profile, if it exists.
    let bkp = profiles_dir().join(format!("{}.json.bkp", id));
    if bkp.exists() {
        let _ = fs::remove_file(&bkp); // Non-fatal — best-effort cleanup
    }

    Ok(())
}
