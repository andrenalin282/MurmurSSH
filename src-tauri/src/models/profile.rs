use serde::{Deserialize, Serialize};

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
