use std::io::Write;
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

    // Use connect_timeout so an unreachable server fails quickly instead of
    // blocking the UI thread for the OS TCP timeout (which can be 2+ minutes).
    let socket_addr = addr.parse::<std::net::SocketAddr>()
        .or_else(|_| {
            // addr contains a hostname — resolve it first
            use std::net::ToSocketAddrs;
            addr.to_socket_addrs()
                .map_err(|e| format!("Server not reachable. Please check the connection details. (resolve: {})", e))?
                .next()
                .ok_or_else(|| format!("Server not reachable. Please check the connection details. (no address for {})", addr))
        })
        .map_err(|e: String| e)?;

    let tcp = TcpStream::connect_timeout(&socket_addr, std::time::Duration::from_secs(15))
        .map_err(|_| "Server not reachable. Please check the connection details.".to_string())?;

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

/// Resolve the server-side effective SFTP start directory using `realpath(".")`.
///
/// Returns the absolute path the SFTP server reports as the initial working
/// directory (typically the user's home directory). Falls back to "/" if the
/// SFTP channel cannot be opened or if realpath is not supported by the server.
///
/// Called during initial connect when the profile has no explicit remote path
/// configured, so the file browser starts at the user's actual home rather
/// than the filesystem root.
pub fn get_sftp_home(profile: &Profile) -> Result<String, String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    let home = sftp
        .realpath(Path::new("."))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/".to_string());

    Ok(home)
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
            path_buf.file_name().map(|name| {
                // readdir returns lstat results: symlinks report S_IFLNK, not S_IFDIR.
                // Follow symlinks via sftp.stat() so that symlinks-to-directories
                // (e.g. public_html → /var/www/html) are shown as navigable folders.
                let is_symlink = stat.perm
                    .map(|p| (p & 0o170000) == 0o120000)
                    .unwrap_or(false);
                let is_dir = if is_symlink {
                    sftp.stat(&path_buf).map(|s| s.is_dir()).unwrap_or(false)
                } else {
                    stat.is_dir()
                };
                FileEntry {
                    name: name.to_string_lossy().to_string(),
                    is_dir,
                    size: stat.size,
                    modified: stat.mtime,
                }
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

/// Recursively download a remote directory to a local destination path.
///
/// Uses a single SFTP session for the entire operation. Creates the local directory
/// structure mirroring the remote tree. Symlinks to directories are followed.
pub fn download_directory(profile: &Profile, remote_path: &str, local_path: &str) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    std::fs::create_dir_all(local_path)
        .map_err(|e| format!("Failed to create local directory '{}': {}", local_path, e))?;

    download_directory_recursive(&sftp, remote_path, local_path)
}

/// Internal recursive helper for directory download — operates on an open SFTP channel.
fn download_directory_recursive(sftp: &ssh2::Sftp, remote_path: &str, local_path: &str) -> Result<(), String> {
    use std::io::Read;

    let entries = sftp
        .readdir(Path::new(remote_path))
        .map_err(|e| format!("Failed to list '{}': {}", remote_path, e))?;

    for (entry_path, stat) in entries {
        let entry_name = match entry_path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };

        let entry_path_str = entry_path.to_string_lossy().to_string();
        let local_entry = format!("{}/{}", local_path.trim_end_matches('/'), entry_name);

        // Determine if the entry is a directory (follow symlinks)
        let is_symlink = stat.perm
            .map(|p| (p & 0o170000) == 0o120000)
            .unwrap_or(false);
        let is_dir = if is_symlink {
            sftp.stat(&entry_path).map(|s| s.is_dir()).unwrap_or(false)
        } else {
            stat.is_dir()
        };

        if is_dir {
            std::fs::create_dir_all(&local_entry)
                .map_err(|e| format!("Failed to create local directory '{}': {}", local_entry, e))?;
            download_directory_recursive(sftp, &entry_path_str, &local_entry)?;
        } else {
            // Download file: symlinks to files are opened and read as regular files
            let mut remote_file = sftp
                .open(&entry_path)
                .map_err(|e| format!("Failed to open remote file '{}': {}", entry_path_str, e))?;

            let mut local_file = std::fs::File::create(&local_entry)
                .map_err(|e| format!("Failed to create local file '{}': {}", local_entry, e))?;

            let mut buf = Vec::new();
            remote_file
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read remote file '{}': {}", entry_path_str, e))?;

            std::io::Write::write_all(&mut local_file, &buf)
                .map_err(|e| format!("Failed to write local file '{}': {}", local_entry, e))?;
        }
    }

    Ok(())
}

/// Recursively delete a remote directory and all of its contents.
///
/// Uses a single SFTP session for the entire operation. Walks the tree depth-first,
/// deleting files and empty subdirectories in order, then removes the root directory.
pub fn delete_directory(profile: &Profile, path: &str) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    delete_directory_recursive(&sftp, path)
}

/// Internal recursive helper — operates on an already-open SFTP channel.
fn delete_directory_recursive(sftp: &ssh2::Sftp, path: &str) -> Result<(), String> {
    let entries = sftp
        .readdir(Path::new(path))
        .map_err(|e| format!("Failed to list '{}': {}", path, e))?;

    for (entry_path, stat) in entries {
        let entry_path_str = entry_path.to_string_lossy().to_string();

        // Determine if this entry is a directory (follow symlinks via stat)
        let is_symlink = stat.perm
            .map(|p| (p & 0o170000) == 0o120000)
            .unwrap_or(false);

        let is_dir = if is_symlink {
            // For symlinks, stat() follows the link. If the target is a directory
            // we treat it as a directory for deletion purposes.
            sftp.stat(&entry_path).map(|s| s.is_dir()).unwrap_or(false)
        } else {
            stat.is_dir()
        };

        if is_dir && !is_symlink {
            // Recurse into real directories
            delete_directory_recursive(sftp, &entry_path_str)?;
        } else {
            // Delete files and symlinks (including symlinks to directories)
            sftp.unlink(&entry_path)
                .map_err(|e| format!("Failed to delete '{}': {}", entry_path_str, e))?;
        }
    }

    // Remove the now-empty directory itself
    sftp.rmdir(Path::new(path))
        .map_err(|e| format!("Failed to remove directory '{}': {}", path, e))
}
