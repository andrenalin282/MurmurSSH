use std::process::Command;

use crate::models::{AuthType, Profile};
use crate::services::ssh_session_service;

/// Launches an SSH session by opening the system terminal emulator.
///
/// If an SSH session has been established for this profile (via `start_session`),
/// the terminal is launched with extra args/env to reuse the authenticated
/// connection (ControlMaster or ssh-agent), so the user is not re-prompted.
///
/// On Debian/Ubuntu, `x-terminal-emulator` is a symlink to the user's configured
/// terminal. The SSH command is passed as separate arguments after `-e`, which is
/// compatible with xterm, lxterminal, gnome-terminal, and other common terminals.
pub fn launch_ssh(profile: &Profile) -> Result<(), String> {
    let mut ssh_args = build_ssh_args(profile);
    let mut cmd = Command::new("x-terminal-emulator");

    // Inject session extras (ControlMaster args or SSH_AUTH_SOCK) if available
    if let Some(extras) = ssh_session_service::get_session_extras(&profile.id) {
        // Insert extra SSH options right after "ssh" (before host args)
        for (i, arg) in extras.extra_args.into_iter().enumerate() {
            ssh_args.insert(1 + i, arg);
        }
        for (k, v) in extras.env {
            cmd.env(k, v);
        }
    }

    cmd.arg("-e")
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

fn build_ssh_args(profile: &Profile) -> Vec<String> {
    let mut args = vec!["ssh".to_string()];

    if profile.port != 22 {
        args.push("-p".to_string());
        args.push(profile.port.to_string());
    }

    if profile.auth_type == AuthType::Key {
        if let Some(key_path) = &profile.key_path {
            args.push("-i".to_string());
            args.push(key_path.clone());
        }
    }

    args.push(format!("{}@{}", profile.username, profile.host));

    args
}
