//! Minimal parser for OpenSSH client configuration files (~/.ssh/config).
//!
//! Parses `Host`, `HostName`, `User`, `Port`, and `IdentityFile` stanzas.
//! All other keywords (Match, Include, ProxyJump, ProxyCommand, etc.) are silently ignored.

use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct SshConfigEntry {
    /// The SSH alias (Host stanza value). Never a wildcard in returned results.
    pub host: String,
    /// The actual hostname or IP to connect to. When absent, `host` is used.
    pub hostname: Option<String>,
    /// Remote username.
    pub user: Option<String>,
    /// Remote port. When absent, 22 is the default.
    pub port: Option<u16>,
    /// Path to the private key file, tilde expanded.
    pub identity_file: Option<String>,
}

fn ssh_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/root"));
    PathBuf::from(home).join(".ssh").join("config")
}

/// Expand a leading `~/` in a path to the user's home directory.
fn resolve_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/root"));
        format!("{}/{}", home, rest)
    } else {
        path.to_string()
    }
}

/// Parse `~/.ssh/config` and return a list of importable host entries.
///
/// Only non-wildcard `Host` stanzas are returned. Entries with `*` or `?`
/// in the host pattern, or with multiple space-separated names, are skipped.
/// Parsing errors on individual lines are silently ignored — the rest of the
/// file is still processed.
pub fn parse_ssh_config() -> Result<Vec<SshConfigEntry>, String> {
    let path = ssh_config_path();
    if !path.exists() {
        return Err(format!(
            "SSH config file not found at {}",
            path.display()
        ));
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read SSH config: {}", e))?;

    let mut entries: Vec<SshConfigEntry> = Vec::new();
    let mut current: Option<SshConfigEntry> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip blank lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Split on first whitespace or '=' to get keyword + value
        let split_pos = trimmed.find(|c: char| c.is_whitespace() || c == '=');
        let (keyword, raw_rest) = match split_pos {
            Some(pos) => (&trimmed[..pos], &trimmed[pos..]),
            None => continue, // bare keyword with no value — skip
        };
        // Strip leading '=', spaces from value
        let value = raw_rest.trim_matches(|c: char| c.is_whitespace() || c == '=').trim();

        if value.is_empty() {
            continue;
        }

        match keyword.to_lowercase().as_str() {
            "host" => {
                // Finalize and push the previous completed block
                if let Some(entry) = current.take() {
                    entries.push(entry);
                }
                // Skip wildcard patterns and multi-name stanzas
                if value.contains('*') || value.contains('?') || value.split_whitespace().count() > 1 {
                    // current stays None — subsequent lines belong to this skipped block
                    continue;
                }
                current = Some(SshConfigEntry {
                    host: value.to_string(),
                    hostname: None,
                    user: None,
                    port: None,
                    identity_file: None,
                });
            }
            "hostname" => {
                if let Some(ref mut e) = current {
                    e.hostname = Some(value.to_string());
                }
            }
            "user" => {
                if let Some(ref mut e) = current {
                    e.user = Some(value.to_string());
                }
            }
            "port" => {
                if let Some(ref mut e) = current {
                    e.port = value.parse::<u16>().ok();
                }
            }
            "identityfile" => {
                if let Some(ref mut e) = current {
                    // Only record the first IdentityFile per block
                    if e.identity_file.is_none() {
                        e.identity_file = Some(resolve_tilde(value));
                    }
                }
            }
            _ => {} // Silently ignore unrecognised keywords
        }
    }

    // Push the last block
    if let Some(entry) = current.take() {
        entries.push(entry);
    }

    Ok(entries)
}
