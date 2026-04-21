use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

const TRANSFER_CHUNK: usize = 256 * 1024; // 256 KB chunks for progress granularity

/// Timeout applied during handshake + authentication. Kept short so a broken
/// host fails quickly instead of hanging the UI.
const HANDSHAKE_TIMEOUT_MS: u32 = 15_000;

/// Timeout applied after authentication to every subsequent blocking libssh2
/// operation (SFTP open/read/write). Large on purpose: a single 256 KB chunk
/// stalling beyond this means the link is effectively dead, not slow. Covers
/// legitimate slow-link transfers without silently dropping them (audit F6).
const TRANSFER_TIMEOUT_MS: u32 = 120_000;

use ssh2::Session;

use crate::models::{AuthType, FileEntry, Profile};
use crate::services::{credentials_store, known_hosts_service, transfer_cancel};

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

    // Short timeout for handshake + authentication so an unresponsive or
    // misconfigured server fails fast. The per-SFTP-op timeout is relaxed
    // below after authentication so large transfers on slow links do not
    // abort mid-chunk (audit F6).
    session.set_timeout(HANDSHAKE_TIMEOUT_MS);
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

    // Relax the per-op timeout now that auth is done so multi-minute transfers
    // over slow links are not aborted mid-chunk (audit F6).
    session.set_timeout(TRANSFER_TIMEOUT_MS);

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
/// `on_progress(bytes_done, bytes_total)` is called after each chunk.
///
/// On any read/write failure the partially written remote file is removed on a
/// best-effort basis (F5) so a retry starts from a clean state.
pub fn upload_file(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    transfer_cancel::clear(&profile.id);
    let result = upload_file_inner(profile, local_path, remote_path, on_progress);
    transfer_cancel::clear(&profile.id);
    result
}

