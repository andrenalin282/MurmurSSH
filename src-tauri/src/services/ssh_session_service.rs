/// SSH session management for terminal SSO.
///
/// After a successful SFTP authentication, this service may establish a background
/// SSH connection so the terminal window can reuse the already-authenticated
/// session without prompting the user again.
///
/// **Password auth → SSH ControlMaster**
///   An SSH process runs as a ControlMaster (`-M -N`) on a Unix socket in /tmp.
///   The terminal is launched with `-o ControlPath=<socket> -o ControlMaster=no`
///   so it reuses the master's authenticated connection.
///   The password is provided to the SSH master via SSH_ASKPASS_REQUIRE=force and
///   a short-lived temp file (0600) that is deleted immediately after the master
///   connects. The password is never passed as a command-line argument.
///
/// **Key auth (with or without passphrase) / Agent auth**
///   No background session is established. The terminal is launched with
///   `ssh -i key_path` and the terminal handles passphrase prompting interactively.
///   This is more reliable than ssh-agent injection, which depends on
///   SSH_ASKPASS_REQUIRE (OpenSSH ≥ 8.4) and agent socket stability.
///
/// **Disconnect**
///   `stop_session()` kills the ControlMaster process and removes the socket file.
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::models::{AuthType, Profile};
use crate::services::credentials_store;

// ── Session store ─────────────────────────────────────────────────────────────

enum SessionKind {
    ControlMaster { socket_path: PathBuf },
}

struct SshSession {
    kind: SessionKind,
    child: Child,
}

static SESSIONS: OnceLock<Mutex<HashMap<String, SshSession>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<String, SshSession>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── Path helpers ──────────────────────────────────────────────────────────────

/// Directory for app-private runtime state (ControlMaster sockets, etc.).
///
/// Uses `~/.config/murmurssh/run/` with `0700` permissions so the socket path
/// is not predictable in world-writable `/tmp` (audit F17). Falls back to `/`
/// on an unset `$HOME` to avoid panicking under `panic=abort`.
fn run_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config").join("murmurssh").join("run")
}

fn ensure_run_dir() -> PathBuf {
    let dir = run_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    dir
}

