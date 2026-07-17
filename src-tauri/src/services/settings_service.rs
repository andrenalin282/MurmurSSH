use std::fs;
use std::path::PathBuf;

use crate::models::Settings;

fn settings_path() -> PathBuf {
    // Fall back to "/" rather than panicking — with panic=abort in release,
    // an unset $HOME would otherwise terminate the entire app.
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("murmurssh")
        .join("settings.json")
}

pub fn get_settings() -> Result<Settings, String> {
    let path = settings_path();
    if !path.exists() {
        return Ok(Settings::default());
    }
    let contents =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read settings: {}", e))?;
    serde_json::from_str(&contents).map_err(|e| format!("Failed to parse settings: {}", e))
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let mut normalized = settings.clone();
    // Normalize extension keys (lowercase, no leading dot) and drop empty entries.
    if let Some(map) = normalized.editor_by_extension.take() {
        let cleaned: std::collections::HashMap<String, String> = map
            .into_iter()
            .filter_map(|(k, v)| {
                let key = crate::services::editor_service::normalize_extension(&k);
                let val = v.trim().to_string();
                if key.is_empty() || val.is_empty() {
                    None
                } else {
                    Some((key, val))
                }
            })
            .collect();
        normalized.editor_by_extension = if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        };
    }
    if let Some(ref ed) = normalized.default_editor {
        if ed.trim().is_empty() {
            normalized.default_editor = None;
        }
    }

    let path = settings_path();
    let json = serde_json::to_string_pretty(&normalized)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write settings: {}", e))
}
