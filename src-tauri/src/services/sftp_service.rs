use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

use ssh2::Session;

use crate::models::{AuthType, FileEntry, Profile};
use crate::services::{credentials_store, known_hosts_service};

/// Compute a SHA-256 fingerprint of the server host key as colon-separated hex.
fn host_fingerprint(session: &Session) -> String {
    match session.host_key_hash(ssh2::HashType::Sha256) {
        Some(bytes) => bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(":"),
        None => "unknown".to_string(),
    }
}

/// Opens an authenticated SSH session to the remote host described by `profile`.
///
/// Error strings prefixed with known tokens are handled by the frontend:
/// - "UNKNOWN_HOST:<fp>"   — host not in known_hosts, user must accept/reject
/// - "HOST_MISMATCH:…"    — stored fingerprint differs (possible MITM)
/// - "NEED_PASSWORD"      — password auth selected but no password in session store
/// - "NEED_PASSPHRASE"    — encrypted key, no passphrase in session store
fn connect(profile: &Profile) -> Result<Session, String> {
    let addr = format!("{}:{}", profile.host, profile.port);

    let tcp = TcpStream::connect(&addr)
        .map_err(|e| format!("Connection to {} failed: {}", addr, e))?;

    let mut session =
        Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    // Timeout covers handshake and all subsequent channel operations
    session.set_timeout(15_000);
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    // ── Host key verification ──────────────────────────────────────────────
    let fingerprint = host_fingerprint(&session);
    match known_hosts_service::check(&profile.host, profile.port, &fingerprint) {
        known_hosts_service::HostStatus::Trusted => {}
        known_hosts_service::HostStatus::Unknown => {
            return Err(format!("UNKNOWN_HOST:{}", fingerprint));
        }
        known_hosts_service::HostStatus::Mismatch { stored } => {
            return Err(format!(
                "HOST_MISMATCH: stored key {} does not match server key {}. \
                 Connection aborted to protect against possible man-in-the-middle attack.",
                stored, fingerprint
            ));
        }
    }

    // ── Authentication ─────────────────────────────────────────────────────
    let creds = credentials_store::get(&profile.id);

    match &profile.auth_type {
        AuthType::Key => {
            let key_path = profile
                .key_path
                .as_deref()
                .ok_or("Key path is required for key authentication")?;

            let passphrase = creds.passphrase.as_deref();

            session
                .userauth_pubkey_file(
                    &profile.username,
                    None,
                    Path::new(key_path),
                    passphrase,
                )
                .map_err(|e| {
                    // libssh2 returns error code -16 (LIBSSH2_ERROR_FILE) when the key
                    // is passphrase-protected and decryption fails.
                    if let ssh2::ErrorCode::Session(code) = e.code() {
                        if code == -16 {
                            if passphrase.is_none() {
                                return "NEED_PASSPHRASE".to_string();
                            } else {
                                // Wrong passphrase — clear it so the user can retry
                                credentials_store::clear(&profile.id);
                                return "Incorrect passphrase for SSH key.".to_string();
                            }
                        }
                    }
                    format!("Key authentication failed ({}): {}", key_path, e)
                })?;
        }

        AuthType::Password => {
            let password = creds
                .password
                .as_deref()
                .ok_or_else(|| "NEED_PASSWORD".to_string())?;

            session
                .userauth_password(&profile.username, password)
                .map_err(|e| {
                    // Clear bad password so the user must re-enter it
                    credentials_store::clear(&profile.id);
                    format!("Password authentication failed: {}", e)
                })?;
        }

        AuthType::Agent => {
            let mut agent = session
                .agent()
                .map_err(|e| format!("Failed to open SSH agent connection: {}", e))?;

            agent
                .connect()
                .map_err(|e| format!("SSH agent connection failed: {}", e))?;

            agent
                .list_identities()
                .map_err(|e| format!("Failed to list SSH agent identities: {}", e))?;

            let mut authenticated = false;
            for identity in agent
                .identities()
                .map_err(|e| format!("Failed to read SSH agent identities: {}", e))?
            {
                if agent.userauth(&profile.username, &identity).is_ok() {
                    authenticated = true;
                    break;
                }
            }

            if !authenticated {
                return Err(
                    "SSH agent authentication failed: no matching identity found".to_string(),
                );
            }
        }
    }

    if !session.authenticated() {
        return Err("Authentication failed".to_string());
    }

    Ok(session)
}

/// Test the connection without performing any file operations.
/// Called by the `connect_sftp` IPC command before the file browser is shown.
pub fn test_connection(profile: &Profile) -> Result<(), String> {
    connect(profile).map(|_| ())
}

pub fn list_directory(profile: &Profile, path: &str) -> Result<Vec<FileEntry>, String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    let raw = sftp
        .readdir(Path::new(path))
        .map_err(|e| format!("Failed to list '{}': {}", path, e))?;

    let mut entries: Vec<FileEntry> = raw
        .into_iter()
        .filter_map(|(path_buf, stat)| {
            path_buf.file_name().map(|name| FileEntry {
                name: name.to_string_lossy().to_string(),
                is_dir: stat.is_dir(),
                size: stat.size,
                modified: stat.mtime,
            })
        })
        .collect();

    // Directories first, then alphabetical
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Ok(entries)
}

/// Upload a local file to a remote path.
pub fn upload_file(profile: &Profile, local_path: &str, remote_path: &str) -> Result<(), String> {
    let mut local = std::fs::File::open(local_path)
        .map_err(|e| format!("Failed to open local file '{}': {}", local_path, e))?;

    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    let mut remote = sftp
        .create(Path::new(remote_path))
        .map_err(|e| format!("Failed to create remote file '{}': {}", remote_path, e))?;

    std::io::copy(&mut local, &mut remote)
        .map(|_| ())
        .map_err(|e| format!("Upload to '{}' failed: {}", remote_path, e))
}

/// Upload raw bytes to a remote path. Used by the file browser upload button.
pub fn upload_bytes(profile: &Profile, remote_path: &str, content: &[u8]) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    let mut remote = sftp
        .create(Path::new(remote_path))
        .map_err(|e| format!("Failed to create remote file '{}': {}", remote_path, e))?;

    remote
        .write_all(content)
        .map_err(|e| format!("Upload to '{}' failed: {}", remote_path, e))
}

/// Download a remote file to a local path.
pub fn download_file(profile: &Profile, remote_path: &str, local_path: &str) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    let mut remote = sftp
        .open(Path::new(remote_path))
        .map_err(|e| format!("Failed to open remote file '{}': {}", remote_path, e))?;

    let mut local = std::fs::File::create(local_path)
        .map_err(|e| format!("Failed to create local file '{}': {}", local_path, e))?;

    std::io::copy(&mut remote, &mut local)
        .map(|_| ())
        .map_err(|e| format!("Download of '{}' failed: {}", remote_path, e))
}

pub fn delete_file(profile: &Profile, remote_path: &str) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    sftp.unlink(Path::new(remote_path))
        .map_err(|e| format!("Failed to delete '{}': {}", remote_path, e))
}

pub fn rename_file(profile: &Profile, from: &str, to: &str) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    sftp.rename(Path::new(from), Path::new(to), None)
        .map_err(|e| format!("Failed to rename '{}' to '{}': {}", from, to, e))
}

pub fn create_directory(profile: &Profile, path: &str) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    sftp.mkdir(Path::new(path), 0o755)
        .map_err(|e| format!("Failed to create directory '{}': {}", path, e))
}