fn ctrl_socket_path(profile_id: &str) -> PathBuf {
    // Sanitise the profile id in case it contains path separators or dots.
    let safe: String = profile_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    run_dir().join(format!("{}.sock", safe))
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Start an SSH session for terminal SSO based on the profile's auth type.
///
/// - Password: establishes an SSH ControlMaster so the terminal reuses the
///   already-authenticated connection without re-prompting.
/// - Key auth (with or without passphrase): no-op. The terminal is launched
///   with `ssh -i key_path` and handles passphrase prompting interactively.
///   This is more reliable than the ssh-agent injection approach which depends
///   on SSH_ASKPASS_REQUIRE (OpenSSH ≥ 8.4) and agent socket availability.
/// - Agent auth: no-op (system SSH_AUTH_SOCK handles it).
///
/// Returns Ok(()) even if no session is needed.
/// Returns Err only if a ControlMaster session was attempted but failed.
pub fn start_session(profile: &Profile) -> Result<(), String> {
    // Idempotent: if a session already exists, reuse it.
    // On a poisoned mutex we treat the store as empty and try to re-establish the
    // session rather than aborting the app (panic=abort would otherwise kill it).
    if let Ok(map) = sessions().lock() {
        if map.contains_key(&profile.id) {
            return Ok(());
        }
    }

    match profile.auth_type {
        AuthType::Password => {
            let creds = credentials_store::get(&profile.id);
            let password = creds
                .password
                .ok_or_else(|| "No password in session store for SSH SSO".to_string())?;
            start_control_master(profile, &password)
        }
        // Key auth: terminal handles passphrase interactively via -i key_path.
        // Agent auth: system SSH_AUTH_SOCK is inherited by the terminal.
        AuthType::Key | AuthType::Agent => Ok(()),
    }
}

/// Stop and clean up the SSH session for a profile (called on disconnect).
pub fn stop_session(profile_id: &str) {
    // On a poisoned mutex skip cleanup rather than aborting the app; the OS will
    // reap the ssh process when the app exits and startup cleanup removes stale sockets.
    let Ok(mut map) = sessions().lock() else { return; };
    if let Some(mut session) = map.remove(profile_id) {
        let _ = session.child.kill();
        let _ = session.child.wait(); // reap to avoid zombies
        let SessionKind::ControlMaster { socket_path } = &session.kind;
        let _ = fs::remove_file(socket_path);
    }
}

pub struct SessionExtras {
    /// Extra arguments appended to the `ssh` command inside the terminal.
    pub extra_args: Vec<String>,
    /// Environment variables to set on the terminal emulator process.
    pub env: HashMap<String, String>,
}

/// Returns extra SSH args / env vars to inject into the terminal launch for SSO.
/// Returns None if no session exists (user must authenticate manually).
/// Only ControlMaster sessions (password auth) are tracked here.
pub fn get_session_extras(profile_id: &str) -> Option<SessionExtras> {
    // Treat a poisoned mutex as "no session" so the terminal falls back to a
    // normal (re-authenticating) launch instead of panic-aborting the app.
    let map = sessions().lock().ok()?;
    let session = map.get(profile_id)?;
    let SessionKind::ControlMaster { socket_path } = &session.kind;
    Some(SessionExtras {
        extra_args: vec![
            "-o".to_string(),
            format!("ControlPath={}", socket_path.to_string_lossy()),
            "-o".to_string(),
            "ControlMaster=no".to_string(),
        ],
        env: HashMap::new(),
    })
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Write a credential to a 0600 temp file and an SSH_ASKPASS wrapper script (0700).
/// Returns (pw_file, askpass_script). The caller MUST delete both files after use.
fn write_askpass_pair(secret: &str) -> Result<(PathBuf, PathBuf), String> {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);

    let pw_file = std::env::temp_dir().join(format!(".murmurssh-pw-{}", suffix));
    let ask_script = std::env::temp_dir().join(format!(".murmurssh-ask-{}.sh", suffix));

    fs::write(&pw_file, format!("{}\n", secret))
        .map_err(|e| format!("Failed to write temp credential: {}", e))?;
    fs::set_permissions(&pw_file, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("Failed to chmod temp credential: {}", e))?;

    let script = format!("#!/bin/sh\ncat '{}'\n", pw_file.to_string_lossy());
    fs::write(&ask_script, &script)
        .map_err(|e| format!("Failed to write askpass script: {}", e))?;
    fs::set_permissions(&ask_script, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("Failed to chmod askpass script: {}", e))?;

    Ok((pw_file, ask_script))
}

fn cleanup_pair(pw: &Path, ask: &Path) {
    let _ = fs::remove_file(pw);
    let _ = fs::remove_file(ask);
}

fn wait_for_socket(path: &Path, timeout_secs: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while !path.exists() {
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    true
}

/// Establish an SSH ControlMaster for password-based SSO.
fn start_control_master(profile: &Profile, password: &str) -> Result<(), String> {
    use std::process::Command;

    // Make sure the app's run dir exists with 0700 before the socket is created.
    ensure_run_dir();
    let socket_path = ctrl_socket_path(&profile.id);
    // Remove any stale socket from a previous session
    if socket_path.exists() {
        let _ = fs::remove_file(&socket_path);
    }

    let (pw_file, ask_script) = write_askpass_pair(password)?;

    let mut ssh_args = vec![
        "-M".to_string(),
        "-N".to_string(),
        "-o".to_string(),
        format!("ControlPath={}", socket_path.to_string_lossy()),
        // We've already verified the host key via MurmurSSH's own known_hosts,
        // so bypass OpenSSH's separate verification for the ControlMaster.
        "-o".to_string(),
        "StrictHostKeyChecking=no".to_string(),
        "-o".to_string(),
        "BatchMode=no".to_string(),
        "-o".to_string(),
        "ConnectTimeout=15".to_string(),
    ];

    if profile.port != 22 {
        ssh_args.push("-p".to_string());
        ssh_args.push(profile.port.to_string());
    }
    ssh_args.push(format!("{}@{}", profile.username, profile.host));

    let child = Command::new("ssh")
        .args(&ssh_args)
        .env("SSH_ASKPASS", ask_script.to_string_lossy().as_ref())
        // SSH_ASKPASS_REQUIRE=force tells OpenSSH to use SSH_ASKPASS even
        // without DISPLAY set and without an attached terminal (OpenSSH ≥ 8.4).
        .env("SSH_ASKPASS_REQUIRE", "force")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            cleanup_pair(&pw_file, &ask_script);
            format!("Failed to start SSH ControlMaster: {}", e)
        })?;

    // Wait for the ControlMaster socket to appear (up to 15 s)
    if !wait_for_socket(&socket_path, 15) {
        cleanup_pair(&pw_file, &ask_script);
        return Err(
            "SSH ControlMaster timed out — password may be incorrect or host unreachable"
                .to_string(),
        );
    }

    // Delete sensitive temp files immediately after the master is up
    cleanup_pair(&pw_file, &ask_script);

    // A poisoned session store means we cannot track this child, so we must not
    // leak it: kill the master we just spawned and report a soft failure instead
    // of panic-aborting the app. Terminal launches will fall back to re-auth.
    let new_session = SshSession {
        kind: SessionKind::ControlMaster { socket_path: socket_path.clone() },
        child,
    };
    match sessions().lock() {
        Ok(mut map) => {
            map.insert(profile.id.clone(), new_session);
            Ok(())
        }
        Err(_) => {
            let mut session = new_session;
            let _ = session.child.kill();
            let _ = session.child.wait();
            let _ = fs::remove_file(&socket_path);
            Err("SSH session store unavailable (internal lock poisoned)".to_string())
        }
    }
}

