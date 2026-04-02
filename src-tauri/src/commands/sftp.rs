use std::path::PathBuf;

use tauri::ipc::Channel;

use crate::models::{FileEntry, Profile, Protocol};
use crate::services::{ftp_service, profile_service, sftp_service};

/// Progress event streamed back to the frontend during file transfers.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    /// Bytes transferred so far (cumulative across all files for folder ops).
    pub bytes_done: u64,
    /// Total bytes for the current file; 0 means unknown (FTP, or folder op).
    pub bytes_total: u64,
    /// Name of the file currently being transferred.
    pub filename: String,
}

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

/// Upload a local file path to a remote path.
#[tauri::command]
pub fn upload_file(
    profile_id: String,
    local_path: String,
    remote_path: String,
    on_progress: Channel<TransferProgress>,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    let filename = std::path::Path::new(&remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if is_ftp(&profile) {
        ftp_service::upload_file(&profile, &local_path, &remote_path, &|done, total, name| {
            let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: name.to_string() });
        })
    } else {
        let fname = filename.clone();
        sftp_service::upload_file(&profile, &local_path, &remote_path, &|done, total| {
            let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: fname.clone() });
        })
    }
}

/// Download a remote file to a user-specified local path.
#[tauri::command]
pub fn download_file_to(
    profile_id: String,
    remote_path: String,
    local_path: String,
    on_progress: Channel<TransferProgress>,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    let filename = std::path::Path::new(&remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if is_ftp(&profile) {
        ftp_service::download_file_to(&profile, &remote_path, &local_path, &|done, total, name| {
            let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: name.to_string() });
        })
    } else {
        let fname = filename.clone();
        sftp_service::download_file(&profile, &remote_path, &local_path, &|done, total| {
            let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: fname.clone() });
        })
    }
}

/// Download a remote file to ~/Downloads/<filename> and return the save path.
#[tauri::command]
pub fn download_file(
    profile_id: String,
    remote_path: String,
    on_progress: Channel<TransferProgress>,
) -> Result<String, String> {
    let profile = profile_service::get_profile(&profile_id)?;

    let filename = std::path::Path::new(&remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());

    let save_dir = downloads_dir();
    let local_path = save_dir.join(&filename);
    let local_path_str = local_path.to_string_lossy().to_string();
    let fname = filename.clone();

    if is_ftp(&profile) {
        ftp_service::download_file_to(&profile, &remote_path, &local_path_str, &|done, total, name| {
            let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: name.to_string() });
        })?;
    } else {
        sftp_service::download_file(&profile, &remote_path, &local_path_str, &|done, total| {
            let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: fname.clone() });
        })?;
    }
    Ok(local_path_str)
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

/// Recursively upload a local directory to a remote destination path.
#[tauri::command]
pub fn upload_directory(
    profile_id: String,
    local_path: String,
    remote_path: String,
    on_progress: Channel<TransferProgress>,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    let cb = |done: u64, total: u64, name: &str| {
        let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: name.to_string() });
    };
    if is_ftp(&profile) {
        ftp_service::upload_directory(&profile, &local_path, &remote_path, &cb)
    } else {
        sftp_service::upload_directory(&profile, &local_path, &remote_path, &cb)
    }
}

/// Upload a local path (file or directory) to a remote destination.
#[tauri::command]
pub fn upload_path(
    profile_id: String,
    local_path: String,
    remote_path: String,
    on_progress: Channel<TransferProgress>,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    let path = std::path::Path::new(&local_path);
    let cb = |done: u64, total: u64, name: &str| {
        let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: name.to_string() });
    };
    if is_ftp(&profile) {
        if path.is_dir() {
            return ftp_service::upload_directory(&profile, &local_path, &remote_path, &cb);
        } else if path.is_file() {
            return ftp_service::upload_file(&profile, &local_path, &remote_path, &cb);
        }
        return Err(format!(
            "Cannot upload '{}': path does not exist or is not a file or directory",
            local_path
        ));
    }
    if path.is_dir() {
        sftp_service::upload_directory(&profile, &local_path, &remote_path, &cb)
    } else if path.is_file() {
        let filename = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        sftp_service::upload_file(&profile, &local_path, &remote_path, &|done, total| {
            let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: filename.clone() });
        })
    } else {
        Err(format!(
            "Cannot upload '{}': path does not exist or is not a file or directory",
            local_path
        ))
    }
}

/// Recursively download a remote directory to a local destination path.
#[tauri::command]
pub fn download_directory(
    profile_id: String,
    remote_path: String,
    local_path: String,
    on_progress: Channel<TransferProgress>,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    let cb = |done: u64, total: u64, name: &str| {
        let _ = on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, filename: name.to_string() });
    };
    if is_ftp(&profile) {
        ftp_service::download_directory(&profile, &remote_path, &local_path, &cb)
    } else {
        sftp_service::download_directory(&profile, &remote_path, &local_path, &cb)
    }
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
