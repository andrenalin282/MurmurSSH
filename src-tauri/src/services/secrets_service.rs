//! Local-machine secret storage for persistent (but non-portable) credential retention.
//!
//! Secrets are stored as plaintext files in:
//!   ~/.config/murmurssh/secrets/<profile_id>
//!
//! File permissions are set to 0600 (owner read/write only).
//!
//! SECURITY NOTE: The stored value is machine-local (the file does not travel with the
//! profile JSON). When an encryption key is configured it is AES-256-GCM ciphertext
//! (`enc:v1:…`, see `crypto_service`); without a key it is plaintext. `get`/`set` store the
//! value verbatim — encryption/decryption happens at the call sites (save_credential /
//! connect flow). Plaintext here is readable by anyone with filesystem access to it.

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

/// List profile ids that currently have a stored secret file.
pub fn list_ids() -> Vec<String> {
    let dir = secrets_dir();
    let mut ids = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if crate::services::fs_secure::valid_id(name) {
                    ids.push(name.to_string());
                }
            }
        }
    }
    ids
}

/// Delete the stored secret for a profile. Silently succeeds if no file exists.
pub fn delete(profile_id: &str) {
    if crate::services::fs_secure::check_id(profile_id).is_err() {
        return;
    }
    let path = secrets_dir().join(profile_id);
    let _ = std::fs::remove_file(path);
}
