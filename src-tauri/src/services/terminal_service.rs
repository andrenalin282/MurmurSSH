//! Terminal emulator selection for SSH sessions.

use std::path::Path;

use crate::models::Settings;

pub const NO_TERMINAL_ERROR: &str = "NO_TERMINAL";

pub struct TerminalSpec {
    pub id: &'static str,
    pub binary: &'static str,
    /// Args placed between the binary and the command to run.
    pub prefix: &'static [&'static str],
}

/// Auto-detection order. `x-terminal-emulator` first keeps Debian/Ubuntu behaviour unchanged.
pub const KNOWN_TERMINALS: &[TerminalSpec] = &[
    TerminalSpec { id: "x-terminal-emulator", binary: "x-terminal-emulator", prefix: &["-e"] },
    TerminalSpec { id: "gnome-terminal", binary: "gnome-terminal", prefix: &["--"] },
    TerminalSpec { id: "ptyxis", binary: "ptyxis", prefix: &["--"] },
    TerminalSpec { id: "kgx", binary: "kgx", prefix: &["--"] },
    TerminalSpec { id: "konsole", binary: "konsole", prefix: &["-e"] },
    TerminalSpec { id: "xfce4-terminal", binary: "xfce4-terminal", prefix: &["-x"] },
    TerminalSpec { id: "kitty", binary: "kitty", prefix: &[] },
    TerminalSpec { id: "alacritty", binary: "alacritty", prefix: &["-e"] },
    TerminalSpec { id: "foot", binary: "foot", prefix: &[] },
    TerminalSpec { id: "wezterm", binary: "wezterm", prefix: &["start", "--"] },
    TerminalSpec { id: "xterm", binary: "xterm", prefix: &["-e"] },
];

fn on_path(binary: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| is_executable(&dir.join(binary)))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// Ids of known terminals installed on this machine, in detection order.
pub fn detect() -> Vec<String> {
    detect_with(&on_path)
}

fn detect_with(exists: &dyn Fn(&str) -> bool) -> Vec<String> {
    KNOWN_TERMINALS.iter().filter(|t| exists(t.binary)).map(|t| t.id.to_string()).collect()
}

fn spec_to_pair(t: &TerminalSpec) -> (String, Vec<String>) {
    (t.binary.to_string(), t.prefix.iter().map(|s| s.to_string()).collect())
}

pub fn resolve(settings: &Settings) -> Result<(String, Vec<String>), String> {
    let env_terminal = std::env::var("TERMINAL").ok();
    resolve_with(settings, env_terminal.as_deref(), &on_path)
}

fn resolve_with(
    settings: &Settings,
    env_terminal: Option<&str>,
    exists: &dyn Fn(&str) -> bool,
) -> Result<(String, Vec<String>), String> {
    let choice = settings.terminal.as_deref().unwrap_or("auto");

    if choice == "custom" {
        // lc-debt: whitespace split, no shell quoting; upgrade to a shell-words parser if users need quoted args.
        let raw = settings.terminal_custom_command.as_deref().unwrap_or("").trim();
        if !raw.is_empty() {
            crate::services::editor_service::check_command(raw)?;
        }
        let mut parts = raw.split_whitespace().map(str::to_string);
        return match parts.next() {
            Some(program) => Ok((program, parts.collect())),
            None => Err(NO_TERMINAL_ERROR.to_string()),
        };
    }

    if choice != "auto" {
        if let Some(t) = KNOWN_TERMINALS.iter().find(|t| t.id == choice) {
            return Ok(spec_to_pair(t));
        }
    }

    if let Some(term) = env_terminal.map(str::trim).filter(|s| !s.is_empty()) {
        let base = Path::new(term).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let known = KNOWN_TERMINALS.iter().find(|t| t.binary == base);
        if Path::new(term).is_absolute() || exists(term) {
            let prefix = known.map(|t| t.prefix.iter().map(|s| s.to_string()).collect()).unwrap_or_else(|| vec!["-e".to_string()]);
            return Ok((term.to_string(), prefix));
        }
    }

    KNOWN_TERMINALS
        .iter()
        .find(|t| exists(t.binary))
        .map(spec_to_pair)
        .ok_or_else(|| NO_TERMINAL_ERROR.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(terminal: Option<&str>, custom: Option<&str>) -> Settings {
        Settings {
            terminal: terminal.map(str::to_string),
            terminal_custom_command: custom.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn auto_on_arch_picks_first_installed_known_terminal() {
        let exists = |b: &str| b == "konsole" || b == "kitty";
        let (p, a) = resolve_with(&settings(None, None), None, &exists).unwrap();
        assert_eq!(p, "konsole");
        assert_eq!(a, vec!["-e"]);
    }

    #[test]
    fn auto_prefers_x_terminal_emulator() {
        let exists = |b: &str| b == "x-terminal-emulator" || b == "gnome-terminal";
        let (p, a) = resolve_with(&settings(Some("auto"), None), None, &exists).unwrap();
        assert_eq!((p.as_str(), a), ("x-terminal-emulator", vec!["-e".to_string()]));
    }

    #[test]
    fn env_terminal_known_basename_uses_its_flag() {
        let exists = |b: &str| b == "wezterm";
        let (p, a) = resolve_with(&settings(None, None), Some("wezterm"), &exists).unwrap();
        assert_eq!(p, "wezterm");
        assert_eq!(a, vec!["start", "--"]);
    }

    #[test]
    fn env_terminal_unknown_defaults_to_dash_e() {
        let exists = |b: &str| b == "myterm";
        let (p, a) = resolve_with(&settings(None, None), Some("myterm"), &exists).unwrap();
        assert_eq!((p.as_str(), a), ("myterm", vec!["-e".to_string()]));
    }

    #[test]
    fn explicit_known_choice_wins() {
        let exists = |_: &str| true;
        let (p, a) = resolve_with(&settings(Some("gnome-terminal"), None), Some("xterm"), &exists).unwrap();
        assert_eq!((p.as_str(), a), ("gnome-terminal", vec!["--".to_string()]));
    }

    #[test]
    fn custom_command_is_split() {
        let exists = |_: &str| false;
        let (p, a) = resolve_with(&settings(Some("custom"), Some("  foot --app-id murmur ")), None, &exists).unwrap();
        assert_eq!(p, "foot");
        assert_eq!(a, vec!["--app-id", "murmur"]);
    }

    #[test]
    fn nothing_found_is_no_terminal() {
        let exists = |_: &str| false;
        assert_eq!(resolve_with(&settings(None, None), None, &exists).unwrap_err(), NO_TERMINAL_ERROR);
        assert_eq!(resolve_with(&settings(Some("custom"), Some("  ")), None, &exists).unwrap_err(), NO_TERMINAL_ERROR);
    }

    #[test]
    fn detect_filters_installed() {
        let exists = |b: &str| b == "xterm" || b == "foot";
        assert_eq!(detect_with(&exists), vec!["foot", "xterm"]);
    }
}
