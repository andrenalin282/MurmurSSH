//! FTP file transfer service.
//!
//! Provides the same surface as `sftp_service` for the commands layer.
//! Credentials come from the session store (same as SFTP password auth).
//! Only plain FTP is supported — no FTPS/TLS.

use std::io::Cursor;

use suppaftp::FtpStream;

use crate::models::{FileEntry, Profile};
use crate::services::credentials_store;

/// Open an authenticated FTP connection.
///
/// Returns `Err("NEED_PASSWORD")` when no password is in the session store.
fn connect(profile: &Profile) -> Result<FtpStream, String> {
    let creds = credentials_store::get(&profile.id);
    let password = creds
        .password
        .ok_or_else(|| "NEED_PASSWORD".to_string())?;

    let addr = format!("{}:{}", profile.host, profile.port);
    let mut ftp = FtpStream::connect(&addr)
        .map_err(|e| format!("FTP connection failed: {}", e))?;

    ftp.login(&profile.username, &password)
        .map_err(|e| format!("FTP login failed: {}", e))?;

    Ok(ftp)
}

/// Try connecting and immediately quit — used by `connect_sftp` to verify the profile.
pub fn test_connection(profile: &Profile) -> Result<(), String> {
    let mut ftp = connect(profile)?;
    let _ = ftp.quit();
    Ok(())
}

/// Return "/" as the home directory for FTP profiles.
pub fn get_home(_profile: &Profile) -> Result<String, String> {
    Ok("/".to_string())
}

/// Parse a single line from a UNIX-format LIST response.
///
/// Format: `drwxr-xr-x  2 user group 4096 Jan  1 12:00 name`
/// Returns `None` for lines that cannot be parsed.
fn parse_list_line(line: &str) -> Option<FileEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    // Minimum tokens: perms links user group size month day time/year name
    if parts.len() < 9 {
        return None;
    }
    let perms = parts[0];
    let is_dir = perms.starts_with('d');
    let size: Option<u64> = if is_dir {
        None
    } else {
        parts[4].parse().ok()
    };
    // Name is everything after the 8th token (index 8), joined with spaces
    let name = parts[8..].join(" ");
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    Some(FileEntry {
        name,
        is_dir,
        size,
        modified: None,
    })
}

