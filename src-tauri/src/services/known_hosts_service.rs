//! Minimal host key verification — similar to SSH known_hosts but simplified.
//!
//! Storage: ~/.config/murmurssh/known_hosts
//! Format (one line per host):
//!   hostname:port SHA256:colon_separated_hex

use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;

fn known_hosts_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("murmurssh")
        .join("known_hosts")
}

fn host_key(host: &str, port: u16) -> String {
    format!("{}:{}", host, port)
}

fn load() -> Vec<(String, String)> {
    let path = known_hosts_path();
    if !path.exists() {
        return vec![];
    }
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|line| {
            let mut parts = line.splitn(2, ' ');
            let key = parts.next()?.to_string();
            let fp = parts.next()?.to_string();
            Some((key, fp))
        })
        .collect()
}

pub enum HostStatus {
    /// Host key matches the stored entry.
    Trusted,
    /// No entry exists for this host yet.
    Unknown,
    /// A different key is stored for this host — possible MITM.
    Mismatch { stored: String },
}

/// Check whether the given host+port fingerprint is trusted.
pub fn check(host: &str, port: u16, fingerprint: &str) -> HostStatus {
    let key = host_key(host, port);
    let entries = load();
    match entries.into_iter().find(|(k, _)| k == &key) {
        None => HostStatus::Unknown,
        Some((_, stored)) if stored == fingerprint => HostStatus::Trusted,
        Some((_, stored)) => HostStatus::Mismatch { stored },
    }
}

/// Append a new trusted host entry to the known_hosts file.
pub fn trust(host: &str, port: u16, fingerprint: &str) -> Result<(), String> {
    let path = known_hosts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed to open known_hosts: {}", e))?;
    writeln!(file, "{} {}", host_key(host, port), fingerprint)
        .map_err(|e| format!("Failed to write known_hosts: {}", e))
}
