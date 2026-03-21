use crate::models::{AuthType, CredentialStorageMode};
use crate::services::{
    credentials_store, known_hosts_service, profile_service, secrets_service, sftp_service,
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

    // For password auth only: if no runtime credential is in the session store,
    // try to auto-load a previously saved password from persistent storage.
    //
    // Passphrases for SSH key auth are NEVER loaded from storage — they are
    // always prompted at connection time and discarded afterwards.
    if profile.auth_type == AuthType::Password {
        let session_creds = credentials_store::get(&profile_id);
        if session_creds.password.is_none() {
            let mode = profile
                .credential_storage_mode
                .as_ref()
                .unwrap_or(&CredentialStorageMode::Never);
            let stored_password = match mode {
                CredentialStorageMode::LocalMachine => secrets_service::get(&profile_id),
                CredentialStorageMode::PortableProfile => profile.stored_secret_portable.clone(),
                CredentialStorageMode::Never => None,
            };
            if let Some(pwd) = stored_password {
                credentials_store::set(
                    &profile_id,
                    credentials_store::Credentials {
                        password: Some(pwd),
                        passphrase: None,
                    },
                );
            }
        }
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

    // Passphrases (SSH key auth) must never be persisted. Only password auth credentials
    // may be stored. Silently no-op if the profile uses key or agent auth.
    if profile.auth_type != AuthType::Password {
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
