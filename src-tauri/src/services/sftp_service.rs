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

use crate::models::{AuthType, FileEntry, Profile, CANCELLED_ERROR};
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

/// TCP connect + SSH handshake (no authentication, no host key check yet).
fn handshake_session(profile: &Profile) -> Result<Session, String> {
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

    Ok(session)
}

/// Verify the server host key against MurmurSSH's trust store and return the raw key.
///
/// Used to pin the OpenSSH ControlMaster to exactly the key the user has trusted,
/// instead of letting a second, unverified connection decide.
pub fn trusted_host_key(profile: &Profile) -> Result<(Vec<u8>, ssh2::HostKeyType), String> {
    let session = handshake_session(profile)?;
    let fingerprint = host_fingerprint(&session);
    match known_hosts_service::check(&profile.host, profile.port, &fingerprint) {
        known_hosts_service::HostStatus::Trusted => {}
        known_hosts_service::HostStatus::Unknown => {
            return Err(format!("UNKNOWN_HOST:{}", fingerprint));
        }
        known_hosts_service::HostStatus::Mismatch { stored } => {
            return Err(format!(
                "HOST_MISMATCH: stored key {} does not match server key {}.",
                stored, fingerprint
            ));
        }
    }
    let (key, kind) = session.host_key().ok_or("Server sent no host key")?;
    Ok((key.to_vec(), kind))
}

