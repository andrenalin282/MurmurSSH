use std::process::Command;

use crate::models::{AuthType, Profile};
use crate::services::{runtime_key_service, ssh_session_service};

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

    // Build a shell command string that runs SSH and keeps the terminal open
    // on failure so the user can read the error before the window closes.
    // On success (exit code 0) the terminal closes normally when SSH exits.
    let shell_cmd = build_shell_cmd(&ssh_args);

    cmd.arg("-e")
        .arg("bash")
        .arg("-c")
        .arg(&shell_cmd)
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

/// Build a bash shell command string that runs SSH and pauses on failure.
///
/// If SSH exits with a non-zero code the terminal stays open so the user can
/// read the error message. On success (exit code 0) the terminal closes when
/// SSH exits normally (after the remote session ends).
///
/// Each argument is single-quoted and internal single-quotes are escaped so
/// paths with spaces or special characters are handled safely.
fn build_shell_cmd(ssh_args: &[String]) -> String {
    let quoted: Vec<String> = ssh_args
        .iter()
        .map(|arg| {
            // Wrap in single-quotes; escape any embedded single-quotes as '\''
            format!("'{}'", arg.replace('\'', "'\\''"))
        })
        .collect();
    let ssh_invocation = quoted.join(" ");

    // Run SSH; if it fails, show the exit code and wait for Enter before closing.
    format!(
        "{ssh}; _rc=$?; if [ $_rc -ne 0 ]; then echo ''; echo \"SSH exited with code $_rc.\"; read -rp 'Press Enter to close this window.'; fi",
        ssh = ssh_invocation,
    )
}
