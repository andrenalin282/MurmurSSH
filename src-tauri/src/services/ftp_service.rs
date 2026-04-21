//! FTP file transfer service.
//!
//! Provides the same surface as `sftp_service` for the commands layer.
//! Credentials come from the session store (same as SFTP password auth).
//! Only plain FTP is supported — no FTPS/TLS.

use std::io::{Cursor, Read, Write};

use suppaftp::FtpStream;

use crate::models::{FileEntry, Profile};
use crate::services::{credentials_store, transfer_cancel};

/// 256 KB chunks — same granularity as SFTP transfers.
const FTP_CHUNK: usize = 256 * 1024;

/// Stream a remote file to `local_path` in fixed-size chunks, polling the
/// cancel flag between chunks. Invokes `on_progress` each chunk. Cleans up the
/// partial local file on any error or cancel.
///
/// The caller must hold an authenticated `FtpStream`; the stream must be in a
/// state where `retr_as_stream` can be issued. On return the stream is always
/// finalised or aborted, so further commands can be issued on it.
fn retr_stream_to_file(
    ftp: &mut FtpStream,
    profile_id: &str,
    remote_path: &str,
    local_path: &str,
    name: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let total = ftp.size(remote_path).unwrap_or(0) as u64;
    on_progress(0, total, name);

    let mut local = std::fs::File::create(local_path)
        .map_err(|e| format!("Cannot create '{}': {}", local_path, e))?;

    let mut stream = match ftp.retr_as_stream(remote_path) {
        Ok(s) => s,
        Err(e) => {
            drop(local);
            let _ = std::fs::remove_file(local_path);
            return Err(format!("FTP download '{}' failed: {}", remote_path, e));
        }
    };

    let mut buf = vec![0u8; FTP_CHUNK];
    let mut done = 0u64;
    let transfer_result: Result<(), String> = loop {
        if transfer_cancel::is_cancelled(profile_id) {
            break Err(transfer_cancel::CANCELLED_ERROR.to_string());
        }
        let n = match stream.read(&mut buf) {
            Ok(v) => v,
            Err(e) => break Err(format!("FTP read '{}' failed: {}", remote_path, e)),
        };
        if n == 0 { break Ok(()); }
        if let Err(e) = local.write_all(&buf[..n]) {
            break Err(format!("Cannot write '{}': {}", local_path, e));
        }
        done += n as u64;
        on_progress(done, total, name);
    };

    match transfer_result {
        Ok(()) => {
            let _ = ftp.finalize_retr_stream(stream);
            Ok(())
        }
        Err(e) => {
            // `abort` sends ABOR + drops the data channel so the control
            // connection stays usable for recursive operations.
            let _ = ftp.abort(stream);
            drop(local);
            let _ = std::fs::remove_file(local_path);
            Err(e)
        }
    }
}

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

