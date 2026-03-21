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

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn secrets_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("murmurssh")
        .join("secrets")
}

/// Read the stored secret for a profile, if any.
pub fn get(profile_id: &str) -> Option<String> {
    let path = secrets_dir().join(profile_id);
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Write a secret for a profile to disk, with 0600 permissions.
pub fn set(profile_id: &str, secret: &str) -> Result<(), String> {
    let dir = secrets_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create secrets directory: {}", e))?;

    let path = dir.join(profile_id);
    std::fs::write(&path, secret)
        .map_err(|e| format!("Failed to write secret file: {}", e))?;

    // Restrict to owner read/write only
    #[cfg(unix)]
    {
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms)
            .map_err(|e| format!("Failed to set secret file permissions: {}", e))?;
    }

    Ok(())
}

/// Delete the stored secret for a profile. Silently succeeds if no file exists.
pub fn delete(profile_id: &str) {
    let path = secrets_dir().join(profile_id);
    let _ = std::fs::remove_file(path);
}
