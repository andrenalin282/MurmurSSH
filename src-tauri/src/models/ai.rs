//! Data types for the token-gated AI access (MCP bridge → local socket → app).

use serde::{Deserialize, Serialize};

/// What a token may do. Everything is off unless switched on in the GUI.
/// `delete` and `exec` always require a click in the app; `confirm_writes`
/// extends that to upload / mkdir / rename / overwriting downloads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiCaps {
    pub read: bool,
    pub download: bool,
    pub upload: bool,
    pub write: bool,
    pub delete: bool,
    pub exec: bool,
    pub confirm_writes: bool,
}

impl Default for AiCaps {
    fn default() -> Self {
        Self {
            read: false,
            download: false,
            upload: false,
            write: false,
            delete: false,
            exec: false,
            confirm_writes: true,
        }
    }
}

/// Stored token. Only the SHA-256 of the secret is kept; the secret is shown once.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToken {
    pub id: String,
    pub name: String,
    pub hash: String,
    pub prefix: String,
    pub profiles: Vec<String>,
    pub caps: AiCaps,
    pub remote_root: Option<String>,
    pub local_root: Option<String>,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub last_used_at: Option<u64>,
}

/// Token as shown in the GUI (never contains the hash or the secret).
#[derive(Debug, Clone, Serialize)]
pub struct AiTokenView {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub profiles: Vec<String>,
    pub caps: AiCaps,
    pub remote_root: Option<String>,
    pub local_root: Option<String>,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub last_used_at: Option<u64>,
}

impl From<&AiToken> for AiTokenView {
    fn from(t: &AiToken) -> Self {
        Self {
            id: t.id.clone(),
            name: t.name.clone(),
            prefix: t.prefix.clone(),
            profiles: t.profiles.clone(),
            caps: t.caps.clone(),
            remote_root: t.remote_root.clone(),
            local_root: t.local_root.clone(),
            created_at: t.created_at,
            expires_at: t.expires_at,
            last_used_at: t.last_used_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiTokenInput {
    pub name: String,
    pub profiles: Vec<String>,
    pub caps: AiCaps,
    pub remote_root: Option<String>,
    pub local_root: Option<String>,
    pub expires_in_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiCreatedToken {
    pub token: AiTokenView,
    pub secret: String,
}

/// Persisted in `~/.config/murmurssh/ai_access.json` (0600). Kept apart from
/// settings.json so the settings dialog's save can never flip the master switch.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AiStore {
    pub enabled: bool,
    pub tokens: Vec<AiToken>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiStatus {
    pub enabled: bool,
    pub listening: bool,
    pub socket_path: String,
    pub audit_path: String,
}

/// Payload of the `ai-confirm-request` event.
#[derive(Debug, Clone, Serialize)]
pub struct AiConfirmRequest {
    pub request_id: String,
    pub token_name: String,
    pub profile_name: String,
    pub action: String,
    pub summary: String,
    pub detail: String,
}
