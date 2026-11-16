//! Tauri commands for the credential encryption key (Settings → Verschlüsselung).

use crate::services::crypto_service;
use serde::Serialize;

#[derive(Serialize)]
pub struct EncryptionStatus {
    /// Whether a usable key file is present.
    pub enabled: bool,
    /// Resolved key file path (shown in the UI; never the key value itself).
    pub key_path: String,
    /// Whether a key file exists at that path, even if its content is invalid.
    pub file_present: bool,
}

#[tauri::command]
pub fn encryption_status() -> EncryptionStatus {
    EncryptionStatus {
        enabled: crypto_service::is_enabled(),
        key_path: crypto_service::key_path().to_string_lossy().to_string(),
        file_present: crypto_service::key_file_present(),
    }
}

/// Create a fresh key and encrypt all existing stored secrets. Returns the base64
/// key value to show the user once (to save and transport). Err if a key exists.
#[tauri::command]
pub fn create_encryption_key() -> Result<String, String> {
    let value = crypto_service::create_key()?;
    crypto_service::encrypt_all_existing()?;
    Ok(value)
}

/// Encrypt any still-plaintext stored secrets now (idempotent). Requires a key.
#[tauri::command]
pub fn encrypt_existing_now() -> Result<usize, String> {
    crypto_service::encrypt_all_existing()
}

/// Return the stored key as its base64 value (for export to another machine).
#[tauri::command]
pub fn export_encryption_key() -> Result<String, String> {
    crypto_service::export_key()
}

/// Install a key from its base64 value. Only installs — it never re-encrypts
/// existing local secrets (so a wrong value cannot destroy them); profiles
/// imported from the source machine decrypt once the matching key is present.
#[tauri::command]
pub fn import_encryption_key(value: String) -> Result<(), String> {
    crypto_service::import_key(&value)
}

/// Decrypt all stored secrets back to plaintext, then remove the key file.
#[tauri::command]
pub fn remove_encryption_key() -> Result<(), String> {
    // Only decrypt when the key is valid; a corrupt key file is just removed
    // (its ciphertext was never decryptable with it anyway), avoiding a dead-end.
    if crypto_service::is_enabled() {
        crypto_service::decrypt_all_existing()?;
    }
    crypto_service::remove_key_file()
}
