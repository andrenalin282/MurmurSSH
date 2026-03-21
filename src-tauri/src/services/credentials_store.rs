//! In-memory, session-only storage for runtime connection credentials.
//! Nothing here is ever written to disk. Contents are cleared when the app exits.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Runtime credentials for a profile. Never persisted.
#[derive(Clone, Default)]
pub struct Credentials {
    /// Runtime-only password for password authentication.
    pub password: Option<String>,
    /// Runtime-only passphrase for encrypted SSH private keys.
    pub passphrase: Option<String>,
}

fn store() -> &'static Mutex<HashMap<String, Credentials>> {
    static STORE: OnceLock<Mutex<HashMap<String, Credentials>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store runtime credentials for a profile. Overwrites any previous entry.
pub fn set(profile_id: &str, creds: Credentials) {
    if let Ok(mut map) = store().lock() {
        map.insert(profile_id.to_string(), creds);
    }
}

/// Retrieve runtime credentials for a profile, or empty defaults if none stored.
pub fn get(profile_id: &str) -> Credentials {
    store()
        .lock()
        .ok()
        .and_then(|map| map.get(profile_id).cloned())
        .unwrap_or_default()
}

/// Remove credentials for a profile (call after auth failure to clear bad credentials).
pub fn clear(profile_id: &str) {
    if let Ok(mut map) = store().lock() {
        map.remove(profile_id);
    }
}