fn upload_file_inner(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    let mut local = std::fs::File::open(local_path)
        .map_err(|e| format!("Failed to open local file '{}': {}", local_path, e))?;
    let total = local.metadata().map(|m| m.len()).unwrap_or(0);

    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    let mut remote = sftp
        .create(Path::new(remote_path))
        .map_err(|e| format!("Failed to create remote file '{}': {}", remote_path, e))?;

    let mut buf = vec![0u8; TRANSFER_CHUNK];
    let mut done = 0u64;
    let write_result: Result<(), String> = loop {
        if transfer_cancel::is_cancelled(&profile.id) {
            break Err(transfer_cancel::CANCELLED_ERROR.to_string());
        }
        let n = match local.read(&mut buf) {
            Ok(v) => v,
            Err(e) => break Err(format!("Read '{}' failed: {}", local_path, e)),
        };
        if n == 0 { break Ok(()); }
        if let Err(e) = remote.write_all(&buf[..n]) {
            break Err(format!("Upload to '{}' failed: {}", remote_path, e));
        }
        done += n as u64;
        on_progress(done, total);
    };
    if let Err(e) = write_result {
        drop(remote);
        let _ = sftp.unlink(Path::new(remote_path));
        return Err(e);
    }
    Ok(())
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
/// `on_progress(bytes_done, bytes_total)` is called after each chunk.
///
/// On any read/write failure the partially written local file is removed on a
/// best-effort basis (F5) so the user does not end up with a truncated file.
pub fn download_file(
    profile: &Profile,
    remote_path: &str,
    local_path: &str,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    transfer_cancel::clear(&profile.id);
    let result = download_file_inner(profile, remote_path, local_path, on_progress);
    transfer_cancel::clear(&profile.id);
    result
}

fn download_file_inner(
    profile: &Profile,
    remote_path: &str,
    local_path: &str,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    let total = sftp.stat(Path::new(remote_path))
        .map(|s| s.size.unwrap_or(0))
        .unwrap_or(0);

    let mut remote = sftp
        .open(Path::new(remote_path))
        .map_err(|e| format!("Failed to open remote file '{}': {}", remote_path, e))?;
    let mut local = std::fs::File::create(local_path)
        .map_err(|e| format!("Failed to create local file '{}': {}", local_path, e))?;

    let mut buf = vec![0u8; TRANSFER_CHUNK];
    let mut done = 0u64;
    let write_result: Result<(), String> = loop {
        if transfer_cancel::is_cancelled(&profile.id) {
            break Err(transfer_cancel::CANCELLED_ERROR.to_string());
        }
        let n = match remote.read(&mut buf) {
            Ok(v) => v,
            Err(e) => break Err(format!("Download of '{}' failed: {}", remote_path, e)),
        };
        if n == 0 { break Ok(()); }
        if let Err(e) = local.write_all(&buf[..n]) {
            break Err(format!("Write '{}' failed: {}", local_path, e));
        }
        done += n as u64;
        on_progress(done, total);
    };
    if let Err(e) = write_result {
        drop(local);
        let _ = std::fs::remove_file(local_path);
        return Err(e);
    }
    Ok(())
}

/// Check whether a path exists on the remote server via SFTP stat().
/// Returns Ok(true) if stat succeeds (file or directory present),
/// Ok(false) if the path does not exist or is otherwise inaccessible.
pub fn remote_file_exists(profile: &Profile, remote_path: &str) -> Result<bool, String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    Ok(sftp.stat(Path::new(remote_path)).is_ok())
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
///
/// Progress callback semantics: `bytes_done` is **per current file** (resets to 0
/// at the start of each file), `bytes_total` is the size of that file, `filename`
/// is the current entry name. This matches the single-file progress shape.
pub fn download_directory(
    profile: &Profile,
    remote_path: &str,
    local_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    transfer_cancel::clear(&profile.id);
    let result = download_directory_inner(profile, remote_path, local_path, on_progress);
    transfer_cancel::clear(&profile.id);
    result
}

fn download_directory_inner(
    profile: &Profile,
    remote_path: &str,
    local_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    std::fs::create_dir_all(local_path)
        .map_err(|e| format!("Failed to create local directory '{}': {}", local_path, e))?;

    download_directory_recursive(&profile.id, &sftp, remote_path, local_path, on_progress)
}

/// Internal recursive helper for directory download — operates on an open SFTP channel.
fn download_directory_recursive(
    profile_id: &str,
    sftp: &ssh2::Sftp,
    remote_path: &str,
    local_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    if transfer_cancel::is_cancelled(profile_id) {
        return Err(transfer_cancel::CANCELLED_ERROR.to_string());
    }
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
            download_directory_recursive(profile_id, sftp, &entry_path_str, &local_entry, on_progress)?;
        } else {
            let file_total = sftp.stat(&entry_path).map(|s| s.size.unwrap_or(0)).unwrap_or(0);
            on_progress(0, file_total, &entry_name);

            let mut remote_file = sftp
                .open(&entry_path)
                .map_err(|e| format!("Failed to open remote file '{}': {}", entry_path_str, e))?;
            let mut local_file = std::fs::File::create(&local_entry)
                .map_err(|e| format!("Failed to create local file '{}': {}", local_entry, e))?;

            let mut buf = vec![0u8; TRANSFER_CHUNK];
            let mut file_done = 0u64;
            let write_result: Result<(), String> = loop {
                if transfer_cancel::is_cancelled(profile_id) {
                    break Err(transfer_cancel::CANCELLED_ERROR.to_string());
                }
                let n = match remote_file.read(&mut buf) {
                    Ok(v) => v,
                    Err(e) => break Err(format!("Failed to read '{}': {}", entry_path_str, e)),
                };
                if n == 0 { break Ok(()); }
                if let Err(e) = local_file.write_all(&buf[..n]) {
                    break Err(format!("Failed to write '{}': {}", local_entry, e));
                }
                file_done += n as u64;
                on_progress(file_done, file_total, &entry_name);
            };
            if let Err(e) = write_result {
                // Best-effort cleanup of the partially downloaded file (F5).
                drop(local_file);
                let _ = std::fs::remove_file(&local_entry);
                return Err(e);
            }
        }
    }

    Ok(())
}

