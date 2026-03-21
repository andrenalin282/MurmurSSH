use crate::services::{profile_service, ssh_service};

#[tauri::command]
pub fn launch_ssh(profile_id: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    ssh_service::launch_ssh(&profile)
}
