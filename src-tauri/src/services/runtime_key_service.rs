/// Runtime SSH key copy service.
///
/// When a key file is stored on a mounted or network filesystem, OpenSSH's
/// `ssh` binary may reject it with "UNPROTECTED PRIVATE KEY FILE" because:
/// - the filesystem reports group/other read bits set (mode & 0o077 != 0), or
/// - the filesystem does not honour UNIX permission semantics at all.
///
/// The `ssh2` crate reads key bytes directly in-process and is not affected.
/// Only the system terminal launch (which forks the system `ssh` binary) fails.
///
/// This service offers a runtime-only local copy of the key file:
/// - stored in ~/.config/murmurssh/runtime-keys/<profile_id>
/// - created with 0600 permissions
/// - deleted on disconnect, quit, and startup cleanup
/// - the original key file is NEVER modified or moved
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn runtime_keys_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    PathBuf::from(home)
        .join(".config")
        .join("murmurssh")
        .join("runtime-keys")
}

fn runtime_key_path(profile_id: &str) -> PathBuf {
    runtime_keys_dir().join(profile_id)
}

/// Check whether the key file at `key_path` would be rejected by the system
/// OpenSSH client due to permissions or filesystem issues.
///
/// Returns true if:
/// - the file has group or other read/write/execute bits set (mode & 0o077 != 0)
///
/// This covers the most common cases. Mounted filesystems that ignore permissions
/// entirely (e.g. FAT, some SMB/CIFS mounts) are a separate case; for those,
/// the user can manually trigger the runtime copy by accepting the prompt even
/// when this check does not flag the key.
pub fn key_needs_runtime_copy(key_path: &str) -> bool {
    match fs::metadata(key_path) {
        Ok(meta) => {
            let mode = meta.permissions().mode();
            // OpenSSH rejects keys if group or other have any bits set (rwx).
            // 0o077 masks group (0o070) and other (0o007) permission bits.
            (mode & 0o077) != 0
        }
        Err(_) => false, // If we can't stat the file, don't force a copy
    }
}

/// Copy the key file at `key_path` to the runtime-keys directory for `profile_id`.
///
/// The copy is given 0600 permissions so OpenSSH accepts it regardless of the
/// source filesystem. Returns the path to the runtime copy on success.
///
/// The original key file is never modified.
pub fn copy_key_for_runtime(profile_id: &str, key_path: &str) -> Result<PathBuf, String> {
    let dir = runtime_keys_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create runtime-keys directory: {}", e))?;

    // Set directory permissions to 0700 so only the user can access it
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("Failed to set runtime-keys directory permissions: {}", e))?;

    let dest = runtime_key_path(profile_id);

    // Remove any stale copy from a previous session
    if dest.exists() {
        fs::remove_file(&dest)
            .map_err(|e| format!("Failed to remove stale runtime key: {}", e))?;
    }

    fs::copy(key_path, &dest)
        .map_err(|e| format!("Failed to copy key to runtime location: {}", e))?;

    fs::set_permissions(&dest, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("Failed to set runtime key permissions: {}", e))?;

    Ok(dest)
}

/// Return the path where the runtime key for a profile would be stored.
/// Does not check whether the file exists.
pub fn get_runtime_key_path(profile_id: &str) -> Option<PathBuf> {
    Some(runtime_key_path(profile_id))
}

/// Delete the runtime key for a specific profile.
/// Called on disconnect or quit. Non-fatal: errors are silently ignored.
pub fn delete_runtime_key(profile_id: &str) {
    let path = runtime_key_path(profile_id);
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
}

/// Delete all runtime keys from the runtime-keys directory.
/// Called on startup to remove leftovers from crashes or forced exits.
/// Non-fatal: errors are silently ignored.
pub fn cleanup_all_runtime_keys() {
    let dir = runtime_keys_dir();
    if !dir.exists() {
        return;
    }
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let _ = fs::remove_file(entry.path());
        }
    }
}
