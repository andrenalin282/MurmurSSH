use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

use ssh2::Session;

use crate::models::{AuthType, FileEntry, Profile};

/// Opens an authenticated SSH session to the remote host in the given profile.
/// Each SFTP operation calls this to get a fresh connection — simple and stateless.
fn connect(profile: &Profile) -> Result<Session, String> {
    let addr = format!("{}:{}", profile.host, profile.port);

    let tcp = TcpStream::connect(&addr)
        .map_err(|e| format!("Connection to {} failed: {}", addr, e))?;

    let mut session =
        Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    match &profile.auth_type {
        AuthType::Key => {
            let key_path = profile
                .key_path
                .as_deref()
                .ok_or("Key path is required for key authentication")?;

            session
                .userauth_pubkey_file(
                    &profile.username,
                    None, // ssh2 derives the public key from the private key
                    Path::new(key_path),
                    None, // no passphrase
                )
                .map_err(|e| format!("Key authentication failed ({}): {}", key_path, e))?;
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