/// Upload a local file to a remote path by streaming directly from the file
/// handle — avoids loading the entire file into RAM (audit F8 partial).
/// `on_progress(bytes_done, bytes_total, filename)` is called at start and end.
pub fn upload_file(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let mut file = std::fs::File::open(local_path)
        .map_err(|e| format!("Cannot read local file '{}': {}", local_path, e))?;
    let total = file.metadata().map(|m| m.len()).unwrap_or(0);
    let name = std::path::Path::new(remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    on_progress(0, total, &name);
    let mut ftp = connect(profile)?;
    if let Err(e) = ftp.put_file(remote_path, &mut file) {
        let _ = ftp.rm(remote_path);
        let _ = ftp.quit();
        return Err(format!("FTP upload failed: {}", e));
    }
    let _ = ftp.quit();
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
    transfer_cancel::clear(&profile.id);
    let name = std::path::Path::new(remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut ftp = match connect(profile) {
        Ok(f) => f,
        Err(e) => { transfer_cancel::clear(&profile.id); return Err(e); }
    };
    let result = retr_stream_to_file(&mut ftp, &profile.id, remote_path, local_path, &name, on_progress);
    let _ = ftp.quit();
    transfer_cancel::clear(&profile.id);
    result
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
///
/// Progress semantics match single-file and SFTP directory operations:
/// `bytes_done` resets to 0 for each file, `bytes_total` is the file's size.
pub fn upload_directory(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    transfer_cancel::clear(&profile.id);
    let mut ftp = match connect(profile) {
        Ok(f) => f,
        Err(e) => { transfer_cancel::clear(&profile.id); return Err(e); }
    };
    let result = upload_dir_recursive(&profile.id, &mut ftp, std::path::Path::new(local_path), remote_path, on_progress);
    let _ = ftp.quit();
    transfer_cancel::clear(&profile.id);
    result
}

fn upload_dir_recursive(
    profile_id: &str,
    ftp: &mut FtpStream,
    local_dir: &std::path::Path,
    remote_dir: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    if transfer_cancel::is_cancelled(profile_id) {
        return Err(transfer_cancel::CANCELLED_ERROR.to_string());
    }
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

        if transfer_cancel::is_cancelled(profile_id) {
            return Err(transfer_cancel::CANCELLED_ERROR.to_string());
        }
        if local_entry.is_dir() {
            upload_dir_recursive(profile_id, ftp, &local_entry, &remote_entry, on_progress)?;
        } else if local_entry.is_file() {
            // Stream from the open file handle directly (F8 partial).
            let mut file = std::fs::File::open(&local_entry)
                .map_err(|e| format!("Cannot read '{}': {}", local_entry.display(), e))?;
            let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
            on_progress(0, file_size, &name);
            if let Err(e) = ftp.put_file(&remote_entry, &mut file) {
                let _ = ftp.rm(&remote_entry);
                return Err(format!("FTP upload '{}' failed: {}", remote_entry, e));
            }
            on_progress(file_size, file_size, &name);
        }
        // Broken symlinks and special files are skipped.
    }

    Ok(())
}

/// Recursively download a remote directory to a local destination path.
///
/// Progress semantics: per-file, matching single-file and SFTP directory ops.
pub fn download_directory(
    profile: &Profile,
    remote_path: &str,
    local_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    transfer_cancel::clear(&profile.id);
    let mut ftp = match connect(profile) {
        Ok(f) => f,
        Err(e) => { transfer_cancel::clear(&profile.id); return Err(e); }
    };
    let result = download_dir_recursive(&profile.id, &mut ftp, remote_path, local_path, on_progress);
    let _ = ftp.quit();
    transfer_cancel::clear(&profile.id);
    result
}

fn download_dir_recursive(
    profile_id: &str,
    ftp: &mut FtpStream,
    remote_dir: &str,
    local_dir: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    if transfer_cancel::is_cancelled(profile_id) {
        return Err(transfer_cancel::CANCELLED_ERROR.to_string());
    }
    std::fs::create_dir_all(local_dir)
        .map_err(|e| format!("Failed to create local directory '{}': {}", local_dir, e))?;

    let lines = ftp
        .list(Some(remote_dir))
        .map_err(|e| format!("FTP LIST '{}' failed: {}", remote_dir, e))?;

    for line in &lines {
        if let Some(entry) = parse_list_line(line) {
            let remote_entry = format!("{}/{}", remote_dir.trim_end_matches('/'), entry.name);
            let local_entry = format!("{}/{}", local_dir.trim_end_matches('/'), entry.name);

            if transfer_cancel::is_cancelled(profile_id) {
                return Err(transfer_cancel::CANCELLED_ERROR.to_string());
            }
            if entry.is_dir {
                download_dir_recursive(profile_id, ftp, &remote_entry, &local_entry, on_progress)?;
            } else {
                // Stream each file so recursive folder downloads do not balloon
                // RAM on large payloads and still honour cancel between chunks.
                retr_stream_to_file(ftp, profile_id, &remote_entry, &local_entry, &entry.name, on_progress)?;
            }
        }
    }

    Ok(())
}
