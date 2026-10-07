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
    validate_target(profile)?;
    let mut ssh_args = build_ssh_args(profile, use_runtime_copy);
    let settings = crate::services::settings_service::get_settings().unwrap_or_default();
    let (program, prefix) = crate::services::terminal_service::resolve(&settings)?;
    let mut cmd = Command::new(&program);
    cmd.args(&prefix);

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
    cmd.arg("bash")
        .arg("-c")
        .arg(TERMINAL_SCRIPT)
        .arg("--")
        .args(&ssh_args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch terminal '{}': {}", program, e))
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
        // Offer only the configured key: no ssh-agent keys, no IdentityFile /
        // CertificateFile entries from ~/.ssh/config (also avoids MaxAuthTries).
        args.push("-o".to_string());
        args.push("IdentitiesOnly=yes".to_string());
    }

    if profile.auth_type == AuthType::Password {
        // Password profiles must not offer ssh-agent / default keys: servers cap the
        // auth attempts (MaxAuthTries) and answer "Too many authentication failures"
        // before the password step is ever reached.
        args.extend(password_only_args());
    }

    // Never let ~/.ssh/config forward the agent / X11 or run local commands.
    args.extend(hardening_args());

    // "--" ends option parsing so a profile field can never be read as an ssh option.
    args.push("--".to_string());
    args.push(format!("{}@{}", profile.username, profile.host));

    args
}

/// Reject profile fields that could be misread as ssh options or break the target.
pub fn validate_target(profile: &Profile) -> Result<(), String> {
    for (name, v) in [("username", &profile.username), ("host", &profile.host)] {
        if v.is_empty()
            || v.starts_with('-')
            || v.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(format!("Invalid {} in profile", name));
        }
    }
    if profile.host.contains('@') {
        return Err("Invalid host in profile".to_string());
    }
    Ok(())
}

/// Options applied to every ssh invocation, overriding ~/.ssh/config.
pub fn hardening_args() -> Vec<String> {
    ["ForwardAgent=no", "ForwardX11=no", "PermitLocalCommand=no"]
        .iter()
        .flat_map(|o| ["-o".to_string(), o.to_string()])
        .collect()
}

/// ssh options that restrict authentication to password / keyboard-interactive.
/// Command-line options win over ~/.ssh/config, so no key, certificate, agent
/// identity, GSSAPI or host-based method can be offered for a password profile.
pub fn password_only_args() -> Vec<String> {
    [
        "PubkeyAuthentication=no",
        "PreferredAuthentications=password,keyboard-interactive",
        "IdentitiesOnly=yes",
        "IdentityAgent=none",
        "PasswordAuthentication=yes",
        "GSSAPIAuthentication=no",
        "HostbasedAuthentication=no",
    ]
    .iter()
    .flat_map(|o| ["-o".to_string(), o.to_string()])
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::profile::{AuthType, Profile, UploadMode};

    fn mk(auth_type: AuthType, key_path: Option<&str>) -> Profile {
        Profile {
            id: "p1".to_string(),
            name: "t".to_string(),
            host: "example.com".to_string(),
            port: 222,
            username: "kaimsf".to_string(),
            auth_type,
            key_path: key_path.map(str::to_string),
            default_remote_path: None,
            editor_command: None,
            upload_mode: UploadMode::Auto,
            protocol: None,
            local_path: None,
            credential_storage_mode: None,
            stored_secret_portable: None,
            local_paths_by_user: None,
            group: None,
            created_at: None,
            directory_cache: None,
        }
    }

    #[test]
    fn password_profile_never_offers_keys() {
        let args = build_ssh_args(&mk(AuthType::Password, None), false);
        assert!(args.contains(&"PubkeyAuthentication=no".to_string()));
        assert!(args
            .contains(&"PreferredAuthentications=password,keyboard-interactive".to_string()));
        // target must stay the last argument
        assert_eq!(args.last().unwrap(), "kaimsf@example.com");
    }

    #[test]
    fn key_profile_keeps_key_auth() {
        let args = build_ssh_args(&mk(AuthType::Key, Some("/k/id")), false);
        assert!(!args.contains(&"PubkeyAuthentication=no".to_string()));
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"IdentitiesOnly=yes".to_string()));
    }
}
