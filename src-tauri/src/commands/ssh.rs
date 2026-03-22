use crate::services::{profile_service, runtime_key_service, ssh_service, ssh_session_service};

/// Return the application version string from the built-in Tauri package info.
#[tauri::command]
pub fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Launch an SSH terminal session for the given profile.
///
/// If `use_runtime_copy` is true, the terminal uses a pre-created local copy of
/// the key file (in ~/.config/murmurssh/runtime-keys/) instead of the original.
/// This fixes "UNPROTECTED PRIVATE KEY FILE" rejections from OpenSSH when keys
/// are stored on mounted or network filesystems with incompatible permissions.
///
/// The runtime copy must be created by calling `copy_key_for_runtime` first.
/// The user must have been informed and have accepted the copy before this is set.
#[tauri::command]
pub fn launch_ssh(profile_id: String, use_runtime_copy: Option<bool>) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    ssh_service::launch_ssh(&profile, use_runtime_copy.unwrap_or(false))
}

/// Close the application cleanly.
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Open a URL in the system default browser using xdg-open.
/// Only allows http/https URLs as a safety measure.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Only http/https URLs are supported".to_string());
    }
    std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open URL: {}", e))
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

/// Check whether the SSH key for a profile needs a local runtime copy for
/// terminal compatibility.
///
/// Returns true if the key file has group or other permission bits set, which
/// would cause the system OpenSSH client to reject it.
///
/// The frontend should call this before launching a terminal for key-auth profiles
/// and prompt the user if a runtime copy is needed.
#[tauri::command]
pub fn check_key_needs_copy(profile_id: String) -> Result<bool, String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if profile.auth_type != crate::models::AuthType::Key {
        return Ok(false);
    }
    match &profile.key_path {
        Some(path) => Ok(runtime_key_service::key_needs_runtime_copy(path)),
        None => Ok(false),
    }
}

/// Create a runtime-only local copy of the SSH key for terminal compatibility.
///
/// The copy is stored in ~/.config/murmurssh/runtime-keys/<profile_id> with 0600
/// permissions so the system OpenSSH client accepts it. The original key file is
/// never modified. The copy is deleted on disconnect, quit, and startup cleanup.
///
/// This must only be called after the user has explicitly accepted the copy in the
/// runtime key prompt dialog.
#[tauri::command]
pub fn copy_key_for_runtime(profile_id: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    let key_path = profile
        .key_path
        .ok_or_else(|| "Profile has no key path configured".to_string())?;
    runtime_key_service::copy_key_for_runtime(&profile_id, &key_path)?;
    Ok(())
}

/// Delete the runtime key copy for a profile.
/// Called on disconnect to clean up the temporary file immediately.
#[tauri::command]
pub fn delete_runtime_key(profile_id: String) -> Result<(), String> {
    runtime_key_service::delete_runtime_key(&profile_id);
    Ok(())
}

/// Delete all runtime key copies (startup cleanup).
/// Removes any leftover runtime keys from previous sessions that crashed or were
/// force-killed without completing the normal disconnect/quit cleanup path.
#[tauri::command]
pub fn cleanup_runtime_keys() -> Result<(), String> {
    runtime_key_service::cleanup_all_runtime_keys();
    Ok(())
}
