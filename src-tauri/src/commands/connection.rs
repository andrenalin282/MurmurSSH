use crate::models::{AuthType, CredentialStorageMode, Protocol};
use crate::services::{
    credentials_store, ftp_service, known_hosts_service, profile_service, secrets_service,
    session_trust_store, sftp_service,
};

/// Verify a connection to the given profile, optionally providing runtime credentials.
///
/// Credential auto-load rules:
/// - Only `password` auth profiles may have persisted credentials. Passphrases
///   for SSH key auth are always runtime-only and are never loaded from storage.
/// - If a password credential is found in persistent storage (local file or portable
///   profile field), it is loaded into the session store before connecting.
///
/// Returns structured error strings the frontend handles:
/// - "UNKNOWN_HOST:<fingerprint>" — host key not yet trusted
/// - "HOST_MISMATCH:<stored>:<actual>" — host key changed (possible MITM)
/// - "NEED_PASSWORD" — password required for this profile
/// - "NEED_PASSPHRASE" — SSH key is passphrase-protected; user will be prompted at runtime
/// - Other strings — connection or auth failure
#[tauri::command]
pub fn connect_sftp(
    profile_id: String,
    password: Option<String>,
    passphrase: Option<String>,
) -> Result<(), String> {
    // Store any supplied runtime credentials (session memory only, never written to disk)
    if password.is_some() || passphrase.is_some() {
        credentials_store::set(
            &profile_id,
            credentials_store::Credentials { password, passphrase },
        );
    }

    let profile = profile_service::get_profile(&profile_id)?;

    let protocol = profile.protocol.clone().unwrap_or_default();

    // FTP: no host-key check; always uses password auth.
    // Auto-load a saved password from persistent storage if available.
    if protocol == Protocol::Ftp {
        let mode = profile
            .credential_storage_mode
            .clone()
            .unwrap_or(CredentialStorageMode::Never);
        let portable = profile.stored_secret_portable.clone();
        let pid_for_loader = profile_id.clone();
        credentials_store::get_or_load(&profile_id, move || match mode {
            CredentialStorageMode::LocalMachine => secrets_service::get(&pid_for_loader),
            CredentialStorageMode::PortableProfile => portable,
            CredentialStorageMode::Never => None,
        });
        return ftp_service::test_connection(&profile);
    }

    // For password auth only: if no runtime credential is in the session store,
    // auto-load a previously saved password from persistent storage.
    //
    // Uses get_or_load to atomically check-and-load under a single lock, avoiding
    // a TOCTOU race between a separate get() and conditional set() pair.
    //
    // Passphrases for SSH key auth are NEVER loaded from storage — they are
    // always prompted at connection time and discarded afterwards.
    if profile.auth_type == AuthType::Password {
        let mode = profile
            .credential_storage_mode
            .clone()
            .unwrap_or(CredentialStorageMode::Never);
        let portable = profile.stored_secret_portable.clone();
        let pid_for_loader = profile_id.clone();
        credentials_store::get_or_load(&profile_id, move || match mode {
            CredentialStorageMode::LocalMachine => secrets_service::get(&pid_for_loader),
            CredentialStorageMode::PortableProfile => portable,
            CredentialStorageMode::Never => None,
        });
    }

    sftp_service::test_connection(&profile)
}

/// Save an accepted host key fingerprint to the local known_hosts file.
#[tauri::command]
pub fn accept_host_key(profile_id: String, fingerprint: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    known_hosts_service::trust(&profile.host, profile.port, &fingerprint)
}

/// Persist a password credential for a profile according to the chosen storage mode.
///
/// This command only applies to **password authentication** profiles. Calling it for
/// an SSH key auth profile is a no-op — passphrases must never be stored.
///
/// Modes:
/// - "local_machine":    plaintext file at ~/.config/murmurssh/secrets/<id>, 0600 perms.
///                       Does not travel with the profile JSON. Machine-local only.
/// - "portable_profile": plaintext inside the profile JSON as `stored_secret_portable`.
///                       SECURITY WARNING: anyone with profile file access can read it.
/// - Any other value:    no-op ("never" — nothing is stored).
#[tauri::command]
pub fn save_credential(profile_id: String, secret: String, mode: String) -> Result<(), String> {
    let mut profile = profile_service::get_profile(&profile_id)?;

    // Passphrases (SSH key auth) must never be persisted. Only password auth and FTP
    // profiles may have stored credentials. Silently no-op for key/agent SSH profiles.
    let protocol = profile.protocol.clone().unwrap_or_default();
    if profile.auth_type != AuthType::Password && protocol != Protocol::Ftp {
        return Ok(());
    }

    match mode.as_str() {
        "local_machine" => {
            secrets_service::set(&profile_id, &secret)?;
            // Remove any portable copy that may have existed previously
            profile.stored_secret_portable = None;
            profile.credential_storage_mode = Some(CredentialStorageMode::LocalMachine);
            profile_service::save_profile(&profile)?;
        }
        "portable_profile" => {
            profile.stored_secret_portable = Some(secret);
            profile.credential_storage_mode = Some(CredentialStorageMode::PortableProfile);
            // Remove the local machine file if it existed
            secrets_service::delete(&profile_id);
            profile_service::save_profile(&profile)?;
        }
        // "never" or anything else — do not store
        _ => {}
    }

    Ok(())
}

/// Remove all stored credentials for a profile:
/// - deletes the local machine secret file (if any)
/// - clears the portable secret field in the profile JSON (if any)
/// - resets credential_storage_mode to None
/// - clears the session store entry so the next connect prompts fresh
#[tauri::command]
pub fn clear_credential(profile_id: String) -> Result<(), String> {
    // Remove local machine secret file (silent if not present)
    secrets_service::delete(&profile_id);

    // Clear portable field and metadata from the profile JSON
    if let Ok(mut profile) = profile_service::get_profile(&profile_id) {
        profile.stored_secret_portable = None;
        profile.credential_storage_mode = None;
        let _ = profile_service::save_profile(&profile);
    }

    // Clear session store so the next connect prompts for credentials
    credentials_store::clear(&profile_id);

    Ok(())
}

/// Trust the host key for this session only — does NOT write to known_hosts.
///
/// The fingerprint is recorded in an in-memory store that lives until the app
/// exits.  On next launch the host will appear unknown again.
///
/// This is the "Accept once" path in the host key dialog.
#[tauri::command]
pub fn accept_host_key_once(profile_id: String, fingerprint: String) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    session_trust_store::trust(&profile.host, profile.port, &fingerprint);
    Ok(())
}

/// Clear only the session-memory credential cache for a profile.
///
/// Unlike `clear_credential`, this does NOT touch any persistent storage
/// (local machine file, portable profile field, or storage mode metadata).
/// It is used on disconnect so that the next connect re-prompts if the user
/// is in "never save" mode, or transparently re-loads if they saved previously.
#[tauri::command]
pub fn clear_session_credentials(profile_id: String) -> Result<(), String> {
    credentials_store::clear(&profile_id);
    Ok(())
}
