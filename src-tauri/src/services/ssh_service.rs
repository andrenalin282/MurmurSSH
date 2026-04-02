use std::process::Command;

use crate::models::{AuthType, Profile};
use crate::services::{runtime_key_service, ssh_session_service};

/// Shell wrapper script used when launching SSH in a terminal.
///
/// Uses `"$@"` to expand SSH arguments from positional parameters — no argument
/// value is ever interpolated into the script string, eliminating any injection
/// risk regardless of argument content (special characters, spaces, quotes, etc.).
///
/// Invoked as: bash -c TERMINAL_SCRIPT -- ssh [args...]
///   "--" becomes $0 (script name placeholder); ssh and its args are $1, $2, …
///   "$@" expands to all positional params from $1 onward = the ssh invocation.
///
/// On success (SSH exits 0) the terminal closes normally.
/// On failure the terminal shows the exit code and waits for Enter before closing.
const TERMINAL_SCRIPT: &str = concat!(
    r#""$@"; _rc=$?; "#,
    r#"if [ "$_rc" -ne 0 ]; then "#,
    r#"printf '\nSSH exited with code %d.\n' "$_rc"; "#,
    r#"read -rp 'Press Enter to close this window.'; "#,
    r#"fi"#
);

/// Launch an SSH session in the system terminal emulator.
///
/// If `use_runtime_copy` is true and the profile uses key auth, a previously
/// created runtime copy of the key (in ~/.config/murmurssh/runtime-keys/) is used
/// for the terminal launch instead of the original key path. This fixes the
/// "UNPROTECTED PRIVATE KEY FILE" rejection from OpenSSH when keys are stored
/// on mounted or network filesystems with incompatible permissions.
///
/// The runtime copy must be created before calling this function by the command
/// layer (which also presents the user prompt for informed consent).
pub fn launch_ssh(profile: &Profile, use_runtime_copy: bool) -> Result<(), String> {
    let mut ssh_args = build_ssh_args(profile, use_runtime_copy);
    let mut cmd = Command::new("x-terminal-emulator");

    // Inject ControlMaster session extras for password-auth profiles only.
    // Key-auth profiles (with or without passphrase) use the direct -i path so
    // the terminal can prompt for the passphrase interactively when needed.
    // This avoids the fragile SSH_ASKPASS / ssh-agent injection path for key auth.
    if profile.auth_type == AuthType::Password {
        if let Some(extras) = ssh_session_service::get_session_extras(&profile.id) {
            // Insert extra SSH options right after "ssh" (before host args)
            for (i, arg) in extras.extra_args.into_iter().enumerate() {
                ssh_args.insert(1 + i, arg);
            }
            for (k, v) in extras.env {
                cmd.env(k, v);
            }
        }
    }

    // Pass SSH arguments as positional parameters to the static script.
    // bash -c TERMINAL_SCRIPT -- ssh [args...]:
    //   "--" is $0 (script name placeholder); ssh and its args are $1, $2, …
    //   "$@" in the script expands to the full ssh invocation, never interpolated.
    cmd.arg("-e")
        .arg("bash")
        .arg("-c")
        .arg(TERMINAL_SCRIPT)
        .arg("--")
        .args(&ssh_args)
        .spawn()
        .map(|_| ())
        .map_err(|e| {
            format!(
                "Failed to launch terminal: {}. Is x-terminal-emulator installed?",
                e
            )
        })
}

fn build_ssh_args(profile: &Profile, use_runtime_copy: bool) -> Vec<String> {
    let mut args = vec!["ssh".to_string()];

    if profile.port != 22 {
        args.push("-p".to_string());
        args.push(profile.port.to_string());
    }

    // Add a connection timeout so the terminal does not hang indefinitely
    args.push("-o".to_string());
    args.push("ConnectTimeout=15".to_string());

    if profile.auth_type == AuthType::Key {
        // Determine which key path to use:
        // - If use_runtime_copy is true and a runtime copy exists, use it.
        // - Otherwise use the configured key path directly.
        let key_path = if use_runtime_copy {
            runtime_key_service::get_runtime_key_path(&profile.id)
                .and_then(|p| if p.exists() { Some(p.to_string_lossy().into_owned()) } else { None })
                .or_else(|| profile.key_path.clone())
        } else {
            profile.key_path.clone()
        };

        if let Some(kp) = key_path {
            args.push("-i".to_string());
            args.push(kp);
        }
        // Disable SSH password fallback for key-auth profiles.
        // Without this, OpenSSH may prompt for a password when key auth fails,
        // which is confusing for key-only setups.
        args.push("-o".to_string());
        args.push("PasswordAuthentication=no".to_string());
    }

    args.push(format!("{}@{}", profile.username, profile.host));

    args
}

