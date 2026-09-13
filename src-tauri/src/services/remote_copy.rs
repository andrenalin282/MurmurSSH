//! Remote "Copy to…": server-side `cp` when the SSH server allows exec, otherwise
//! download into a per-job temp dir and upload again.

use std::collections::HashMap;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::models::{Profile, Protocol, CANCELLED_ERROR};
use crate::services::sftp_service::ExecOptions;
use crate::services::{ftp_service, sftp_service};

const PROBE_TOKEN: &str = "murmur-exec-ok";
/// Hard deadline for the exec-capability probe. A server with exec disabled or
/// firewalled should fail fast, not hang the job.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// Cap on the cp error text folded into the job's error message.
const ERROR_TEXT_MAX_CHARS: usize = 2000;

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

/// Keep only the last `max_chars` characters, char-boundary safe (never splits a
/// multi-byte codepoint). Used to bound cp stdout/stderr folded into job errors.
fn truncate_tail(s: &str, max_chars: usize) -> String {
    let total = s.chars().count();
    if total <= max_chars {
        return s.to_string();
    }
    let skip = total - max_chars;
    s.chars().skip(skip).collect()
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

fn cache_verdict(profile_id: &str, ok: bool) {
    if let Ok(mut c) = exec_cache().lock() {
        c.insert(profile_id.to_string(), ok);
    }
}

/// Probe whether the server allows running commands over an exec channel.
/// The verdict is cached per profile id — but only when the probe actually ran
/// to a conclusion (matched, didn't match, or timed out). A failure to even
/// connect or open a channel is treated as transient and is never cached, so a
/// blip doesn't permanently fall back to download+upload for the session.
fn exec_available(profile: &Profile, cancel: &dyn Fn() -> bool) -> bool {
    if let Some(v) = exec_cache().lock().ok().and_then(|c| c.get(&profile.id).copied()) {
        return v;
    }
    let opts = ExecOptions { pty: false, timeout: Some(PROBE_TIMEOUT) };
    match sftp_service::exec_command(profile, &format!("printf {}", PROBE_TOKEN), opts, cancel) {
        Ok((0, ref out, _)) if out == PROBE_TOKEN => {
            cache_verdict(&profile.id, true);
            true
        }
        Ok(_) => {
            cache_verdict(&profile.id, false);
            false
        }
        Err(ref e) if e == sftp_service::EXEC_TIMEOUT_ERROR => {
            cache_verdict(&profile.id, false);
            false
        }
        Err(_) => false, // connect/channel-open failure or cancel: transient, don't cache
    }
}

fn tmp_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config").join("murmurssh").join("workspace").join(".copy-tmp")
}

/// Parse a `.copy-tmp` child directory name as `<pid>-<job_id>`. `None` when the
/// name doesn't match (garbage/foreign entry).
fn parse_copy_tmp_name(name: &str) -> Option<(u32, u64)> {
    let (pid_str, job_str) = name.split_once('-')?;
    let pid: u32 = pid_str.parse().ok()?;
    let job: u64 = job_str.parse().ok()?;
    Some((pid, job))
}

/// True when a startup sweep should remove this `.copy-tmp` entry: it either
/// doesn't match the `<pid>-<job_id>` naming scheme at all, or its pid is not
/// the current process and is not alive (a previous run's leftover). Entries
/// belonging to `own_pid` are never considered stale here — this process
/// manages its own directory via `wipe_own_tmp` at exit, not at startup.
fn is_stale_entry(name: &str, alive: &dyn Fn(u32) -> bool, own_pid: u32) -> bool {
    match parse_copy_tmp_name(name) {
        Some((pid, _job)) => pid != own_pid && !alive(pid),
        None => true,
    }
}

fn proc_is_alive(pid: u32) -> bool {
    std::path::Path::new("/proc").join(pid.to_string()).exists()
}