/// Recursively upload a local directory to a remote destination path.
///
/// Uses a single SFTP session for the entire operation. Creates the remote directory
/// structure mirroring the local tree. Existing remote directories are tolerated
/// so a re-upload of the same folder does not fail on the root mkdir.
/// Symlinks are followed; broken symlinks and non-regular files are skipped.
pub fn upload_directory(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    transfer_cancel::clear(&profile.id);
    let result = upload_directory_inner(profile, local_path, remote_path, on_progress);
    transfer_cancel::clear(&profile.id);
    result
}

fn upload_directory_inner(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    mkdir_ok_if_exists(&sftp, Path::new(remote_path))?;
    upload_directory_recursive(&profile.id, &sftp, Path::new(local_path), remote_path, on_progress)
}

/// Try to create a remote directory; silently succeed if it already exists.
fn mkdir_ok_if_exists(sftp: &ssh2::Sftp, path: &Path) -> Result<(), String> {
    match sftp.mkdir(path, 0o755) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Directory may already exist — confirm with stat before reporting an error.
            sftp.stat(path)
                .map(|_| ())
                .map_err(|_| format!("Failed to create remote directory '{}'", path.display()))
        }
    }
}

/// Internal recursive helper for directory upload — operates on an open SFTP channel.
///
/// Progress: `bytes_done` is per current file, `bytes_total` is the current
/// file's size, `filename` is the current entry. Matches single-file shape.
fn upload_directory_recursive(
    profile_id: &str,
    sftp: &ssh2::Sftp,
    local_dir: &Path,
    remote_dir: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    if transfer_cancel::is_cancelled(profile_id) {
        return Err(transfer_cancel::CANCELLED_ERROR.to_string());
    }
    let read_dir = std::fs::read_dir(local_dir)
        .map_err(|e| format!("Failed to read local directory '{}': {}", local_dir.display(), e))?;

    for entry in read_dir {
        let entry = entry
            .map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let local_entry = entry.path();

        let entry_name = match local_entry.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        if entry_name.is_empty() {
            continue;
        }

        let remote_entry = format!("{}/{}", remote_dir.trim_end_matches('/'), entry_name);

        // Use is_dir() / is_file() which follow symlinks.
        // Broken symlinks and special files (sockets, devices) are skipped.
        if local_entry.is_dir() {
            mkdir_ok_if_exists(sftp, Path::new(&remote_entry))?;
            upload_directory_recursive(profile_id, sftp, &local_entry, &remote_entry, on_progress)?;
        } else if local_entry.is_file() {
            let file_total = local_entry.metadata().map(|m| m.len()).unwrap_or(0);
            on_progress(0, file_total, &entry_name);

            let mut local_file = std::fs::File::open(&local_entry)
                .map_err(|e| format!("Failed to open '{}': {}", local_entry.display(), e))?;
            let mut remote_file = sftp
                .create(Path::new(&remote_entry))
                .map_err(|e| format!("Failed to create remote file '{}': {}", remote_entry, e))?;

            let mut buf = vec![0u8; TRANSFER_CHUNK];
            let mut file_done = 0u64;
            let write_result: Result<(), String> = loop {
                if transfer_cancel::is_cancelled(profile_id) {
                    break Err(transfer_cancel::CANCELLED_ERROR.to_string());
                }
                let n = match local_file.read(&mut buf) {
                    Ok(v) => v,
                    Err(e) => break Err(format!("Read '{}' failed: {}", local_entry.display(), e)),
                };
                if n == 0 { break Ok(()); }
                if let Err(e) = remote_file.write_all(&buf[..n]) {
                    break Err(format!("Upload '{}' failed: {}", remote_entry, e));
                }
                file_done += n as u64;
                on_progress(file_done, file_total, &entry_name);
            };
            if let Err(e) = write_result {
                // Best-effort cleanup of the partial remote file (F5).
                drop(remote_file);
                let _ = sftp.unlink(Path::new(&remote_entry));
                return Err(e);
            }
        }
        // Broken symlinks and special files are skipped without error.
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
