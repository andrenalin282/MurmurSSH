//! Session-only trusted host key fingerprints.
//!
//! When the user clicks "Accept once" in the host key dialog, the fingerprint is
//! recorded here for the lifetime of the app session — it is NEVER written to the
//! known_hosts file on disk.  The next time the app starts, the host will appear
//! unknown again and the user will be prompted once more.
//!
//! This is distinct from `known_hosts_service` which provides durable trust.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

fn store() -> &'static Mutex<HashSet<String>> {
    static STORE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashSet::new()))
}

fn key(host: &str, port: u16, fingerprint: &str) -> String {
    format!("{}:{}:{}", host, port, fingerprint)
}

/// Record a fingerprint as session-trusted (not written to disk).
pub fn trust(host: &str, port: u16, fingerprint: &str) {
    if let Ok(mut set) = store().lock() {
        set.insert(key(host, port, fingerprint));
    }
}

/// Returns true if this fingerprint was accepted via "Accept once" this session.
pub fn is_trusted(host: &str, port: u16, fingerprint: &str) -> bool {
    store()
        .lock()
        .ok()
        .map(|set| set.contains(&key(host, port, fingerprint)))
        .unwrap_or(false)
}
