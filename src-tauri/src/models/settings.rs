use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub last_used_profile_id: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            last_used_profile_id: None,
        }
    }
}
