use crate::services::{profile_service, workspace_service};

#[tauri::command]
pub fn open_for_edit(
    app: tauri::AppHandle,
    profile_id: String,
    remote_path: String,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    workspace_service::open_for_edit(app, &profile, &remote_path)
}
