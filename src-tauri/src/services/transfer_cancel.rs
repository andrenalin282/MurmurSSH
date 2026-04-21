//! Cooperative cancellation for in-flight SFTP/FTP transfers.
//!
//! A single profile_id can only have one transfer in flight at a time (the
//! frontend's `busy` guard enforces this), so a per-profile boolean flag is
//! sufficient. Transfer functions poll `is_cancelled(profile_id)` between
//! chunks and unwind with `CANCELLED_ERROR` when the flag is set.
//!
//! The flag is best-effort: it is cleared at the end of each transfer so a
//! stale cancel request cannot bleed into the next operation.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

fn store() -> &'static Mutex<HashSet<String>> {
    static S: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Sentinel returned from transfer functions when a cancel was requested.
/// Frontend recognises this prefix and renders a cancel-friendly status.
pub const CANCELLED_ERROR: &str = "TRANSFER_CANCELLED";

/// Mark a profile's in-flight transfer for cancellation.
pub fn request_cancel(profile_id: &str) {
    if let Ok(mut s) = store().lock() {
        s.insert(profile_id.to_string());
    }
}

/// Drop any pending cancel request for a profile. Call at the start and end of
/// a transfer so stale requests do not affect the next operation.
pub fn clear(profile_id: &str) {
    if let Ok(mut s) = store().lock() {
        s.remove(profile_id);
    }
}

/// Check whether a cancel was requested for the given profile.
pub fn is_cancelled(profile_id: &str) -> bool {
    store()
        .lock()
        .map(|s| s.contains(profile_id))
        .unwrap_or(false)
}
