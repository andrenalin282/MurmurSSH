use crate::models::Settings;
use crate::services::settings_service;

#[tauri::command]
pub fn get_settings() -> Result<Settings, String> {
    settings_service::get_settings()
}

#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<(), String> {
    settings_service::save_settings(&settings)
}
