//! Symmetrische Verschlüsselung gespeicherter Credentials (AES-256-GCM).
//!
//! Gespeicherte Geheimwerte tragen das Präfix `enc:v1:` gefolgt von
//! base64(nonce ‖ ciphertext+tag). Fehlt das Präfix, ist der Wert Legacy-Klartext
//! und wird unverändert durchgereicht (abwärtskompatibel).
//!
//! Der 32-Byte-Schlüssel liegt als base64-Text in einer eigenen 0600-Datei
//! (`key_path()`), standardmäßig `~/.config/murmurssh/master.key`. In `settings.json`
//! steht höchstens der PFAD (`encryption_key_path`), nie der Schlüssel selbst.

#[cfg(test)]
pub(crate) static TEST_KEY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use crate::models::CredentialStorageMode;
use zeroize::Zeroizing;

const PREFIX: &str = "enc:v1:";
const NONCE_LEN: usize = 12;

fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config").join("murmurssh")
}

/// Default location of the key file when settings give no explicit path.
pub fn default_key_path() -> PathBuf {
    config_dir().join("master.key")
}

/// Resolve the key file path: test override → settings → default.
pub fn key_path() -> PathBuf {
    #[cfg(test)]
    if let Ok(p) = std::env::var("MURMURSSH_TEST_KEY_PATH") {
        return PathBuf::from(p);
    }
    if let Ok(s) = crate::services::settings_service::get_settings() {
        if let Some(p) = s.encryption_key_path {
            if !p.trim().is_empty() {
                return PathBuf::from(p);
            }
        }
    }
    default_key_path()
}

/// True if a value is in the encrypted `enc:v1:` form.
pub fn is_ciphertext(s: &str) -> bool {
    s.starts_with(PREFIX)
}

// --- pure core (no I/O) ---

/// Decode a base64 key value into exactly 32 bytes.
pub fn decode_key(value: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    let bytes = Zeroizing::new(
        STANDARD
            .decode(value.trim())
            .map_err(|_| "Invalid key value (not base64)".to_string())?,
    );
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| "Key must be exactly 32 bytes (256 bit)".to_string())?;
    Ok(Zeroizing::new(arr))
}

/// Encrypt plaintext with a 32-byte key: `enc:v1:` + base64(nonce ‖ ciphertext+tag).
pub fn encrypt_with_key(key: &[u8; 32], plaintext: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| "Encryption failed".to_string())?;
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ct);
    Ok(format!("{}{}", PREFIX, STANDARD.encode(blob)))
}

/// Decrypt a stored value. No `enc:v1:` prefix → returned unchanged (legacy plaintext).
pub fn decrypt_with_key(key: &[u8; 32], stored: &str) -> Result<String, String> {
    let body = match stored.strip_prefix(PREFIX) {
        Some(b) => b,
        None => return Ok(stored.to_string()),
    };
    let blob = STANDARD
        .decode(body)
        .map_err(|_| "Invalid ciphertext data".to_string())?;
    if blob.len() <= NONCE_LEN {
        return Err("Ciphertext too short".to_string());
    }
    let (nonce_bytes, ct) = blob.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let pt = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ct)
        .map_err(|_| "Decryption failed (wrong key or corrupted data)".to_string())?;
    String::from_utf8(pt).map_err(|_| "Decrypted value is not valid UTF-8".to_string())
}

// --- file layer ---

/// Load the master key from disk, if present and valid.
pub fn load_key() -> Option<Zeroizing<[u8; 32]>> {
    let raw = Zeroizing::new(std::fs::read_to_string(key_path()).ok()?);
    decode_key(&raw).ok()
}

/// True if a usable (valid) key is present.
pub fn is_enabled() -> bool {
    load_key().is_some()
}

/// True if a key FILE exists at the resolved path, even if its content is invalid.
/// Used so a corrupt key file can still be removed via the UI.
pub fn key_file_present() -> bool {
    key_path().exists()
}

/// Encrypt with the stored key. Err if no key is set.
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    let key = load_key().ok_or_else(|| "No encryption key set".to_string())?;
    encrypt_with_key(&key, plaintext)
}

