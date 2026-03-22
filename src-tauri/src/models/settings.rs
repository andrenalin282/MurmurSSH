use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub last_used_profile_id: Option<String>,
    /// Custom directory to load/save profiles from.
    /// When None, the default ~/.config/murmurssh/profiles/ is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles_path: Option<String>,
    /// UI theme preference: "dark", "light", or "system".
    /// When None, defaults to "system" on the frontend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

