use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// SSH terminal + SFTP file browser (default for existing profiles).
    #[default]
    Ssh,
    /// SFTP file browser only — no terminal.
    Sftp,
    /// FTP file browser only — no terminal, plain FTP protocol.
    Ftp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: AuthType,
    /// Path to SSH private key. Required when auth_type is Key, otherwise None.
    pub key_path: Option<String>,
    pub default_remote_path: Option<String>,
    pub editor_command: Option<String>,
    pub upload_mode: UploadMode,

    /// Connection protocol. Defaults to Ssh when absent (backward compatible).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Protocol>,

    /// Optional local working directory for uploads and downloads.
    /// When set: downloads save here directly; upload picker starts here.
    /// When absent/None: download shows a save dialog; upload uses system default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,

    /// How the credential (password or key passphrase) is persisted between sessions.
    /// Defaults to Never if absent — old profiles without this field always prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_storage_mode: Option<CredentialStorageMode>,

    /// Credential stored in the profile file itself (portable_profile mode).
    ///
    /// SECURITY WARNING: This is plaintext. Anyone with access to the profile file
    /// can read it. It is stored here intentionally only when the user explicitly
    /// chooses the portable storage option and accepts the security trade-off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_secret_portable: Option<String>,

    /// Per-OS-user local browser paths, used only for portable (shared) profiles.
    ///
    /// Maps OS username (from $USER) → absolute local directory path.
    /// Each user who opens a shared profile gets their own local browser start path.
    /// For local-machine profiles `local_path` is used instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_paths_by_user: Option<HashMap<String, String>>,

    /// Optional group/folder this profile belongs to (free-text). Empty/None = ungrouped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    /// Profile creation time, epoch seconds. Set once at first save; preserved on edit.
    /// Legacy profiles without it are backfilled from the JSON file mtime at list time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    Key,
    Agent,
    /// Password authentication. Password is never stored — prompted at runtime.
    Password,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadMode {
    Auto,
    Confirm,
}

/// Determines how a credential (password or key passphrase) is retained between app sessions.
///
/// Security tiers from highest to lowest: Never > LocalMachine > PortableProfile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStorageMode {
    /// The user is prompted at every connection. Nothing is written to disk. (Safest.)
    #[default]
    Never,
    /// The credential is stored in a plaintext file at ~/.config/murmurssh/secrets/<id>
    /// with 0600 permissions. It does not travel with the profile, making it
    /// machine-local. It is NOT encrypted — anyone with filesystem access can read it.
    LocalMachine,
    /// The credential is stored as plaintext inside the profile JSON file.
    /// This makes the profile portable to another PC, but the credential is
    /// completely unprotected against anyone with access to the file.
    /// This is intentionally the weakest option and must be clearly labeled as such.
    PortableProfile,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_legacy_profile_without_group_or_created_at() {
        let json = r#"{
            "id": "old",
            "name": "Old",
            "host": "h",
            "port": 22,
            "username": "u",
            "auth_type": "agent",
            "key_path": null,
            "default_remote_path": null,
            "editor_command": null,
            "upload_mode": "confirm"
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.group, None);
        assert_eq!(p.created_at, None);
    }

    #[test]
    fn group_and_created_at_round_trip() {
        let json = r#"{
            "id": "x","name":"X","host":"h","port":22,"username":"u",
            "auth_type":"agent","key_path":null,"default_remote_path":null,
            "editor_command":null,"upload_mode":"confirm",
            "group":"Work","created_at":1700000000
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.group.as_deref(), Some("Work"));
        assert_eq!(p.created_at, Some(1_700_000_000));
    }
}
