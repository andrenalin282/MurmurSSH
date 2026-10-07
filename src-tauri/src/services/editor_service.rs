//! Shared editor command resolution and launch.
//!
//! Resolution order (PRD §15.3):
//! per-profile override → per-extension map → global default → None (caller uses xdg-open).

use crate::models::Settings;
use std::path::Path;

/// Normalize an extension key: strip leading dots, lowercase, trim.
pub fn normalize_extension(ext: &str) -> String {
    ext.trim().trim_start_matches('.').to_lowercase()
}

/// Resolve which editor command string to use for `path`.
///
/// Returns `None` when nothing is configured — caller should fall back to `xdg-open`.
pub fn resolve_editor(
    profile_override: Option<&str>,
    path: &Path,
    settings: &Settings,
) -> Option<String> {
    if let Some(cmd) = profile_override {
        let trimmed = cmd.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    if let Some(map) = &settings.editor_by_extension {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let key = normalize_extension(ext);
            if let Some(cmd) = map.get(&key) {
                let trimmed = cmd.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    if let Some(cmd) = &settings.default_editor {
        let trimmed = cmd.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

/// Programs that turn a "command" into arbitrary code execution (shells, interpreters,
/// privilege/launcher wrappers, network fetchers). lc-debt: denylist, not an allowlist;
/// upgrade to an explicit allowlist or confirmation for profile-supplied commands.
const FORBIDDEN_PROGRAMS: &[&str] = &[
    "sh", "bash", "dash", "zsh", "fish", "ksh", "csh", "tcsh", "ash", "busybox", "env", "sudo",
    "doas", "su", "pkexec", "xargs", "nohup", "setsid", "timeout", "nice", "eval", "exec",
    "python", "python2", "python3", "perl", "ruby", "node", "nodejs", "php", "lua", "tclsh",
    "awk", "gawk", "curl", "wget", "nc", "ncat", "netcat", "socat", "ssh", "scp", "rsync", "find",
];

/// Reject command lines whose program is on the forbidden list.
pub fn check_command(cmd: &str) -> Result<(), String> {
    let first = cmd.split_whitespace().next().ok_or("Command is empty")?;
    let base = Path::new(first)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let stem = base.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    if FORBIDDEN_PROGRAMS.contains(&base.as_str()) || FORBIDDEN_PROGRAMS.contains(&stem) {
        return Err(format!("Command '{}' is not allowed as editor/terminal", first));
    }
    Ok(())
}

/// Spawn `editor` (whitespace-split argv) with `path`, or `xdg-open` when `editor` is None/empty.
pub fn launch_editor(editor: Option<&str>, path: &Path) -> Result<(), String> {
    let path_str = path.to_string_lossy();
    match editor {
        Some(e) if !e.trim().is_empty() => {
            check_command(e)?;
            let mut parts = e.split_whitespace();
            let cmd = parts.next().ok_or("Editor command is empty")?;
            let extra_args: Vec<&str> = parts.collect();
            std::process::Command::new(cmd)
                .args(&extra_args)
                .arg(path_str.as_ref())
                .spawn()
                .map(|_| ())
                .map_err(|err| format!("Failed to launch editor '{}': {}", e, err))
        }
        _ => std::process::Command::new("xdg-open")
            .arg(path_str.as_ref())
            .spawn()
            .map(|_| ())
            .map_err(|e| {
                format!(
                    "Failed to open file with xdg-open: {}. Is xdg-utils installed?",
                    e
                )
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn settings_with(
        default: Option<&str>,
        map: Vec<(&str, &str)>,
    ) -> Settings {
        let mut s = Settings::default();
        s.default_editor = default.map(|d| d.to_string());
        if !map.is_empty() {
            s.editor_by_extension = Some(
                map.into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<HashMap<_, _>>(),
            );
        }
        s
    }

    #[test]
    fn forbidden_programs_are_rejected() {
        for c in ["sh -c evil", "/bin/bash x", "python3 a.py", "env FOO=1 vim", "PYTHON3.11 x", "curl http://x"] {
            assert!(check_command(c).is_err(), "{c}");
        }
        for c in ["code --wait", "/usr/bin/gedit", "kate", "nvim-qt"] {
            assert!(check_command(c).is_ok(), "{c}");
        }
    }

    #[test]
    fn normalize_strips_dot_and_lowercases() {
        assert_eq!(normalize_extension(".CONF"), "conf");
        assert_eq!(normalize_extension("Py"), "py");
        assert_eq!(normalize_extension("  .Ts  "), "ts");
    }

    #[test]
    fn profile_override_wins() {
        let s = settings_with(Some("gedit"), vec![("conf", "nano")]);
        let path = PathBuf::from("/tmp/app.conf");
        assert_eq!(
            resolve_editor(Some("code --wait"), &path, &s).as_deref(),
            Some("code --wait")
        );
    }

    #[test]
    fn extension_map_case_insensitive() {
        let s = settings_with(Some("gedit"), vec![("conf", "nano")]);
        let path = PathBuf::from("/tmp/App.CONF");
        assert_eq!(
            resolve_editor(None, &path, &s).as_deref(),
            Some("nano")
        );
    }

    #[test]
    fn falls_back_to_default_editor() {
        let s = settings_with(Some("gedit"), vec![("py", "code")]);
        let path = PathBuf::from("/tmp/readme.md");
        assert_eq!(
            resolve_editor(None, &path, &s).as_deref(),
            Some("gedit")
        );
    }

    #[test]
    fn empty_returns_none_for_xdg_open() {
        let s = Settings::default();
        let path = PathBuf::from("/tmp/readme.md");
        assert_eq!(resolve_editor(None, &path, &s), None);
        assert_eq!(resolve_editor(Some("  "), &path, &s), None);
    }

    #[test]
    fn blank_profile_falls_through_to_extension() {
        let s = settings_with(Some("gedit"), vec![("conf", "nano")]);
        let path = PathBuf::from("/tmp/app.conf");
        assert_eq!(
            resolve_editor(Some(""), &path, &s).as_deref(),
            Some("nano")
        );
    }
}
