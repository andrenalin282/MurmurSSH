use std::path::{Path, PathBuf};

use crate::models::{CredentialStorageMode, FileEntry};
use crate::services::profile_service;

// ── Security helpers ───────────────────────────────────────────────────────────

/// Reject paths that contain null bytes — these can bypass subsequent checks.
fn reject_null_bytes(path: &str) -> Result<(), String> {
    if path.contains('\0') {
        return Err("Path contains null bytes".to_string());
    }
    Ok(())
}

/// Validate and canonicalize a local directory path.
///
/// Returns the canonical path after:
/// - Rejecting null bytes
/// - Resolving symlinks via `canonicalize()`
/// - Confirming the result is a directory
fn validated_dir(path: &str) -> Result<PathBuf, String> {
    reject_null_bytes(path)?;
    let p = PathBuf::from(path);
    let canonical = p
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path '{}': {}", path, e))?;
    if !canonical.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }
    Ok(canonical)
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Return the current user's home directory ($HOME).
/// Falls back to "/" if $HOME is not set.
pub fn get_home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
}

/// Return the current OS username ($USER or $LOGNAME).
/// Returns "unknown" if neither env var is set.
pub fn get_current_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// List a local directory.
///
/// Returns entries sorted: directories first, then files, both alphabetically.
/// Hidden files (starting with '.') are included.
pub fn list_local_directory(path: &str) -> Result<Vec<FileEntry>, String> {
    let canonical = validated_dir(path)?;

    let read_dir = std::fs::read_dir(&canonical)
        .map_err(|e| format!("Cannot read directory '{}': {}", canonical.display(), e))?;

    let mut entries: Vec<FileEntry> = Vec::new();

    for entry_result in read_dir {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue, // Skip unreadable entries rather than failing the whole list
        };

        let name = entry.file_name().to_string_lossy().to_string();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => {
                // Cannot stat the entry — include it with unknown size/time
                entries.push(FileEntry {
                    name,
                    is_dir: false,
                    size: None,
                    modified: None,
                });
                continue;
            }
        };

        let is_dir = meta.is_dir();
        let size = if is_dir { None } else { Some(meta.len()) };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        entries.push(FileEntry {
            name,
            is_dir,
            size,
            modified,
        });
    }

    // Sort: directories first, then by name (case-insensitive)
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

/// Return the local browser start path for the current user and profile.
///
/// Resolution order:
/// 1. For portable profiles: `local_paths_by_user[$USER]`
/// 2. For local-machine profiles: `profile.local_path`
/// 3. Fallback: `$HOME`
///
/// If the stored path no longer exists on disk the fallback is used silently.
pub fn get_local_browser_path(profile_id: &str) -> Result<String, String> {
    let profile = profile_service::get_profile(profile_id)?;
    let home = get_home_dir();

    let is_portable = profile.credential_storage_mode.as_ref()
        == Some(&CredentialStorageMode::PortableProfile);

    let stored: Option<String> = if is_portable {
        let user = get_current_user();
        profile
            .local_paths_by_user
            .as_ref()
            .and_then(|m| m.get(&user))
            .cloned()
    } else {
        profile.local_path.clone()
    };

    // Validate the stored path still exists; fall back to $HOME silently
    if let Some(ref p) = stored {
        if Path::new(p).is_dir() {
            return Ok(p.clone());
        }
    }

    Ok(home)
}

/// Rename (or move within the same directory) a local file or directory.
///
/// Both paths must be absolute, free of null bytes, and share the same parent directory.
pub fn rename_local_file(from_path: &str, to_path: &str) -> Result<(), String> {
    reject_null_bytes(from_path)?;
    reject_null_bytes(to_path)?;

    if !from_path.starts_with('/') || !to_path.starts_with('/') {
        return Err("Only absolute paths are accepted".to_string());
    }

    let from = Path::new(from_path);
    let to = Path::new(to_path);

    if from.parent() != to.parent() {
        return Err("Rename may only change the file name, not the directory".to_string());
    }

    if !from.exists() {
        return Err(format!("'{}' does not exist", from_path));
    }

    std::fs::rename(from, to)
        .map_err(|e| format!("Rename failed: {}", e))
}

/// Open a local file with xdg-open or a custom editor command.
///
/// If `editor` is Some and non-empty, the first whitespace-separated token is
/// treated as the program and the rest as additional arguments, followed by
/// `path`. This matches the behaviour of `workspace_service::open_in_editor`
/// so `"code --new-window"` and similar settings work consistently.
///
/// Otherwise falls back to `xdg-open <path>`.
pub fn open_local_file(path: &str, editor: Option<&str>) -> Result<(), String> {
    reject_null_bytes(path)?;

    if !path.starts_with('/') {
        return Err("Only absolute paths are accepted".to_string());
    }

    match editor {
        Some(e) if !e.trim().is_empty() => {
            let mut parts = e.split_whitespace();
            let cmd = parts.next().ok_or("Editor command is empty")?;
            let extra_args: Vec<&str> = parts.collect();
            std::process::Command::new(cmd)
                .args(&extra_args)
                .arg(path)
                .spawn()
                .map_err(|err| format!("Failed to open '{}' with '{}': {}", path, e, err))?;
        }
        _ => {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|e| format!("Failed to open '{}' with xdg-open: {}", path, e))?;
        }
    }

    Ok(())
}

/// Persist the local browser path for the current user and profile.
///
/// - Portable profiles: written into `local_paths_by_user[$USER]`
/// - Local-machine profiles: written into `local_path`
pub fn save_local_browser_path(profile_id: &str, path: &str) -> Result<(), String> {
    reject_null_bytes(path)?;

    // Only accept absolute paths to avoid accidental relative paths leaking
    if !path.starts_with('/') {
        return Err("Only absolute paths are accepted".to_string());
    }

    // Verify the path is a real directory (canonicalize handles symlinks)
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path: {}", e))?;
    if !canonical.is_dir() {
        return Err("Path is not a directory".to_string());
    }
    let canonical_str = canonical
        .to_str()
        .ok_or_else(|| "Path contains non-UTF-8 characters".to_string())?
        .to_string();

    let mut profile = profile_service::get_profile(profile_id)?;

    let is_portable = profile.credential_storage_mode.as_ref()
        == Some(&CredentialStorageMode::PortableProfile);

    if is_portable {
        let user = get_current_user();
        let map = profile.local_paths_by_user.get_or_insert_with(Default::default);
        map.insert(user, canonical_str);
    } else {
        profile.local_path = Some(canonical_str);
    }

    profile_service::save_profile(&profile)
}