/// Decrypt a stored value. Plaintext passes through; ciphertext without a key
/// yields `NEED_ENCRYPTION_KEY`.
pub fn decrypt(stored: &str) -> Result<String, String> {
    if !is_ciphertext(stored) {
        return Ok(stored.to_string());
    }
    let key = load_key().ok_or_else(|| "NEED_ENCRYPTION_KEY".to_string())?;
    decrypt_with_key(&key, stored)
}

/// Generate and store a fresh key. Err if one already exists. Returns the base64 value.
pub fn create_key() -> Result<String, String> {
    let path = key_path();
    if path.exists() {
        return Err("An encryption key already exists".to_string());
    }
    let mut key = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(&mut *key);
    let value = STANDARD.encode(key.as_slice());
    if let Some(dir) = path.parent() {
        let _ = crate::services::fs_secure::private_dir_all(dir);
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                "An encryption key already exists".to_string()
            } else {
                format!("Could not create key file: {}", e)
            }
        })?;
    file.write_all(value.as_bytes())
        .map_err(|e| format!("Could not write key file: {}", e))?;
    Ok(value)
}

/// Return the stored key as its base64 value. Err if no key is set.
pub fn export_key() -> Result<String, String> {
    std::fs::read_to_string(key_path())
        .map(|s| s.trim().to_string())
        .map_err(|_| "No key present".to_string())
}

/// Install a key from its base64 value (validated to 32 bytes).
pub fn import_key(value: &str) -> Result<(), String> {
    if key_path().exists() {
        return Err("An encryption key already exists; remove it first".to_string());
    }
    decode_key(value)?;
    write_key_file(&key_path(), value.trim())
}

/// Delete the key file (if any).
pub fn remove_key_file() -> Result<(), String> {
    let path = key_path();
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Could not remove key: {}", e))?;
    }
    Ok(())
}

fn write_key_file(path: &Path, value: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        let _ = crate::services::fs_secure::private_dir_all(dir);
    }
    crate::services::fs_secure::write_private(path, value.as_bytes())
}

/// For migration: given a stored value and direction, return `Some(new_value)` to
/// write, or `None` to leave it unchanged. Idempotent by construction.
fn migrate_value(key: &[u8; 32], stored: &str, encrypting: bool) -> Result<Option<String>, String> {
    match (encrypting, is_ciphertext(stored)) {
        (true, false) => Ok(Some(encrypt_with_key(key, stored)?)),
        (false, true) => Ok(Some(decrypt_with_key(key, stored)?)),
        _ => Ok(None),
    }
}

/// Encrypt every still-plaintext stored secret (portable profile fields and local
/// secret files). Idempotent. Returns how many values were changed.
pub fn encrypt_all_existing() -> Result<usize, String> {
    run_migration(true)
}

/// Decrypt every encrypted stored secret back to plaintext (used when removing the
/// key). Idempotent. Returns how many values were changed.
pub fn decrypt_all_existing() -> Result<usize, String> {
    run_migration(false)
}

fn run_migration(encrypting: bool) -> Result<usize, String> {
    // Load the key once (not per value) — faster migration.
    let key = load_key().ok_or_else(|| "No encryption key set".to_string())?;
    let mut changed = 0usize;
    for mut profile in crate::services::profile_service::list_profiles()? {
        if let Some(secret) = profile.stored_secret_portable.clone() {
            // On decrypt (key removal) a value we cannot decrypt (foreign key / corrupt)
            // is skipped, not fatal — it was never usable with this key anyway.
            let step = match migrate_value(&key, &secret, encrypting) {
                Ok(v) => v,
                Err(e) if encrypting => return Err(e),
                Err(_) => None,
            };
            if let Some(new_value) = step {
                profile.stored_secret_portable = Some(new_value);
                crate::services::profile_service::save_profile(&profile)?;
                if encrypting {
                    crate::services::profile_service::delete_backup(&profile.id);
                }
                changed += 1;
            }
        }
    }
    for id in crate::services::secrets_service::list_ids() {
        if let Some(secret) = crate::services::secrets_service::get(&id) {
            let step = match migrate_value(&key, &secret, encrypting) {
                Ok(v) => v,
                Err(e) if encrypting => return Err(e),
                Err(_) => None,
            };
            if let Some(new_value) = step {
                crate::services::secrets_service::set(&id, &new_value)?;
                changed += 1;
            }
        }
    }
    Ok(changed)
}

