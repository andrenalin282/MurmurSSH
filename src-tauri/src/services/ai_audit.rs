//! Append-only audit log of every AI request: `~/.config/murmurssh/ai_audit.jsonl` (0600).
//! Retention: rotated to `.1` at 5 MB, the previous `.1` is dropped (max ~10 MB on disk).
//! Never contains secrets or file contents.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{json, Value};

const ROTATE_AT: u64 = 5 * 1024 * 1024;
static LOCK: Mutex<()> = Mutex::new(());

pub fn audit_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config").join("murmurssh").join("ai_audit.jsonl")
}

/// Best-effort: a failing audit write is reported on stderr but never blocks the action.
pub fn record(token: &str, tool: &str, profile: Option<&str>, args: &Value, outcome: &Result<Value, String>, confirmed: Option<bool>) {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let path = audit_path();
    if std::fs::metadata(&path).map(|m| m.len() >= ROTATE_AT).unwrap_or(false) {
        let _ = std::fs::rename(&path, path.with_extension("jsonl.1"));
    }
    let line = json!({
        "ts": crate::services::ai_token_service::now(),
        "token": token,
        "tool": tool,
        "profile": profile,
        "args": args,
        "ok": outcome.is_ok(),
        "error": outcome.as_ref().err(),
        "confirmed": confirmed,
    });
    let res = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&path)
        .and_then(|mut f| writeln!(f, "{}", line));
    if let Err(e) = res {
        eprintln!("[ai_audit] write failed: {}", e);
    }
}
