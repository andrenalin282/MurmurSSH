use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::Profile;
use crate::services::settings_service;

/// Current time as epoch seconds (0 if the clock is before the epoch — never panics).
fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn config_base() -> PathBuf {
    // Fall back to "/" rather than panicking — with panic=abort in release,
    // an unset $HOME would otherwise terminate the entire app.
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
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
            let mut profile: Profile = serde_json::from_str(&contents)
                .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
            if profile.created_at.is_none() {
                profile.created_at = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
            }
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
    let mut profile: Profile = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse profile '{}': {}", id, e))?;
    // Backfill created_at from the file mtime for legacy profiles, so editing one
    // preserves its original creation date instead of resetting it to "now".
    // Single source of truth shared with list_profiles and save_profile's re-read.
    if profile.created_at.is_none() {
        profile.created_at = fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
    }
    Ok(profile)
}

pub fn save_profile(profile: &Profile) -> Result<(), String> {
    ensure_dirs()?;
    let path = profiles_dir().join(format!("{}.json", profile.id));

    // Stamp created_at exactly once: keep the existing value if the on-disk
    // profile already has one, otherwise use the incoming value or now().
    let mut to_write = profile.clone();
    if to_write.created_at.is_none() {
        let existing = get_profile(&profile.id).ok().and_then(|p| p.created_at);
        to_write.created_at = Some(existing.unwrap_or_else(now_epoch_secs));
    }

    // Create a backup of the existing file before overwriting
    if path.exists() {
        let bkp = profiles_dir().join(format!("{}.json.bkp", profile.id));
        fs::copy(&path, &bkp).map_err(|e| format!("Failed to create profile backup: {}", e))?;
    }
    let json = serde_json::to_string_pretty(&to_write)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_epoch_secs_is_after_2020() {
        assert!(now_epoch_secs() > 1_600_000_000);
    }
}
