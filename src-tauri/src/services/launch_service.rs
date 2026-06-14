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
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=MurmurSSH — {name}\n\
         Comment=Connect to {user}@{host}\n\
         Exec=\"{exec}\" --profile {id}\n\
         Icon=murmurssh\n\
         Terminal=false\n\
         Categories=Network;RemoteAccess;\n",
        name = profile.name,
        user = profile.username,
        host = profile.host,
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

/// Write a `.desktop` launcher for `profile_id` into the user's applications dir.
/// Returns the written path.
pub fn create_desktop_shortcut(profile_id: &str) -> Result<String, String> {
    if profile_id.contains('\0') || profile_id.contains('/') {
        return Err("Invalid profile id".to_string());
    }
    let profile = profile_service::get_profile(profile_id)?;
    let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let dir = std::path::Path::new(&home).join(".local/share/applications");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {e}"))?;
    let path = dir.join(format!("murmurssh-{profile_id}.desktop"));
    let content = desktop_entry(&exe.to_string_lossy(), &profile);
    std::fs::write(&path, content).map_err(|e| format!("Failed to write shortcut: {e}"))?;
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
}