/// Resolve a profile's stored credential to plaintext, decrypting if needed.
/// `Err("NEED_ENCRYPTION_KEY")` when the value is encrypted but no key is available.
/// Shared by the connect flow and the AI server.
pub fn resolve_stored_secret(
    mode: &CredentialStorageMode,
    portable: Option<String>,
    profile_id: &str,
) -> Result<Option<String>, String> {
    let raw = match mode {
        CredentialStorageMode::LocalMachine => crate::services::secrets_service::get(profile_id),
        CredentialStorageMode::PortableProfile => portable,
        CredentialStorageMode::Never => None,
    };
    match raw {
        None => Ok(None),
        Some(value) => decrypt(&value).map(Some),
    }
}

/// Encrypt a secret for storage. Plaintext only when NO key file exists; if a key
/// file is present it MUST be usable, otherwise an error (never a silent plaintext fallback).
pub fn encrypt_for_storage(plaintext: &str) -> Result<String, String> {
    if !key_path().exists() {
        return Ok(plaintext.to_string());
    }
    // A key file is present: it MUST be usable. Never silently store plaintext.
    encrypt(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};

    const KEY: [u8; 32] = [7u8; 32];

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let ct = encrypt_with_key(&KEY, "s3cret").unwrap();
        assert!(is_ciphertext(&ct));
        assert_eq!(decrypt_with_key(&KEY, &ct).unwrap(), "s3cret");
    }

    #[test]
    fn two_encryptions_differ_but_decrypt_same() {
        let a = encrypt_with_key(&KEY, "pw").unwrap();
        let b = encrypt_with_key(&KEY, "pw").unwrap();
        assert_ne!(a, b, "nonce must be random per value");
        assert_eq!(decrypt_with_key(&KEY, &a).unwrap(), "pw");
        assert_eq!(decrypt_with_key(&KEY, &b).unwrap(), "pw");
    }

    #[test]
    fn decrypt_with_key_passes_through_plaintext() {
        assert_eq!(decrypt_with_key(&KEY, "plainpw").unwrap(), "plainpw");
    }

    #[test]
    fn tampered_ciphertext_errs() {
        let ct = encrypt_with_key(&KEY, "pw").unwrap();
        let i = ct.len() - 1;
        let repl = if &ct[i..] == "A" { "B" } else { "A" };
        let mut bad = ct.clone();
        bad.replace_range(i.., repl);
        assert!(decrypt_with_key(&KEY, &bad).is_err());
    }

    #[test]
    fn wrong_key_errs() {
        let ct = encrypt_with_key(&KEY, "pw").unwrap();
        assert!(decrypt_with_key(&[9u8; 32], &ct).is_err());
    }

    #[test]
    fn decode_key_valid_and_invalid() {
        let v = STANDARD.encode(KEY);
        assert_eq!(*decode_key(&v).unwrap(), KEY);
        assert!(decode_key("not-32-bytes").is_err());
    }

    #[test]
    fn decrypt_ciphertext_without_key_errs() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!("murmur-nokey-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &tmp);
        let ct = encrypt_with_key(&KEY, "pw").unwrap();
        assert_eq!(decrypt(&ct).unwrap_err(), "NEED_ENCRYPTION_KEY");
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn create_import_export_load_round_trip() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!("murmur-key-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &tmp);
        assert!(!is_enabled());
        let val = create_key().unwrap();
        assert!(is_enabled());
        assert_eq!(export_key().unwrap(), val);
        let ct = encrypt("pw").unwrap();
        assert_eq!(decrypt(&ct).unwrap(), "pw");
        remove_key_file().unwrap();
        assert!(!is_enabled());
        // import installs the key when none is present
        import_key(&val).unwrap();
        assert!(is_enabled());
        assert_eq!(export_key().unwrap(), val);
        remove_key_file().unwrap();
        assert!(!is_enabled());
        let _ = std::fs::remove_file(&tmp);
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn create_key_fails_if_exists() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!("murmur-key2-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &tmp);
        create_key().unwrap();
        assert!(create_key().is_err());
        let _ = std::fs::remove_file(&tmp);
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn migrate_encrypt_plaintext_then_decryptable() {
        const K: [u8; 32] = [5u8; 32];
        let out = migrate_value(&K, "plainpw", true).unwrap().expect("should change");
        assert!(is_ciphertext(&out));
        assert_eq!(decrypt_with_key(&K, &out).unwrap(), "plainpw");
    }

    #[test]
    fn migrate_encrypt_is_idempotent() {
        const K: [u8; 32] = [5u8; 32];
        let ct = encrypt_with_key(&K, "pw").unwrap();
        assert_eq!(migrate_value(&K, &ct, true).unwrap(), None);
    }

    #[test]
    fn migrate_decrypt_ciphertext_restores_plaintext() {
        const K: [u8; 32] = [5u8; 32];
        let ct = encrypt_with_key(&K, "pw").unwrap();
        assert_eq!(migrate_value(&K, &ct, false).unwrap(), Some("pw".to_string()));
        assert_eq!(migrate_value(&K, "plainpw", false).unwrap(), None);
    }

    #[test]
    fn migrate_decrypt_wrong_key_errs() {
        const K: [u8; 32] = [5u8; 32];
        const OTHER: [u8; 32] = [9u8; 32];
        let ct = encrypt_with_key(&K, "pw").unwrap();
        assert!(migrate_value(&OTHER, &ct, false).is_err());
    }


    #[test]
    fn resolve_returns_none_for_never() {
        assert_eq!(
            resolve_stored_secret(&CredentialStorageMode::Never, Some("x".into()), "pid").unwrap(),
            None
        );
    }

    #[test]
    fn resolve_passes_through_plaintext_portable() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-rs-plain-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        assert_eq!(
            resolve_stored_secret(&CredentialStorageMode::PortableProfile, Some("plainpw".into()), "pid").unwrap(),
            Some("plainpw".to_string())
        );
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn resolve_decrypts_when_key_set() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-rs-set-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        create_key().unwrap();
        let ct = encrypt("pw").unwrap();
        assert_eq!(
            resolve_stored_secret(&CredentialStorageMode::PortableProfile, Some(ct), "pid").unwrap(),
            Some("pw".to_string())
        );
        remove_key_file().unwrap();
        let _ = std::fs::remove_file(&p);
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn resolve_errs_need_key_for_ciphertext_without_key() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-rs-nokey-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        create_key().unwrap();
        let ct = encrypt("pw").unwrap();
        remove_key_file().unwrap();
        assert_eq!(
            resolve_stored_secret(&CredentialStorageMode::PortableProfile, Some(ct), "pid").unwrap_err(),
            "NEED_ENCRYPTION_KEY"
        );
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn import_rejects_when_key_exists() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-imp-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        let a = create_key().unwrap();
        // importing a different value must NOT silently overwrite the active key
        let other = STANDARD.encode([3u8; 32]);
        assert!(import_key(&other).is_err());
        assert_eq!(export_key().unwrap(), a, "active key must be unchanged");
        remove_key_file().unwrap();
        let _ = std::fs::remove_file(&p);
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn encrypt_for_storage_plaintext_without_key() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-efs-none-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        assert_eq!(encrypt_for_storage("pw").unwrap(), "pw");
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn encrypt_for_storage_encrypts_with_key() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-efs-key-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        create_key().unwrap();
        let out = encrypt_for_storage("pw").unwrap();
        assert!(is_ciphertext(&out));
        remove_key_file().unwrap();
        let _ = std::fs::remove_file(&p);
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn encrypt_for_storage_errs_when_key_file_unreadable() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-efs-bad-{}.key", std::process::id()));
        std::fs::write(&p, b"not-valid-base64-key").unwrap();
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        assert!(encrypt_for_storage("pw").is_err(), "must not silently store plaintext");
        let _ = std::fs::remove_file(&p);
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

}
