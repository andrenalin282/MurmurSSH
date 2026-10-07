//! Small helpers for files that must never be world-readable or half-written,
//! plus the one place that decides what a profile id may look like.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// A profile id is used as a file name in several directories: allow only
/// `[A-Za-z0-9_-]`, 1..=128 chars. Rejects `.`, `/`, `..`, absolute paths, NUL.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn check_id(id: &str) -> Result<(), String> {
    if valid_id(id) {
        Ok(())
    } else {
        Err("Invalid profile id".to_string())
    }
}

/// Map an arbitrary string to a single safe path component (never traverses).
pub fn safe_component(s: &str) -> String {
    let c: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if c.is_empty() { "_".to_string() } else { c }
}

/// `create_dir_all` with 0700 for every directory it creates; also tightens `path` itself.
pub fn private_dir_all(path: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new().recursive(true).mode(0o700).create(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_sibling(path: &Path) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    path.with_file_name(format!(".{}.{}-{}-{}.tmp", name, std::process::id(), nanos, n))
}

/// Write `data` atomically with mode 0600: the content never exists with wider
/// permissions and a reader never sees a partial file.
pub fn write_private(path: &Path, data: &[u8]) -> Result<(), String> {
    let tmp = temp_sibling(path);
    let result = (|| -> std::io::Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

/// Create a new private (0600) file that must not exist yet; returns the open handle.
pub fn create_new_private(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        for ok in ["a", "my-profile_1", "A9"] {
            assert!(valid_id(ok), "{ok}");
        }
        for bad in ["", ".", "..", "../x", "a/b", "/etc", "a\0b", "a b", &"x".repeat(129)] {
            assert!(!valid_id(bad), "{bad:?}");
        }
        assert_eq!(safe_component("../a"), "___a");
    }

    #[test]
    fn write_private_is_0600_and_atomic() {
        let dir = std::env::temp_dir().join(format!("murmur_fs_{}", std::process::id()));
        private_dir_all(&dir).unwrap();
        let p = dir.join("s");
        write_private(&p, b"one").unwrap();
        write_private(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
