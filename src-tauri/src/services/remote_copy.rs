//! Remote "Copy to…": server-side `cp` when the SSH server allows exec, otherwise
//! download into a per-job temp dir and upload again.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::models::{Profile, Protocol, CANCELLED_ERROR};
use crate::services::{ftp_service, sftp_service};

const PROBE_TOKEN: &str = "murmur-exec-ok";

/// POSIX single-quote escaping: `it's` → `'it'\''s'`.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Directory copies merge into `dst` (mkdir -p + `src/.`), matching folder-upload semantics
/// and avoiding `cp` nesting `src` inside an existing `dst`.
pub fn cp_command(src: &str, dst: &str, is_dir: bool) -> String {
    if is_dir {
        let src_dot = format!("{}/.", src.trim_end_matches('/'));
        format!("mkdir -p -- {} && cp -a -- {} {}", shell_quote(dst), shell_quote(&src_dot), shell_quote(dst))
    } else {
        format!("cp -a -- {} {}", shell_quote(src), shell_quote(dst))
    }
}

fn exec_cache() -> &'static Mutex<HashMap<String, bool>> {
    static C: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Drop the cached exec capability (called on disconnect).
pub fn forget_profile(profile_id: &str) {
    if let Ok(mut c) = exec_cache().lock() {
        c.remove(profile_id);
    }
}

fn exec_available(profile: &Profile) -> bool {
    if let Some(v) = exec_cache().lock().ok().and_then(|c| c.get(&profile.id).copied()) {
        return v;
    }
    let ok = matches!(
        sftp_service::exec_command(profile, &format!("printf {}", PROBE_TOKEN), &|| false),
        Ok((0, ref out, _)) if out == PROBE_TOKEN
    );
    if let Ok(mut c) = exec_cache().lock() {
        c.insert(profile.id.clone(), ok);
    }
    ok
}

fn tmp_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config").join("murmurssh").join("workspace").join(".copy-tmp")
}

/// Remove leftovers from crashed runs (startup) and on exit.
pub fn wipe_tmp_root() {
    let _ = std::fs::remove_dir_all(tmp_root());
}

fn file_name(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("copy").to_string()
}

pub fn run(
    profile: &Profile,
    job_id: u64,
    src: &str,
    dst: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let is_ftp = profile.protocol.as_ref() == Some(&Protocol::Ftp);
    let is_dir = if is_ftp { ftp_service::is_remote_dir(profile, src)? } else { sftp_service::is_remote_dir(profile, src)? };
    let name = file_name(src);
    on_progress(0, 0, &name);

    if !is_ftp && exec_available(profile) {
        let (status, _out, err) = sftp_service::exec_command(profile, &cp_command(src, dst, is_dir), cancel)?;
        if status != 0 {
            return Err(format!("cp failed ({}): {}", status, err.trim()));
        }
        return Ok(());
    }

    // Fallback: download into a per-job temp dir, then upload.
    let tmp = tmp_root().join(job_id.to_string());
    std::fs::create_dir_all(&tmp).map_err(|e| format!("Cannot create temp dir: {}", e))?;
    let local = tmp.join(&name);
    let local_str = local.to_string_lossy().to_string();
    let result = (|| -> Result<(), String> {
        match (is_ftp, is_dir) {
            (true, true) => ftp_service::download_directory(profile, src, &local_str, cancel, on_progress)?,
            (true, false) => ftp_service::download_file_to(profile, src, &local_str, cancel, on_progress)?,
            (false, true) => sftp_service::download_directory(profile, src, &local_str, cancel, on_progress)?,
            (false, false) => sftp_service::download_file(profile, src, &local_str, cancel, &|d, t| on_progress(d, t, &name))?,
        }
        if cancel() {
            return Err(CANCELLED_ERROR.to_string());
        }
        match (is_ftp, is_dir) {
            (true, true) => ftp_service::upload_directory(profile, &local_str, dst, cancel, on_progress),
            (true, false) => ftp_service::upload_file(profile, &local_str, dst, cancel, on_progress),
            (false, true) => sftp_service::upload_directory(profile, &local_str, dst, cancel, on_progress),
            (false, false) => sftp_service::upload_file(profile, &local_str, dst, cancel, &|d, t| on_progress(d, t, &name)),
        }
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
    }

    #[test]
    fn cp_command_file_and_dir() {
        assert_eq!(cp_command("/a/f.txt", "/b/f.txt", false), "cp -a -- '/a/f.txt' '/b/f.txt'");
        assert_eq!(
            cp_command("/a/dir/", "/b/dir", true),
            "mkdir -p -- '/b/dir' && cp -a -- '/a/dir/.' '/b/dir'"
        );
    }

    #[test]
    fn file_name_takes_last_segment() {
        assert_eq!(file_name("/a/b/c.txt"), "c.txt");
        assert_eq!(file_name("/a/b/dir/"), "dir");
    }
}
