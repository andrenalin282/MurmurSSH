//! Pure checks for the AI access: remote path normalisation + root jail, local
//! path jail, capability gate. No I/O except `canonicalize` for local paths.

use std::path::{Component, Path, PathBuf};

use crate::models::ai::AiCaps;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cap {
    Read,
    Download,
    Upload,
    Write,
    Delete,
    Exec,
}

impl Cap {
    pub fn name(self) -> &'static str {
        match self {
            Cap::Read => "read",
            Cap::Download => "download",
            Cap::Upload => "upload",
            Cap::Write => "write",
            Cap::Delete => "delete",
            Cap::Exec => "exec",
        }
    }
}

pub fn require_cap(caps: &AiCaps, cap: Cap) -> Result<(), String> {
    let on = match cap {
        Cap::Read => caps.read,
        Cap::Download => caps.download,
        Cap::Upload => caps.upload,
        Cap::Write => caps.write,
        Cap::Delete => caps.delete,
        Cap::Exec => caps.exec,
    };
    if on {
        Ok(())
    } else {
        Err(format!("Token has no '{}' permission", cap.name()))
    }
}

/// Control characters plus invisible Unicode format characters (bidi overrides,
/// zero-width, BOM, soft hyphen) that could disguise a path or command in the
/// confirmation dialog.
pub fn has_bad_chars(s: &str) -> bool {
    s.chars().any(|c| {
        c.is_control()
            || matches!(c, '\u{00AD}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
    })
}

/// Path components the AI may never touch on the local side, anywhere below the root
/// (git hooks/config would allow code execution, key directories leak secrets).
pub const LOCAL_DENY_COMPONENTS: &[&str] = &[".git", ".ssh", ".gnupg"];

pub fn has_denied_component(rel: &Path) -> bool {
    rel.components().any(|c| match c {
        Component::Normal(s) => LOCAL_DENY_COMPONENTS.iter().any(|d| s == std::ffi::OsStr::new(d)),
        _ => false,
    })
}

/// Lexically normalise an absolute remote path. Rejects relative paths, control
/// characters and any `..` that would climb above `/`.
pub fn normalize_remote(path: &str) -> Result<String, String> {
    if path.is_empty() || !path.starts_with('/') {
        return Err(format!("Remote path must be absolute: '{}'", path));
    }
    if has_bad_chars(path) {
        return Err("Remote path contains control characters".to_string());
    }
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(format!("Remote path escapes '/': '{}'", path));
                }
            }
            s => parts.push(s),
        }
    }
    Ok(format!("/{}", parts.join("/")))
}

/// True when `path` (normalised) is `root` or below it. `root` "/" allows all.
pub fn remote_within(root: &str, path: &str) -> bool {
    let root = root.trim_end_matches('/');
    root.is_empty() || path == root || path.starts_with(&format!("{}/", root))
}

pub fn remote_parent(path: &str) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => path[..i].to_string(),
    }
}

/// Directories under $HOME the AI may never read from or write to, even when the
/// token's local root contains them.
const LOCAL_DENY: &[&str] = &[".ssh", ".gnupg", ".config/murmurssh", ".local/share/keyrings", ".password-store"];

fn lexical_clean(p: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::RootDir => out.push("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err("Local path escapes '/'".to_string());
                }
            }
            Component::Normal(s) => out.push(s),
            Component::Prefix(_) => return Err("Unsupported path".to_string()),
        }
    }
    Ok(out)
}

