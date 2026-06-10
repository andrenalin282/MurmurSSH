use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    /// Unix timestamp in seconds.
    pub modified: Option<u64>,
    /// Unix permission/mode bits (e.g. 0o644). None when unavailable (e.g. FTP).
    pub perm: Option<u32>,
}
