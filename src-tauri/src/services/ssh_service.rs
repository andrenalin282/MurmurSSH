use std::process::Command;

use crate::models::{AuthType, Profile};

/// Launches an SSH session by opening the system terminal emulator.
///
/// On Debian/Ubuntu, `x-terminal-emulator` is a symlink to the user's configured
/// terminal. The SSH command is passed as separate arguments after `-e`, which is
/// compatible with xterm, lxterminal, gnome-terminal, and other common terminals.
pub fn launch_ssh(profile: &Profile) -> Result<(), String> {
    let ssh_args = build_ssh_args(profile);

    Command::new("x-terminal-emulator")
        .arg("-e")
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