/// Opens an authenticated SSH session to the remote host described by `profile`.
///
/// Error strings prefixed with known tokens are handled by the frontend:
/// - "UNKNOWN_HOST:<fp>"   — host not in known_hosts, user must accept/reject
/// - "HOST_MISMATCH:…"    — stored fingerprint differs (possible MITM)
/// - "NEED_PASSWORD"      — password auth selected but no password in session store
/// - "NEED_PASSPHRASE"    — encrypted key, no passphrase in session store
fn connect(profile: &Profile) -> Result<Session, String> {
    let session = handshake_session(profile)?;

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
                    perm: stat.perm,
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

/// True when `err` is an SFTP permission-denied response (F1). ssh2 0.9 normally surfaces
/// this as `ErrorCode::SFTP(LIBSSH2_FX_PERMISSION_DENIED)` (value 3); some servers/paths
/// instead bubble up a generic error whose message still says so, so that is checked too.
fn is_permission_denied(err: &ssh2::Error) -> bool {
    if let ssh2::ErrorCode::SFTP(3) = err.code() {
        return true;
    }
    err.message().to_ascii_lowercase().contains("permission denied")
}

/// Upload `local` to `<dir>/.<name>.murmur-part`, then rename over the effective target,
/// then finalize onto it. On error/cancel only the part file is removed — an existing
/// target stays intact.
///
/// F1: overwriting an existing file must not silently change what it looks like on disk:
/// - If `remote_path` is a symlink, the part file is written and finalized next to what the
///   symlink resolves to, so the symlink itself is preserved instead of being replaced by a
///   regular file.
/// - If the effective target already exists as a regular file, its permission bits (and
///   owner, best-effort) are re-applied to the part file before the rename, so overwriting
///   a 0600 secret or a +x script does not quietly reset it to the part file's default mode.
/// - If the part file cannot even be created (e.g. permission denied on the containing
///   directory) but the target itself exists, this falls back to the pre-branch in-place
///   write (`sftp.create(target)`, truncating) so a file the user CAN write stays writable
///   even in a directory they cannot otherwise write in — at the cost of atomicity and the
///   on-error cleanup that the part-file path provides.
fn write_via_part(
    sftp: &ssh2::Sftp,
    local: &Path,
    remote_path: &str,
    name: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let mut local_file = std::fs::File::open(local)
        .map_err(|e| format!("Failed to open '{}': {}", local.display(), e))?;
    let total = local_file.metadata().map(|m| m.len()).unwrap_or(0);
    on_progress(0, total, name);

    // F1 step 1: never replace a symlink with a regular file — upload onto what it resolves to.
    let is_symlink = sftp
        .lstat(Path::new(remote_path))
        .map(|s| s.file_type().is_symlink())
        .unwrap_or(false);
    let effective_target = if is_symlink {
        sftp.realpath(Path::new(remote_path))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| remote_path.to_string())
    } else {
        remote_path.to_string()
    };

    // F1 step 2: remember the previous file's perm/owner so it can be re-applied before finalize.
    let existing_stat = sftp
        .stat(Path::new(&effective_target))
        .ok()
        .filter(|s| s.is_file());

    let part = crate::services::transfer_paths::part_path(&effective_target);
    // Hardening: if the target already exists with a known mode, create the part file with
    // that mode from the start instead of the default 0644 — otherwise a 0600 secret's part
    // file is briefly world-readable while the upload is in flight, or indefinitely if the
    // process crashes before the setstat below runs. The setstat before finalize still runs
    // afterward regardless, since a server umask can strip bits from the mode passed here.
    let create_result = match existing_stat.as_ref().and_then(|s| s.perm) {
        Some(perm) => sftp.open_mode(
            Path::new(&part),
            ssh2::OpenFlags::WRITE | ssh2::OpenFlags::TRUNCATE,
            (perm & 0o777) as i32,
            ssh2::OpenType::File,
        ),
        None => sftp.create(Path::new(&part)),
    };
    let mut remote_file = match create_result {
        Ok(f) => f,
        Err(e) => {
            // F1 step 3: containing directory not writable but the target file itself is —
            // fall back to the pre-branch in-place write rather than failing outright.
            if is_permission_denied(&e) && sftp.stat(Path::new(&effective_target)).is_ok() {
                return write_in_place(sftp, &mut local_file, &effective_target, total, name, cancel, on_progress);
            }
            return Err(format!("Failed to create remote file '{}': {}", part, e));
        }
    };

    let mut buf = vec![0u8; TRANSFER_CHUNK];
    let mut done = 0u64;
    let write_result: Result<(), String> = loop {
        if cancel() {
            break Err(CANCELLED_ERROR.to_string());
        }
        let n = match local_file.read(&mut buf) {
            Ok(v) => v,
            Err(e) => break Err(format!("Read '{}' failed: {}", local.display(), e)),
        };
        if n == 0 {
            break Ok(());
        }
        if let Err(e) = remote_file.write_all(&buf[..n]) {
            break Err(format!("Upload '{}' failed: {}", remote_path, e));
        }
        done += n as u64;
        on_progress(done, total, name);
    };
    drop(remote_file);
    if let Err(e) = write_result {
        let _ = sftp.unlink(Path::new(&part));
        return Err(e);
    }

    // F1 step 2 (cont.): re-apply the previous file's perm/owner to the part file before
    // it takes the target's place.
    if let Some(old) = existing_stat.and_then(|s| s.perm.map(|perm| (perm, s.uid, s.gid))) {
        let (perm, uid, gid) = old;
        let full = ssh2::FileStat {
            size: None,
            uid,
            gid,
            perm: Some(perm & 0o7777),
            atime: None,
            mtime: None,
        };
        if sftp.setstat(Path::new(&part), full).is_err() {
            // Setting uid/gid usually requires privileges the connecting user does not have
            // — retry permission bits only. Best-effort: if even that fails, proceed anyway,
            // the completed upload must not be lost over a metadata detail.
            let perm_only = ssh2::FileStat {
                size: None,
                uid: None,
                gid: None,
                perm: Some(perm & 0o7777),
                atime: None,
                mtime: None,
            };
            let _ = sftp.setstat(Path::new(&part), perm_only);
        }
    }

    finalize_part(sftp, &part, &effective_target)
}

/// F1 step 3 fallback: write directly onto `target` (truncating), matching pre-branch
/// behaviour exactly — no part file, no atomicity, and any partial content written before
/// an error/cancel is left in place rather than cleaned up. Used only when a part file
/// could not be created next to the target for permission reasons.
fn write_in_place(
    sftp: &ssh2::Sftp,
    local_file: &mut std::fs::File,
    target: &str,
    total: u64,
    name: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let mut remote_file = sftp
        .create(Path::new(target))
        .map_err(|e| format!("Failed to create remote file '{}': {}", target, e))?;

    let mut buf = vec![0u8; TRANSFER_CHUNK];
    let mut done = 0u64;
    loop {
        if cancel() {
            return Err(CANCELLED_ERROR.to_string());
        }
        let n = local_file
            .read(&mut buf)
            .map_err(|e| format!("Read failed: {}", e))?;
        if n == 0 {
            return Ok(());
        }
        remote_file
            .write_all(&buf[..n])
            .map_err(|e| format!("Upload '{}' failed: {}", target, e))?;
        done += n as u64;
        on_progress(done, total, name);
    }
}

