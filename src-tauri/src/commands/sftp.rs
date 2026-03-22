use std::path::PathBuf;

use crate::models::FileEntry;
use crate::services::{profile_service, sftp_service};

#[tauri::command]
pub fn list_directory(profile_id: String, path: String) -> Result<Vec<FileEntry>, String> {
    let profile = profile_service::get_profile(&profile_id)?;
    sftp_service::list_directory(&profile, &path)
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
    sftp_service::upload_bytes(&profile, &remote_path, &content)
}

/// Upload a local file path to a remote path.
/// Used internally by the workspace confirm flow.
#[tauri::command]
pub fn upload_file(
    profile_id: String,
    local_path: String,
    remote_path: String,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    sftp_service::upload_file(&profile, &local_path, &remote_path)
}

/// Download a remote file to a user-specified local path.
/// Called by the frontend after the user picks a save location via the save dialog.
#[tauri::command]
pub fn download_file_to(
    profile_id: String,
    remote_path: String,
    local_path: String,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    sftp_service::download_file(&profile, &remote_path, &local_path)
}

/// Download a remote file to ~/Downloads/<filename> and return the save path.
#[tauri::command]
pub fn download_file(profile_id: String, remote_path: String) -> Result<String, String> {
    let profile = profile_service::get_profile(&profile_id)?;

    let filename = std::path::Path::new(&remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());

    let save_dir = downloads_dir();
    let local_path = save_dir.join(&filename);
    let local_path_str = local_path.to_string_lossy().to_string();

    sftp_service::download_file(&profile, &remote_path, &local_path_str)?;
    Ok(local_path_str)
}

#[tauri::command]
pub fn delete_file(profile_id: String, remote_path: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    sftp_service::delete_file(&profile, &remote_path)
}

#[tauri::command]
pub fn rename_file(
    profile_id: String,
    from_path: String,
    to_path: String,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    sftp_service::rename_file(&profile, &from_path, &to_path)
}

#[tauri::command]
pub fn create_directory(profile_id: String, path: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    sftp_service::create_directory(&profile, &path)
}

/// Recursively delete a remote directory and all of its contents.
/// The frontend must confirm with the user before calling this command.
#[tauri::command]
pub fn delete_directory(profile_id: String, remote_path: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    sftp_service::delete_directory(&profile, &remote_path)
}

/// Returns ~/Downloads if it exists, otherwise ~/ as a fallback.
fn downloads_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let downloads = PathBuf::from(&home).join("Downloads");
    if downloads.is_dir() {
        downloads
    } else {
        PathBuf::from(home)
    }
}
