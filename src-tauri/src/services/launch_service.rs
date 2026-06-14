use crate::models::profile::Profile;
use crate::services::profile_service;
use std::sync::OnceLock;

/// Raw `--profile <value>` captured once from process args at startup.
static LAUNCH_ARG: OnceLock<Option<String>> = OnceLock::new();

/// Capture the launch arg from the current process args. Call once at startup.
pub fn capture_launch_arg() {
    let _ = LAUNCH_ARG.set(parse_profile_arg(std::env::args().skip(1).collect()));
}

/// Pure: extract the value of `--profile <v>` or `--profile=<v>` from args.
/// Returns the first non-empty value found, else None.
pub fn parse_profile_arg(args: Vec<String>) -> Option<String> {
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if let Some(v) = a.strip_prefix("--profile=") {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        } else if a == "--profile" {
            if let Some(v) = it.next() {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// Pure: resolve a raw id-or-name token to a canonical profile id.
/// Exact `id` match wins; otherwise case-insensitive `name` match; else None.
pub fn resolve_profile_id(raw: &str, profiles: &[Profile]) -> Option<String> {
    if let Some(p) = profiles.iter().find(|p| p.id == raw) {
        return Some(p.id.clone());
    }
    let lower = raw.to_lowercase();
    profiles
        .iter()
        .find(|p| p.name.to_lowercase() == lower)
        .map(|p| p.id.clone())
}

/// Pure: build the `.desktop` file content for a profile.
/// The exec path is double-quoted so paths with spaces (e.g. AppImage) stay valid.
pub fn desktop_entry(exec_path: &str, profile: &Profile) -> String {
    // Strip CR/LF from free-text profile fields so a newline can't inject extra
    // Desktop Entry keys into the generated file. The Exec line is already safe
    // (it uses only the slug `id`, which is validated by the caller).
    let strip = |s: &str| s.replace(['\n', '\r'], " ");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=MurmurSSH — {name}\n\
         Comment=Connect to {user}@{host}\n\
         Exec=\"{exec}\" --profile {id}\n\
         Icon=murmurssh\n\
         Terminal=false\n\
         Categories=Network;RemoteAccess;\n",
        name = strip(&profile.name),
        user = strip(&profile.username),
        host = strip(&profile.host),
        exec = exec_path,
        id = profile.id,
    )
}

/// Resolve the captured launch arg against current profiles → canonical id.
pub fn launch_profile_id() -> Option<String> {
    let raw = LAUNCH_ARG.get().and_then(|o| o.clone())?;
    let profiles = profile_service::list_profiles().ok()?;
    resolve_profile_id(&raw, &profiles)
}

/// Spawn a new, detached MurmurSSH instance pointed at `profile_id`.
pub fn open_in_new_window(profile_id: &str) -> Result<(), String> {
    if profile_id.contains('\0') {
        return Err("Invalid profile id".to_string());
    }
    // Validate the profile exists before launching.
    profile_service::get_profile(profile_id)?;
    let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    std::process::Command::new(exe)
        .arg("--profile")
        .arg(profile_id)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch new window: {e}"))
}

/// Locale-aware desktop directory for the save dialog's default location.
/// Prefers the XDG desktop dir (e.g. `~/Schreibtisch`), then `~/Desktop`, then `$HOME`.
pub fn desktop_dir() -> String {
    if let Ok(out) = std::process::Command::new("xdg-user-dir").arg("DESKTOP").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() && std::path::Path::new(&p).is_dir() {
                return p;
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let desktop = std::path::Path::new(&home).join("Desktop");
    if desktop.is_dir() {
        return desktop.to_string_lossy().to_string();
    }
    home
}

/// Stable executable path to bake into a persistent `.desktop` launcher.
/// For an AppImage `current_exe()` is an ephemeral mount under /tmp that vanishes
/// on exit, so prefer `$APPIMAGE` (the real .AppImage file) when present.
fn exec_target() -> Result<String, String> {
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        if !appimage.is_empty() {
            return Ok(appimage);
        }
    }
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("current_exe failed: {e}"))
}

/// Write a `.desktop` launcher for `profile_id` to a user-chosen `target_path`,
/// then make it an executable, trusted launcher so desktop environments (GNOME)
/// run it on double-click instead of showing a dead text file. Returns the path.
pub fn create_desktop_shortcut(profile_id: &str, target_path: &str) -> Result<String, String> {
    if profile_id.contains('\0') || profile_id.contains('/') {
        return Err("Invalid profile id".to_string());
    }
    if target_path.is_empty() || target_path.contains('\0') {
        return Err("Invalid target path".to_string());
    }
    let profile = profile_service::get_profile(profile_id)?;
    let exe = exec_target()?;
    let path = std::path::PathBuf::from(target_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {e}"))?;
    }
    let content = desktop_entry(&exe, &profile);
    std::fs::write(&path, content).map_err(|e| format!("Failed to write shortcut: {e}"))?;

    // GNOME (and others) only run a .desktop launcher that is BOTH executable and
    // marked trusted — otherwise it shows as a plain text file. Both are
    // best-effort: the file is already written, so we don't fail the whole op.
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    let _ = std::process::Command::new("gio")
        .args(["set", &path.to_string_lossy(), "metadata::trusted", "true"])
        .status();

    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::profile::{AuthType, Profile, UploadMode};

    fn mk(id: &str, name: &str) -> Profile {
        Profile {
            id: id.to_string(),
            name: name.to_string(),
            host: "example.com".to_string(),
            port: 22,
            username: "root".to_string(),
            auth_type: AuthType::Agent,
            key_path: None,
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
        }
    }

    #[test]
    fn parse_space_form() {
        let a = vec!["--profile".to_string(), "web1".to_string()];
        assert_eq!(parse_profile_arg(a), Some("web1".to_string()));
    }

    #[test]
    fn parse_equals_form() {
        let a = vec!["--profile=web1".to_string()];
        assert_eq!(parse_profile_arg(a), Some("web1".to_string()));
    }

    #[test]
    fn parse_absent() {
        let a = vec!["--other".to_string(), "x".to_string()];
        assert_eq!(parse_profile_arg(a), None);
    }

    #[test]
    fn parse_empty_value_ignored() {
        let a = vec!["--profile".to_string(), "".to_string()];
        assert_eq!(parse_profile_arg(a), None);
    }

    #[test]
    fn resolve_by_id_exact() {
        let ps = vec![mk("web-1", "Web One"), mk("db-1", "DB One")];
        assert_eq!(resolve_profile_id("db-1", &ps), Some("db-1".to_string()));
    }

    #[test]
    fn resolve_by_name_case_insensitive() {
        let ps = vec![mk("web-1", "Web One")];
        assert_eq!(resolve_profile_id("web one", &ps), Some("web-1".to_string()));
    }

    #[test]
    fn resolve_unknown_is_none() {
        let ps = vec![mk("web-1", "Web One")];
        assert_eq!(resolve_profile_id("nope", &ps), None);
    }

    #[test]
    fn desktop_entry_quotes_exec_and_uses_id() {
        let p = mk("web-1", "Web One");
        let s = desktop_entry("/opt/My App/murmurssh", &p);
        assert!(s.contains("Exec=\"/opt/My App/murmurssh\" --profile web-1"));
        assert!(s.contains("Name=MurmurSSH — Web One"));
        assert!(s.contains("Comment=Connect to root@example.com"));
    }

    #[test]
    fn desktop_entry_strips_newlines_from_fields() {
        let mut p = mk("web-1", "Web\nOne");
        p.host = "ex\r\nample.com".to_string();
        let s = desktop_entry("/opt/murmurssh", &p);
        // No interpolated field may introduce a bare newline that starts a new key.
        assert!(s.contains("Name=MurmurSSH — Web One"));
        assert!(!s.contains("Name=MurmurSSH — Web\nOne"));
        assert!(s.contains("ex  ample.com"));
    }
}