/// Rename the finished part file over the target. SFTP v3 servers (OpenSSH) refuse to
/// rename onto an existing file, so fall back to unlink + rename; the old content is only
/// removed after the new content is completely on the server.
fn finalize_part(sftp: &ssh2::Sftp, part: &str, target: &str) -> Result<(), String> {
    use ssh2::RenameFlags;
    let flags = RenameFlags::OVERWRITE | RenameFlags::ATOMIC | RenameFlags::NATIVE;
    if sftp.rename(Path::new(part), Path::new(target), Some(flags)).is_ok() {
        return Ok(());
    }
    // F2: only unlink the target once our own part file is confirmed still there and ready
    // to take its place — otherwise a concurrent job to the same target could delete it
    // without anything of ours left to rename over it.
    if sftp.stat(Path::new(part)).is_err() {
        return Err(format!(
            "Failed to finalize '{}': upload part '{}' is missing",
            target, part
        ));
    }
    let mut removed_target = false;
    if sftp.stat(Path::new(target)).is_ok() {
        if let Err(e) = sftp.unlink(Path::new(target)) {
            // The target was not removed here, so the part file still holds the only copy
            // of the upload — but finalize has failed outright, so clean it up rather than
            // leaving it behind forever (M-ruling: nothing is lost, the target is intact).
            let _ = sftp.unlink(Path::new(part));
            return Err(format!("Cannot replace '{}': {}", target, e));
        }
        removed_target = true;
    }
    sftp.rename(Path::new(part), Path::new(target), None).map_err(|e| {
        if removed_target {
            // The old target is already gone — keep the part file so the just-uploaded
            // data is not lost too (neither old nor new would otherwise survive).
            format!(
                "Failed to finalize '{}': {} (uploaded data kept at '{}')",
                target, e, part
            )
        } else {
            let _ = sftp.unlink(Path::new(part));
            format!("Failed to finalize '{}': {}", target, e)
        }
    })
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
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    upload_file_inner(profile, local_path, remote_path, cancel, on_progress)
}

fn upload_file_inner(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    // Fail fast on a missing local file before opening a network session.
    std::fs::metadata(local_path)
        .map_err(|e| format!("Failed to open local file '{}': {}", local_path, e))?;
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;
    write_via_part(&sftp, Path::new(local_path), remote_path, "", cancel, &|d, t, _| on_progress(d, t))
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
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    download_file_inner(profile, remote_path, local_path, cancel, on_progress)
}

fn download_file_inner(
    profile: &Profile,
    remote_path: &str,
    local_path: &str,
    cancel: &dyn Fn() -> bool,
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
        if cancel() {
            break Err(CANCELLED_ERROR.to_string());
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

/// Copy a remote file to another remote path via a server-side stream copy
/// (open source, create destination, copy bytes over the same SFTP channel).
/// On failure the partially written destination is removed on a best-effort basis.
pub fn copy_file(profile: &Profile, from: &str, to: &str) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    let mut src = sftp
        .open(Path::new(from))
        .map_err(|e| format!("Failed to open remote file '{}': {}", from, e))?;
    let mut dst = sftp
        .create(Path::new(to))
        .map_err(|e| format!("Failed to create remote file '{}': {}", to, e))?;

    let mut buf = vec![0u8; TRANSFER_CHUNK];
    let result: Result<(), String> = loop {
        let n = match src.read(&mut buf) {
            Ok(v) => v,
            Err(e) => break Err(format!("Read '{}' failed: {}", from, e)),
        };
        if n == 0 {
            break Ok(());
        }
        if let Err(e) = dst.write_all(&buf[..n]) {
            break Err(format!("Write '{}' failed: {}", to, e));
        }
    };
    if let Err(e) = result {
        drop(dst);
        let _ = sftp.unlink(Path::new(to));
        return Err(e);
    }
    Ok(())
}

/// Change the Unix permission bits of a remote file or directory.
/// `mode` is the permission value (e.g. 0o644); only the perm field is set,
/// leaving size/uid/gid/atime/mtime untouched on the server.
pub fn set_permissions(profile: &Profile, path: &str, mode: u32) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    // Preserve the existing file-type bits (S_IFDIR/S_IFREG/etc.) and apply only
    // the permission/special bits from `mode`. Some strict SFTP servers reject a
    // setstat that drops the type bits.
    let type_bits = sftp
        .stat(Path::new(path))
        .ok()
        .and_then(|s| s.perm)
        .map(|p| p & 0o170000)
        .unwrap_or(0);
    let new_perm = type_bits | (mode & 0o7777);

    let stat = ssh2::FileStat {
        size: None,
        uid: None,
        gid: None,
        perm: Some(new_perm),
        atime: None,
        mtime: None,
    };
    sftp.setstat(Path::new(path), stat)
        .map_err(|e| format!("Failed to change permissions of '{}': {}", path, e))
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
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    download_directory_inner(profile, remote_path, local_path, cancel, on_progress)
}

