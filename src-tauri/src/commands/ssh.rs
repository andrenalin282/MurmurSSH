use crate::services::{profile_service, ssh_service, ssh_session_service};

#[tauri::command]
pub fn launch_ssh(profile_id: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    ssh_service::launch_ssh(&profile)
}

/// Establish a background SSH session for terminal SSO.
///
/// Must be called after `connect_sftp` succeeds so the session credential
/// store is populated. Non-fatal: if this fails the terminal still works,
/// it will just prompt for credentials again.
#[tauri::command]
pub fn start_ssh_session(profile_id: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    ssh_session_service::start_session(&profile)
}

/// Stop and clean up the SSH session for a profile.
/// Called on disconnect to release the ControlMaster/agent process and socket.
#[tauri::command]
pub fn stop_ssh_session(profile_id: String) -> Result<(), String> {
    ssh_session_service::stop_session(&profile_id);
    Ok(())
}
