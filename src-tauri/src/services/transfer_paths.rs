//! Pure helpers shared by SFTP and FTP upload paths.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Temporary upload name next to the final target: `/a/b/file.txt` → `/a/b/.file.txt.murmur-part`.
/// The original file stays untouched until the upload completed and is renamed over it.
pub fn part_path(remote_path: &str) -> String {
    let trimmed = remote_path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(idx) => format!("{}/.{}.murmur-part", &trimmed[..idx], &trimmed[idx + 1..]),
        None => format!(".{}.murmur-part", trimmed),
    }
}

/// Tracks canonical directories on the current recursion stack so a symlink that points
/// back to an ancestor does not recurse forever.
pub struct LoopGuard {
    stack: HashSet<PathBuf>,
}

impl LoopGuard {
    pub fn new() -> Self {
        Self { stack: HashSet::new() }
    }

    /// Returns false when `dir` is already being walked (cycle) or cannot be resolved.
    pub fn enter(&mut self, dir: &Path) -> bool {
        match dir.canonicalize() {
            Ok(c) => self.stack.insert(c),
            Err(_) => false,
        }
    }

    pub fn leave(&mut self, dir: &Path) {
        if let Ok(c) = dir.canonicalize() {
            self.stack.remove(&c);
        }
    }
}

impl Default for LoopGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// `Ok(())` when nothing failed, otherwise one error line listing up to 3 failures.
pub fn summarize_failures(failures: &[String], total: usize) -> Result<(), String> {
    if failures.is_empty() {
        return Ok(());
    }
    let shown: Vec<&str> = failures.iter().take(3).map(|s| s.as_str()).collect();
    let more = if failures.len() > 3 { "; …" } else { "" };
    Err(format!(
        "{} of {} entries failed: {}{}",
        failures.len(),
        total,
        shown.join("; "),
        more
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_path_places_hidden_part_next_to_target() {
        assert_eq!(part_path("/a/b/file.txt"), "/a/b/.file.txt.murmur-part");
        assert_eq!(part_path("/file"), "/.file.murmur-part");
        assert_eq!(part_path("file"), ".file.murmur-part");
    }

    #[test]
    fn loop_guard_detects_symlink_cycle() {
        let root = std::env::temp_dir().join(format!("murmur-loopguard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::os::unix::fs::symlink(&root, root.join("sub").join("back")).unwrap();

        let mut g = LoopGuard::new();
        assert!(g.enter(&root));
        assert!(g.enter(&root.join("sub")));
        assert!(!g.enter(&root.join("sub").join("back")), "cycle must be rejected");
        g.leave(&root.join("sub"));
        assert!(g.enter(&root.join("sub")), "re-enter after leave is allowed");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn summarize_failures_formats() {
        assert!(summarize_failures(&[], 5).is_ok());
        let f: Vec<String> = (1..=4).map(|i| format!("f{}: denied", i)).collect();
        let err = summarize_failures(&f, 10).unwrap_err();
        assert_eq!(err, "4 of 10 entries failed: f1: denied; f2: denied; f3: denied; …");
    }
}
