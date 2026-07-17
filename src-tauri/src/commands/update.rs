use crate::models::update::UpdateCheckResult;
use crate::services::update_service;

#[tauri::command]
pub fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateCheckResult, String> {
    let current = app.package_info().version.to_string();
    update_service::check_latest(&current)
}