fn download_directory_inner(
    profile: &Profile,
    remote_path: &str,
    local_path: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    std::fs::create_dir_all(local_path)
        .map_err(|e| format!("Failed to create local directory '{}': {}", local_path, e))?;

    download_directory_recursive(cancel, &sftp, remote_path, local_path, on_progress)
}

/// Internal recursive helper for directory download — operates on an open SFTP channel.
fn download_directory_recursive(
    cancel: &dyn Fn() -> bool,
    sftp: &ssh2::Sftp,
    remote_path: &str,
    local_path: &str,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    if cancel() {
        return Err(CANCELLED_ERROR.to_string());
    }
    let entries = sftp
        .readdir(Path::new(remote_path))
        .map_err(|e| format!("Failed to list '{}': {}", remote_path, e))?;

    for (entry_path, stat) in entries {
        let entry_name = match entry_path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        // M7: never download a leftover in-flight upload part file as if it were real content.
        if crate::services::transfer_paths::is_part_file(&entry_name) {
            continue;
        }

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
            download_directory_recursive(cancel, sftp, &entry_path_str, &local_entry, on_progress)?;
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
                if cancel() {
                    break Err(CANCELLED_ERROR.to_string());
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
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    upload_directory_inner(profile, local_path, remote_path, cancel, on_progress)
}

fn upload_directory_inner(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    crate::services::transfer_paths::validate_local_root(local_path)?;
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    mkdir_ok_if_exists(&sftp, Path::new(remote_path))?;
    let mut guard = crate::services::transfer_paths::LoopGuard::new();
    let mut failures: Vec<String> = Vec::new();
    let mut total = 0usize;
    upload_directory_recursive(
        cancel, &sftp, Path::new(local_path), remote_path, on_progress,
        &mut guard, &mut failures, &mut total,
    )?;
    crate::services::transfer_paths::summarize_failures(&failures, total)
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

/// Walks `local_dir`. Per-entry failures are pushed to `failures` and the walk continues;
/// only cancellation returns `Err` early.
#[allow(clippy::too_many_arguments)]
fn upload_directory_recursive(
    cancel: &dyn Fn() -> bool,
    sftp: &ssh2::Sftp,
    local_dir: &Path,
    remote_dir: &str,
    on_progress: &dyn Fn(u64, u64, &str),
    guard: &mut crate::services::transfer_paths::LoopGuard,
    failures: &mut Vec<String>,
    total: &mut usize,
) -> Result<(), String> {
    if cancel() {
        return Err(CANCELLED_ERROR.to_string());
    }
    if !guard.enter(local_dir) {
        return Ok(()); // already on the recursion stack (cycle) or unresolvable
    }
    let read_dir = match std::fs::read_dir(local_dir) {
        Ok(r) => r,
        Err(e) => {
            failures.push(format!("{}: {}", local_dir.display(), e));
            guard.leave(local_dir);
            return Ok(());
        }
    };

    for entry in read_dir {
        if cancel() {
            guard.leave(local_dir);
            return Err(CANCELLED_ERROR.to_string());
        }
        let local_entry = match entry {
            Ok(e) => e.path(),
            Err(e) => { failures.push(format!("{}: {}", local_dir.display(), e)); continue; }
        };
        let entry_name = match local_entry.file_name() {
            Some(n) if !n.is_empty() => n.to_string_lossy().to_string(),
            _ => continue,
        };
        // M7: never re-upload a leftover in-flight part file as if it were real content.
        if crate::services::transfer_paths::is_part_file(&entry_name) {
            continue;
        }
        let remote_entry = format!("{}/{}", remote_dir.trim_end_matches('/'), entry_name);

        if local_entry.is_dir() {
            *total += 1;
            if let Err(e) = mkdir_ok_if_exists(sftp, Path::new(&remote_entry)) {
                failures.push(format!("{}: {}", entry_name, e));
                continue;
            }
            upload_directory_recursive(cancel, sftp, &local_entry, &remote_entry, on_progress, guard, failures, total)?;
        } else if local_entry.is_file() {
            *total += 1;
            match write_via_part(sftp, &local_entry, &remote_entry, &entry_name, cancel, on_progress) {
                Ok(()) => {}
                Err(e) if e == CANCELLED_ERROR => { guard.leave(local_dir); return Err(e); }
                Err(e) => failures.push(format!("{}: {}", entry_name, e)),
            }
        }
        // Broken symlinks and special files are skipped without error.
    }
    guard.leave(local_dir);
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

/// True when `path` is a directory (stat follows symlinks).
pub fn is_remote_dir(profile: &Profile, path: &str) -> Result<bool, String> {
    let session = connect(profile)?;
    let sftp = session.sftp().map_err(|e| format!("Failed to open SFTP channel: {}", e))?;
    let stat = sftp.stat(Path::new(path)).map_err(|e| format!("Cannot stat '{}': {}", path, e))?;
    Ok(stat.is_dir())
}

/// Sentinel returned by `exec_command` when `opts.timeout` elapses before the
/// remote command finishes. Distinct from a generic failure so callers (e.g. the
/// exec-capability probe) can tell "the server didn't answer in time" apart from
/// "the connection/channel could not even be opened".
pub const EXEC_TIMEOUT_ERROR: &str = "EXEC_TIMEOUT";

/// Options for `exec_command`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExecOptions {
    /// Request a PTY before `exec`. Without a PTY, closing the channel on cancel
    /// does not signal the remote process — it keeps running detached from the
    /// (now-gone) channel. With a PTY, closing the channel hangs up the line
    /// (SIGHUP) and the remote process group is killed, so cancel of a PTY exec
    /// actually stops the remote command. Side effect: with a PTY the remote
    /// stderr is merged into the stdout stream, so `stderr` returned by
    /// `exec_command` will typically be empty.
    pub pty: bool,
    /// Hard deadline for the whole exec (not per-chunk). `None` means no timeout
    /// (only `cancel` can stop it). On expiry the channel is closed and
    /// `EXEC_TIMEOUT_ERROR` is returned.
    pub timeout: Option<std::time::Duration>,
}

/// Run `cmd` on the server over an exec channel. Polls non-blocking so `cancel` is honoured
/// and long commands are not cut by the per-op timeout. Returns (exit status, stdout, stderr).
///
/// Cancelling a non-PTY exec only closes our side of the channel — the remote command
/// keeps running server-side. Pass `opts.pty = true` when the command must actually be
/// killed on cancel (see `ExecOptions::pty`).
pub fn exec_command(
    profile: &Profile,
    cmd: &str,
    opts: ExecOptions,
    cancel: &dyn Fn() -> bool,
) -> Result<(i32, String, String), String> {
    let session = connect(profile)?;
    let mut channel = session
        .channel_session()
        .map_err(|e| format!("Cannot open exec channel: {}", e))?;
    if opts.pty {
        channel
            .request_pty("dumb", None, None)
            .map_err(|e| format!("Cannot request pty: {}", e))?;
    }
    channel.exec(cmd).map_err(|e| format!("Exec failed: {}", e))?;
    // We never send stdin; signal EOF immediately so commands that read stdin
    // (there shouldn't be any) don't block forever waiting for input.
    let _ = channel.send_eof();

    session.set_blocking(false);
    let start = std::time::Instant::now();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if cancel() {
            session.set_blocking(true);
            let _ = channel.close();
            return Err(CANCELLED_ERROR.to_string());
        }
        if let Some(timeout) = opts.timeout {
            if start.elapsed() >= timeout {
                session.set_blocking(true);
                let _ = channel.close();
                return Err(EXEC_TIMEOUT_ERROR.to_string());
            }
        }
        let mut progressed = false;
        match channel.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => { out.extend_from_slice(&buf[..n]); progressed = true; }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => { session.set_blocking(true); return Err(format!("Exec read failed: {}", e)); }
        }
        match channel.stderr().read(&mut buf) {
            Ok(0) => {}
            Ok(n) => { err.extend_from_slice(&buf[..n]); progressed = true; }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => { session.set_blocking(true); return Err(format!("Exec read failed: {}", e)); }
        }
        if channel.eof() {
            break;
        }
        if !progressed {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    session.set_blocking(true);
    let _ = channel.wait_close();
    let status = channel.exit_status().unwrap_or(-1);
    Ok((status, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned()))
}
