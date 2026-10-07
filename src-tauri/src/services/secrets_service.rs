//! Local-machine secret storage for persistent (but non-portable) credential retention.
//!
//! Secrets are stored as plaintext files in:
//!   ~/.config/murmurssh/secrets/<profile_id>
//!
//! File permissions are set to 0600 (owner read/write only).
//!
//! SECURITY NOTE: This is NOT encrypted. It is machine-local only because the file
//! does not travel with the profile JSON. Anyone with filesystem read access to the
//! user's home directory can read the stored secret. This is intentionally labeled
//! as a lower-security option compared to never storing the secret at all.

use std::path::PathBuf;

fn secrets_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("murmurssh")
        .join("secrets")
}

/// Read the stored secret for a profile, if any.
pub fn get(profile_id: &str) -> Option<String> {
    crate::services::fs_secure::check_id(profile_id).ok()?;
    let path = secrets_dir().join(profile_id);
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Write a secret for a profile to disk, with 0600 permissions.
pub fn set(profile_id: &str, secret: &str) -> Result<(), String> {
    crate::services::fs_secure::check_id(profile_id)?;
    let dir = secrets_dir();
    crate::services::fs_secure::private_dir_all(&dir)
        .map_err(|e| format!("Failed to create secrets directory: {}", e))?;

    // Created 0600 from the start and renamed into place: never readable by others.
    crate::services::fs_secure::write_private(&dir.join(profile_id), secret.as_bytes())
}

/// Delete the stored secret for a profile. Silently succeeds if no file exists.
pub fn delete(profile_id: &str) {
    if crate::services::fs_secure::check_id(profile_id).is_err() {
        return;
    }
    let path = secrets_dir().join(profile_id);
    let _ = std::fs::remove_file(path);
}