/// List directory contents. Uses passive-mode LIST.
pub fn list_directory(profile: &Profile, path: &str) -> Result<Vec<FileEntry>, String> {
    let mut ftp = connect(profile)?;

    let lines = ftp
        .list(Some(path))
        .map_err(|e| format!("FTP LIST failed: {}", e))?;

    let _ = ftp.quit();

    let mut entries: Vec<FileEntry> = lines
        .iter()
        .filter_map(|line| parse_list_line(line))
        .collect();

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

/// Check whether a remote path exists by attempting NLST.
pub fn remote_file_exists(profile: &Profile, remote_path: &str) -> Result<bool, String> {
    let mut ftp = connect(profile)?;
    let exists = ftp.size(remote_path).is_ok();
    let _ = ftp.quit();
    Ok(exists)
}

/// Upload raw bytes to a remote path.
pub fn upload_bytes(profile: &Profile, remote_path: &str, content: &[u8]) -> Result<(), String> {
    let mut ftp = connect(profile)?;
    let mut cursor = Cursor::new(content);
    ftp.put_file(remote_path, &mut cursor)
        .map_err(|e| format!("FTP upload failed: {}", e))?;
    let _ = ftp.quit();
    Ok(())
}

/// Upload a local file to a remote path.
/// `on_progress(bytes_done, bytes_total, filename)` is called at start and end.
pub fn upload_file(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let content = std::fs::read(local_path)
        .map_err(|e| format!("Cannot read local file '{}': {}", local_path, e))?;
    let name = std::path::Path::new(remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let total = content.len() as u64;
    on_progress(0, total, &name);
    upload_bytes(profile, remote_path, &content)?;
    on_progress(total, total, &name);
    Ok(())
}

/// Download a remote file and write it to a local path.
/// `on_progress(bytes_done, bytes_total, filename)` is called at start and end.
pub fn download_file_to(
    profile: &Profile,
    remote_path: &str,
    local_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let name = std::path::Path::new(remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut ftp = connect(profile)?;
    // Try to get file size for total; 0 = unknown
    let total = ftp.size(remote_path).unwrap_or(0) as u64;
    on_progress(0, total, &name);
    let data = ftp
        .retr_as_buffer(remote_path)
        .map_err(|e| format!("FTP download failed: {}", e))?;
    let bytes = data.into_inner();
    let actual = bytes.len() as u64;
    let _ = ftp.quit();
    std::fs::write(local_path, &bytes)
        .map_err(|e| format!("Cannot write to '{}': {}", local_path, e))?;
    on_progress(actual, actual, &name);
    Ok(())
}

/// Delete a remote file.
pub fn delete_file(profile: &Profile, remote_path: &str) -> Result<(), String> {
    let mut ftp = connect(profile)?;
    ftp.rm(remote_path)
        .map_err(|e| format!("FTP delete failed: {}", e))?;
    let _ = ftp.quit();
    Ok(())
}

/// Rename or move a remote file.
pub fn rename_file(profile: &Profile, from: &str, to: &str) -> Result<(), String> {
    let mut ftp = connect(profile)?;
    ftp.rename(from, to)
        .map_err(|e| format!("FTP rename failed: {}", e))?;
    let _ = ftp.quit();
    Ok(())
}

/// Create a remote directory (non-recursive).
pub fn create_directory(profile: &Profile, path: &str) -> Result<(), String> {
    let mut ftp = connect(profile)?;
    ftp.mkdir(path)
        .map_err(|e| format!("FTP mkdir failed: {}", e))?;
    let _ = ftp.quit();
    Ok(())
}

/// Recursively delete a remote directory and all of its contents.
pub fn delete_directory(profile: &Profile, path: &str) -> Result<(), String> {
    let mut ftp = connect(profile)?;
    let result = delete_dir_recursive(&mut ftp, path);
    let _ = ftp.quit();
    result
}

fn delete_dir_recursive(ftp: &mut FtpStream, path: &str) -> Result<(), String> {
    let lines = ftp
        .list(Some(path))
        .map_err(|e| format!("FTP LIST '{}' failed: {}", path, e))?;

    for line in &lines {
        if let Some(entry) = parse_list_line(line) {
            let entry_path = format!("{}/{}", path.trim_end_matches('/'), entry.name);
            if entry.is_dir {
                delete_dir_recursive(ftp, &entry_path)?;
            } else {
                ftp.rm(&entry_path)
                    .map_err(|e| format!("FTP delete '{}' failed: {}", entry_path, e))?;
            }
        }
    }

    ftp.rmdir(path)
        .map_err(|e| format!("FTP rmdir '{}' failed: {}", path, e))
}

/// Recursively upload a local directory to a remote destination path.
pub fn upload_directory(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let mut ftp = connect(profile)?;
    let mut bytes_done = 0u64;
    let result = upload_dir_recursive(&mut ftp, std::path::Path::new(local_path), remote_path, &mut bytes_done, on_progress);
    let _ = ftp.quit();
    result
}

fn upload_dir_recursive(
    ftp: &mut FtpStream,
    local_dir: &std::path::Path,
    remote_dir: &str,
    bytes_done: &mut u64,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    // Create the remote directory; ignore error if it already exists.
    let _ = ftp.mkdir(remote_dir);

    let read_dir = std::fs::read_dir(local_dir)
        .map_err(|e| format!("Failed to read local directory '{}': {}", local_dir.display(), e))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Directory entry error: {}", e))?;
        let local_entry = entry.path();
        let name = local_entry
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let remote_entry = format!("{}/{}", remote_dir.trim_end_matches('/'), name);

        if local_entry.is_dir() {
            upload_dir_recursive(ftp, &local_entry, &remote_entry, bytes_done, on_progress)?;
        } else if local_entry.is_file() {
            let content = std::fs::read(&local_entry)
                .map_err(|e| format!("Cannot read '{}': {}", local_entry.display(), e))?;
            let file_size = content.len() as u64;
            on_progress(*bytes_done, 0, &name);
            let mut cursor = Cursor::new(&content);
            ftp.put_file(&remote_entry, &mut cursor)
                .map_err(|e| format!("FTP upload '{}' failed: {}", remote_entry, e))?;
            *bytes_done += file_size;
            on_progress(*bytes_done, 0, &name);
        }
        // Broken symlinks and special files are skipped.
    }

    Ok(())
}

/// Recursively download a remote directory to a local destination path.
pub fn download_directory(
    profile: &Profile,
    remote_path: &str,
    local_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let mut ftp = connect(profile)?;
    let mut bytes_done = 0u64;
    let result = download_dir_recursive(&mut ftp, remote_path, local_path, &mut bytes_done, on_progress);
    let _ = ftp.quit();
    result
}

fn download_dir_recursive(
    ftp: &mut FtpStream,
    remote_dir: &str,
    local_dir: &str,
    bytes_done: &mut u64,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    std::fs::create_dir_all(local_dir)
        .map_err(|e| format!("Failed to create local directory '{}': {}", local_dir, e))?;

    let lines = ftp
        .list(Some(remote_dir))
        .map_err(|e| format!("FTP LIST '{}' failed: {}", remote_dir, e))?;

    for line in &lines {
        if let Some(entry) = parse_list_line(line) {
            let remote_entry = format!("{}/{}", remote_dir.trim_end_matches('/'), entry.name);
            let local_entry = format!("{}/{}", local_dir.trim_end_matches('/'), entry.name);

            if entry.is_dir {
                download_dir_recursive(ftp, &remote_entry, &local_entry, bytes_done, on_progress)?;
            } else {
                on_progress(*bytes_done, 0, &entry.name);
                let data = ftp
                    .retr_as_buffer(&remote_entry)
                    .map_err(|e| format!("FTP download '{}' failed: {}", remote_entry, e))?;
                let bytes = data.into_inner();
                *bytes_done += bytes.len() as u64;
                std::fs::write(&local_entry, &bytes)
                    .map_err(|e| format!("Cannot write '{}': {}", local_entry, e))?;
                on_progress(*bytes_done, 0, &entry.name);
            }
        }
    }

    Ok(())
}
