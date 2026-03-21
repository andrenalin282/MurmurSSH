use std::fs;
use std::path::PathBuf;

use crate::models::Profile;

fn config_base() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    PathBuf::from(home).join(".config").join("murmurssh")
}

fn profiles_dir() -> PathBuf {
    config_base().join("profiles")
}

pub fn ensure_dirs() -> Result<(), String> {
    let base = config_base();
    for dir in [
        base.join("profiles"),
        base.join("workspace"),
        base.join("logs"),
    ] {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    }
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
    let json = serde_json::to_string_pretty(profile)
        .map_err(|e| format!("Failed to serialize profile: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write profile: {}", e))
}

pub fn delete_profile(id: &str) -> Result<(), String> {
    let path = profiles_dir().join(format!("{}.json", id));
    if !path.exists() {
        return Err(format!("Profile '{}' not found", id));
    }
    fs::remove_file(&path).map_err(|e| format!("Failed to delete profile: {}", e))
}
