//! Symmetrische Verschlüsselung gespeicherter Credentials (AES-256-GCM).
//!
//! Gespeicherte Geheimwerte tragen das Präfix `enc:v1:` gefolgt von
//! base64(nonce ‖ ciphertext+tag). Fehlt das Präfix, ist der Wert Legacy-Klartext
//! und wird unverändert durchgereicht (abwärtskompatibel).
//!
//! Der 32-Byte-Schlüssel liegt als base64-Text in einer eigenen 0600-Datei
//! (`key_path()`), standardmäßig `~/.config/murmurssh/master.key`. In `settings.json`
//! steht höchstens der PFAD (`encryption_key_path`), nie der Schlüssel selbst.

#![allow(dead_code)] // TODO Task 4: entfernen, sobald alle Funktionen verdrahtet sind

#[cfg(test)]
pub(crate) static TEST_KEY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::{Path, PathBuf};

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
pub fn decode_key(value: &str) -> Result<[u8; 32], String> {
    let bytes = STANDARD
        .decode(value.trim())
        .map_err(|_| "Ungültiger Schlüsselwert (kein Base64)".to_string())?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| "Schlüssel muss genau 32 Byte (256 Bit) sein".to_string())
}

/// Encrypt plaintext with a 32-byte key: `enc:v1:` + base64(nonce ‖ ciphertext+tag).
pub fn encrypt_with_key(key: &[u8; 32], plaintext: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| "Verschlüsselung fehlgeschlagen".to_string())?;
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
        .map_err(|_| "Ungültige Ciphertext-Daten".to_string())?;
    if blob.len() <= NONCE_LEN {
        return Err("Ciphertext zu kurz".to_string());
    }
    let (nonce_bytes, ct) = blob.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let pt = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ct)
        .map_err(|_| "Entschlüsselung fehlgeschlagen (falscher Schlüssel oder beschädigte Daten)".to_string())?;
    String::from_utf8(pt).map_err(|_| "Entschlüsselter Wert ist kein gültiges UTF-8".to_string())
}

// --- file layer ---

/// Load the master key from disk, if present and valid.
pub fn load_key() -> Option<[u8; 32]> {
    let raw = std::fs::read_to_string(key_path()).ok()?;
    decode_key(&raw).ok()
}

/// True if a usable key file is present.
pub fn is_enabled() -> bool {
    load_key().is_some()
}

/// Encrypt with the stored key. Err if no key is set.
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    let key = load_key().ok_or_else(|| "Kein Verschlüsselungsschlüssel gesetzt".to_string())?;
    encrypt_with_key(&key, plaintext)
}

/// Encrypt if a key is set, otherwise return the plaintext unchanged.
pub fn encrypt_if_enabled(plaintext: &str) -> String {
    encrypt(plaintext).unwrap_or_else(|_| plaintext.to_string())
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
        return Err("Es ist bereits ein Schlüssel vorhanden".to_string());
    }
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    let value = STANDARD.encode(key);
    write_key_file(&path, &value)?;
    Ok(value)
}

/// Return the stored key as its base64 value. Err if no key is set.
pub fn export_key() -> Result<String, String> {
    std::fs::read_to_string(key_path())
        .map(|s| s.trim().to_string())
        .map_err(|_| "Kein Schlüssel vorhanden".to_string())
}

/// Install a key from its base64 value (validated to 32 bytes).
pub fn import_key(value: &str) -> Result<(), String> {
    decode_key(value)?;
    write_key_file(&key_path(), value.trim())
}

/// Delete the key file (if any).
pub fn remove_key_file() -> Result<(), String> {
    let path = key_path();
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Schlüssel konnte nicht entfernt werden: {}", e))?;
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
fn migrate_value(stored: &str, encrypting: bool) -> Result<Option<String>, String> {
    match (encrypting, is_ciphertext(stored)) {
        (true, false) => Ok(Some(encrypt(stored)?)),
        (false, true) => Ok(Some(decrypt(stored)?)),
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
    if !is_enabled() {
        return Err("Kein Verschl\u{00fc}sselungsschl\u{00fc}ssel gesetzt".to_string());
    }
    let mut changed = 0usize;
    for mut profile in crate::services::profile_service::list_profiles()? {
        if let Some(secret) = profile.stored_secret_portable.clone() {
            if let Some(new_value) = migrate_value(&secret, encrypting)? {
                profile.stored_secret_portable = Some(new_value);
                crate::services::profile_service::save_profile(&profile)?;
                changed += 1;
            }
        }
    }
    for id in crate::services::secrets_service::list_ids() {
        if let Some(secret) = crate::services::secrets_service::get(&id) {
            if let Some(new_value) = migrate_value(&secret, encrypting)? {
                crate::services::secrets_service::set(&id, &new_value)?;
                changed += 1;
            }
        }
    }
    Ok(changed)
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
        assert_eq!(decode_key(&v).unwrap(), KEY);
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
        import_key(&val).unwrap();
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
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-mig1-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        create_key().unwrap();
        let out = migrate_value("plainpw", true).unwrap().expect("should change");
        assert!(is_ciphertext(&out));
        assert_eq!(decrypt(&out).unwrap(), "plainpw");
        remove_key_file().unwrap();
        let _ = std::fs::remove_file(&p);
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn migrate_encrypt_is_idempotent() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-mig2-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        create_key().unwrap();
        let ct = encrypt("pw").unwrap();
        assert_eq!(migrate_value(&ct, true).unwrap(), None);
        remove_key_file().unwrap();
        let _ = std::fs::remove_file(&p);
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

    #[test]
    fn migrate_decrypt_ciphertext_restores_plaintext() {
        let _g = TEST_KEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::temp_dir().join(format!("murmur-mig3-{}.key", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("MURMURSSH_TEST_KEY_PATH", &p);
        create_key().unwrap();
        let ct = encrypt("pw").unwrap();
        assert_eq!(migrate_value(&ct, false).unwrap(), Some("pw".to_string()));
        assert_eq!(migrate_value("plainpw", false).unwrap(), None);
        remove_key_file().unwrap();
        let _ = std::fs::remove_file(&p);
        std::env::remove_var("MURMURSSH_TEST_KEY_PATH");
    }

}