fn sweep_tmp_root(should_remove: impl Fn(&str) -> bool) {
    let Ok(entries) = std::fs::read_dir(tmp_root()) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if should_remove(&name) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Remove this process's own per-job temp dirs (called on exit). Parallel
/// windows are separate processes with independent job-id counters starting at
/// 1, so directories are namespaced by pid and each process only ever touches
/// its own.
pub fn wipe_own_tmp() {
    let own_pid = std::process::id();
    sweep_tmp_root(|name| matches!(parse_copy_tmp_name(name), Some((pid, _)) if pid == own_pid));
}

/// Remove leftovers from crashed/killed previous runs (called at startup).
/// Never touches entries belonging to a still-alive process (including, in
/// principle, this one, though it has created none yet at startup).
pub fn wipe_stale_tmp() {
    let own_pid = std::process::id();
    sweep_tmp_root(|name| is_stale_entry(name, &proc_is_alive, own_pid));
}

fn file_name(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or_default().to_string()
}

/// True when `dst` is the same path as `src`, or (`is_dir`) `dst` lies inside `src`
/// — both would make `cp`/download-then-upload copy a tree into itself.
fn is_copy_into_itself(src: &str, dst: &str, is_dir: bool) -> bool {
    if dst == src {
        return true;
    }
    if is_dir {
        let prefix = format!("{}/", src.trim_end_matches('/'));
        return dst.starts_with(&prefix);
    }
    false
}

pub fn run(
    profile: &Profile,
    job_id: u64,
    src: &str,
    dst: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let name = file_name(src);
    if name.is_empty() || name == "." || name == ".." {
        return Err(format!("Cannot copy '{}': could not determine a file name", src));
    }

    let is_ftp = profile.protocol.as_ref() == Some(&Protocol::Ftp);
    let is_dir = if is_ftp { ftp_service::is_remote_dir(profile, src)? } else { sftp_service::is_remote_dir(profile, src)? };

    if is_copy_into_itself(src, dst, is_dir) {
        return Err(format!("Cannot copy '{}' into itself", src));
    }

    on_progress(0, 0, &name);

    if !is_ftp && exec_available(profile, cancel) {
        let opts = ExecOptions { pty: true, timeout: None };
        let (status, out, err) = sftp_service::exec_command(profile, &cp_command(src, dst, is_dir), opts, cancel)?;
        if status != 0 {
            // With a PTY, remote stderr is merged into stdout, so `out` normally carries
            // the error text; fold in `err` too in case anything landed there anyway.
            let mut text = out.trim().to_string();
            let err_trimmed = err.trim();
            if !err_trimmed.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(err_trimmed);
            }
            return Err(format!("cp failed ({}): {}", status, truncate_tail(&text, ERROR_TEXT_MAX_CHARS)));
        }
        return Ok(());
    }

    // Fallback: download into a per-job temp dir, then upload.
    let tmp = tmp_root().join(format!("{}-{}", std::process::id(), job_id));
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&tmp)
        .map_err(|e| format!("Cannot create temp dir: {}", e))?;
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

    #[test]
    fn file_name_edge_cases_are_invalid() {
        assert_eq!(file_name("/"), "");
        assert_eq!(file_name("."), ".");
        assert_eq!(file_name(".."), "..");
        assert_eq!(file_name(""), "");
    }

    #[test]
    fn truncate_tail_keeps_short_strings_unchanged() {
        assert_eq!(truncate_tail("short", 2000), "short");
        assert_eq!(truncate_tail("", 2000), "");
    }

    #[test]
    fn truncate_tail_keeps_last_n_chars_on_char_boundary() {
        // Multibyte chars (emoji, 4 bytes each in UTF-8) must not be split.
        let s: String = "\u{1F600}".repeat(10);
        let truncated = truncate_tail(&s, 3);
        assert_eq!(truncated.chars().count(), 3);
        assert_eq!(truncated, "\u{1F600}\u{1F600}\u{1F600}");

        let ascii: String = (0..5000).map(|i| (b'a' + (i % 26) as u8) as char).collect();
        let truncated_ascii = truncate_tail(&ascii, 2000);
        assert_eq!(truncated_ascii.chars().count(), 2000);
        assert_eq!(truncated_ascii, &ascii[ascii.len() - 2000..]);
    }

    #[test]
    fn is_copy_into_itself_same_path() {
        assert!(is_copy_into_itself("/a/f.txt", "/a/f.txt", false));
        assert!(is_copy_into_itself("/a/dir", "/a/dir", true));
    }

    #[test]
    fn is_copy_into_itself_nested_dir() {
        assert!(is_copy_into_itself("/a/dir", "/a/dir/sub", true));
        assert!(is_copy_into_itself("/a/dir/", "/a/dir/sub", true));
    }

    #[test]
    fn is_copy_into_itself_false_positives_avoided() {
        // Sibling with a matching prefix but not actually nested.
        assert!(!is_copy_into_itself("/a/dir", "/a/dir2", true));
        // Not a directory copy: prefix rule doesn't apply.
        assert!(!is_copy_into_itself("/a/dir", "/a/dir/sub", false));
        // Unrelated paths.
        assert!(!is_copy_into_itself("/a/f.txt", "/b/f.txt", false));
    }

    #[test]
    fn parse_copy_tmp_name_valid_and_invalid() {
        assert_eq!(parse_copy_tmp_name("123-45"), Some((123, 45)));
        assert_eq!(parse_copy_tmp_name("not-a-pid"), None);
        assert_eq!(parse_copy_tmp_name("noseparator"), None);
        assert_eq!(parse_copy_tmp_name("123-"), None);
        assert_eq!(parse_copy_tmp_name("-45"), None);
    }

    #[test]
    fn is_stale_entry_own_pid_is_never_stale() {
        assert!(!is_stale_entry("777-1", &|_| false, 777));
    }

    #[test]
    fn is_stale_entry_dead_foreign_pid_is_stale() {
        assert!(is_stale_entry("111-1", &|_| false, 777));
    }

    #[test]
    fn is_stale_entry_alive_foreign_pid_is_kept() {
        assert!(!is_stale_entry("111-1", &|pid| pid == 111, 777));
    }

    #[test]
    fn is_stale_entry_malformed_name_is_stale() {
        assert!(is_stale_entry("garbage", &|_| true, 777));
    }
}
