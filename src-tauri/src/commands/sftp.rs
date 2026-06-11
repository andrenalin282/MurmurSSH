use crate::models::{FileEntry, Profile, Protocol};
use crate::services::{ftp_service, profile_service, sftp_service};

fn is_ftp(profile: &Profile) -> bool {
    profile.protocol.as_ref() == Some(&Protocol::Ftp)
}

/// Resolve the server-side effective home directory.
///
/// For SFTP: uses realpath(".") (typically the user's home). Falls back to "/" on error.
/// For FTP:  always returns "/".
/// Called when a profile has no explicit default_remote_path configured.
#[tauri::command]
pub fn get_sftp_home(profile_id: String) -> Result<String, String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::get_home(&profile)
    } else {
        sftp_service::get_sftp_home(&profile)
    }
}

#[tauri::command]
pub fn list_directory(profile_id: String, path: String) -> Result<Vec<FileEntry>, String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::list_directory(&profile, &path)
    } else {
        sftp_service::list_directory(&profile, &path)
    }
}

/// Check whether a remote path exists (file or directory).
/// Returns true if accessible, false if not found or inaccessible.
#[tauri::command]
pub fn remote_file_exists(profile_id: String, remote_path: String) -> Result<bool, String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::remote_file_exists(&profile, &remote_path)
    } else {
        sftp_service::remote_file_exists(&profile, &remote_path)
    }
}

/// Upload raw bytes from the frontend to a remote path.
/// Used by the file browser upload button, which reads the file in JS.
#[tauri::command]
pub fn upload_file_bytes(
    profile_id: String,
    remote_path: String,
    content: Vec<u8>,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::upload_bytes(&profile, &remote_path, &content)
    } else {
        sftp_service::upload_bytes(&profile, &remote_path, &content)
    }
}

#[tauri::command]
pub fn delete_file(profile_id: String, remote_path: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::delete_file(&profile, &remote_path)
    } else {
        sftp_service::delete_file(&profile, &remote_path)
    }
}

#[tauri::command]
pub fn rename_file(
    profile_id: String,
    from_path: String,
    to_path: String,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::rename_file(&profile, &from_path, &to_path)
    } else {
        sftp_service::rename_file(&profile, &from_path, &to_path)
    }
}

/// Change the Unix permission bits (mode) of a remote file or directory.
#[tauri::command]
pub fn set_permissions(
    profile_id: String,
    remote_path: String,
    mode: u32,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::set_permissions(&profile, &remote_path, mode)
    } else {
        sftp_service::set_permissions(&profile, &remote_path, mode)
    }
}

#[tauri::command]
pub fn create_directory(profile_id: String, path: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::create_directory(&profile, &path)
    } else {
        sftp_service::create_directory(&profile, &path)
    }
}

/// Recursively delete a remote directory and all of its contents.
/// The frontend must confirm with the user before calling this command.
#[tauri::command]
pub fn delete_directory(profile_id: String, remote_path: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::delete_directory(&profile, &remote_path)
    } else {
        sftp_service::delete_directory(&profile, &remote_path)
    }
}

/// Check whether a local path exists (file or directory).
/// Used by the frontend to prompt before silently overwriting a local file on download.
#[tauri::command]
pub fn local_file_exists(path: String) -> bool {
    if path.is_empty() { return false; }
    std::path::Path::new(&path).exists()
}