/// Resolve `input` (absolute, or relative to `root`) inside the token's local root.
/// Existing paths are canonicalised (symlinks followed); for a not-yet-existing
/// target the parent must exist and is canonicalised. The result must lie in the
/// canonical root and outside the deny list.
pub fn resolve_local(root: &str, input: &str, home: Option<&Path>) -> Result<PathBuf, String> {
    if input.is_empty() || has_bad_chars(input) {
        return Err("Invalid local path".to_string());
    }
    let root_c = std::fs::canonicalize(root).map_err(|e| format!("Local root '{}': {}", root, e))?;
    let joined = if Path::new(input).is_absolute() {
        PathBuf::from(input)
    } else {
        root_c.join(input)
    };
    let clean = lexical_clean(&joined)?;
    let resolved = if std::fs::symlink_metadata(&clean).is_ok() {
        std::fs::canonicalize(&clean).map_err(|e| format!("'{}': {}", input, e))?
    } else {
        let parent = clean.parent().ok_or("Invalid local path")?;
        let name = clean.file_name().ok_or("Invalid local path")?;
        std::fs::canonicalize(parent)
            .map_err(|e| format!("Parent of '{}': {}", input, e))?
            .join(name)
    };
    if !resolved.starts_with(&root_c) {
        return Err(format!("Local path is outside the token's local root: '{}'", input));
    }
    if has_denied_component(resolved.strip_prefix(&root_c).unwrap_or(&resolved)) {
        return Err(format!("Local path touches a protected directory (.git/.ssh/.gnupg): '{}'", input));
    }
    if let Some(home) = home {
        for d in LOCAL_DENY {
            if resolved.starts_with(home.join(d)) {
                return Err(format!("Local path is in a protected directory: '{}'", input));
            }
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_remote_cases() {
        assert_eq!(normalize_remote("/a/b/../c/./d/").unwrap(), "/a/c/d");
        assert_eq!(normalize_remote("/").unwrap(), "/");
        assert_eq!(normalize_remote("//a//b").unwrap(), "/a/b");
        assert!(normalize_remote("a/b").is_err());
        assert!(normalize_remote("").is_err());
        assert!(normalize_remote("/..").is_err());
        assert!(normalize_remote("/a/../../etc").is_err());
        assert!(normalize_remote("/a\0b").is_err());
        assert!(normalize_remote("/a\nb").is_err());
        assert!(normalize_remote("/a\u{202E}b").is_err(), "bidi override");
        assert!(normalize_remote("/a\u{200B}b").is_err(), "zero width");
        assert!(normalize_remote("/café").is_ok());
    }

    #[test]
    fn denied_components() {
        assert!(has_denied_component(Path::new("proj/.git/config")));
        assert!(has_denied_component(Path::new(".ssh")));
        assert!(!has_denied_component(Path::new("proj/.gitignore")));
        assert!(!has_denied_component(Path::new("a/b.txt")));
    }

    #[test]
    fn remote_within_boundaries() {
        assert!(remote_within("/var/www", "/var/www"));
        assert!(remote_within("/var/www", "/var/www/x"));
        assert!(remote_within("/var/www/", "/var/www/x"));
        assert!(!remote_within("/var/www", "/var/www2"));
        assert!(!remote_within("/var/www", "/var"));
        assert!(remote_within("/", "/etc"));
    }

    #[test]
    fn remote_parent_cases() {
        assert_eq!(remote_parent("/a/b"), "/a");
        assert_eq!(remote_parent("/a"), "/");
    }

    #[test]
    fn caps_gate() {
        let mut c = AiCaps::default();
        assert!(require_cap(&c, Cap::Read).is_err());
        c.read = true;
        assert!(require_cap(&c, Cap::Read).is_ok());
        assert!(require_cap(&c, Cap::Delete).is_err());
        assert!(AiCaps::default().confirm_writes);
    }

    #[test]
    fn resolve_local_jail() {
        let base = std::env::temp_dir().join(format!("murmur-ai-guard-{}", std::process::id()));
        let root = base.join("root");
        let outside = base.join("outside");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(root.join("sub/f.txt"), b"x").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        let r = root.to_str().unwrap();

        assert!(resolve_local(r, "sub/f.txt", None).is_ok());
        assert!(resolve_local(r, "sub/new.txt", None).is_ok());
        assert!(resolve_local(r, root.join("sub").to_str().unwrap(), None).is_ok());
        assert!(resolve_local(r, "../outside", None).is_err());
        assert!(resolve_local(r, "link", None).is_err(), "symlink escape");
        assert!(resolve_local(r, "link/new.txt", None).is_err(), "symlink parent escape");
        assert!(resolve_local(r, "/etc/passwd", None).is_err());
        assert!(resolve_local(r, "nodir/new.txt", None).is_err(), "parent must exist");
        assert!(resolve_local(r, "", None).is_err());
        std::fs::create_dir_all(root.join("repo/.git")).unwrap();
        assert!(resolve_local(r, "repo/.git/config", None).is_err(), ".git denied");

        // deny list applies even inside the root
        std::fs::create_dir_all(root.join(".ssh")).unwrap();
        let canon = std::fs::canonicalize(&root).unwrap();
        assert!(resolve_local(r, ".ssh/id", Some(&canon)).is_err());

        std::fs::remove_dir_all(&base).unwrap();
    }
}
