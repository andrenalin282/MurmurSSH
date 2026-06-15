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
    /// Side the local file browser panel is shown on: "left" or "right".
    /// When None, defaults to "left".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_browser_position: Option<String>,
    /// Maximum number of transfers the background queue runs concurrently.
    /// When None, defaults to 2. Clamped to 1..=8 at use sites.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrent_transfers: Option<u32>,
    /// Profile list sort mode: "name" (alphabetical) or "created" (creation date).
    /// When None, defaults to "name" on the frontend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_sort: Option<String>,
    /// Which profile group is currently expanded in the selector accordion.
    /// At most one group is open at a time. `Some("")` is the ungrouped bucket;
    /// `None` means nothing is expanded. Restored on next launch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded_profile_group: Option<String>,
}

