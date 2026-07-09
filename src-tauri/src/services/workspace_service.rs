use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::Emitter;

use crate::models::{Profile, Protocol, UploadMode};
use crate::services::{ftp_service, sftp_service};

fn is_ftp(profile: &Profile) -> bool {
    profile.protocol.as_ref() == Some(&Protocol::Ftp)
}

/// Maximum file size allowed for the edit flow.
const MAX_EDIT_BYTES: u64 = 1024 * 1024; // 1 MB

/// How long to wait after a file event before acting, to let editors finish writing.
const DEBOUNCE_DELAY: Duration = Duration::from_millis(300);

/// Payload emitted as a Tauri event when a watched file changes in `confirm` upload mode.
#[derive(Debug, Clone, Serialize)]
pub struct UploadReadyPayload {
    pub profile_id: String,
    pub local_path: String,
    pub remote_path: String,
}

fn workspace_base() -> PathBuf {
    // Fall back to "/" rather than panicking — with panic=abort in release,
    // an unset $HOME would otherwise terminate the entire app.
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("murmurssh")
        .join("workspace")
}

/// Returns the local cache path for a given profile + remote file.
///
/// The remote directory structure is mirrored under the profile's workspace dir
/// (e.g. `/var/www/config.php` -> `<workspace>/<profile>/var/www/config.php`) so
/// that identically named files from different remote directories do not collide
/// in the local edit cache. Path components are sanitized (empty, `.` and `..`
/// dropped) to keep everything inside the workspace directory.
fn local_cache_path(profile_id: &str, remote_path: &str) -> PathBuf {
    let mut path = workspace_base().join(profile_id);
    let components: Vec<&str> = remote_path
        .split('/')
        .filter(|c| !c.is_empty() && *c != "." && *c != "..")
        .collect();
    if components.is_empty() {
        return path.join("file");
    }
    for comp in components {
        path.push(comp);
    }
    path
}

/// Computes a fast content hash of a file. Returns None if the file can't be read.
/// Used to detect *real* edits and ignore re-downloads / duplicate save events.
fn file_content_hash(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(format!("{:016x}", hasher.finish()))
}

/// Registry of paths currently being watched. Prevents duplicate watchers.
fn active_watchers() -> &'static Mutex<HashSet<PathBuf>> {
    static WATCHERS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    WATCHERS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn register_watcher(path: &Path) -> bool {
    if let Ok(mut set) = active_watchers().lock() {
        set.insert(path.to_path_buf())
    } else {
        true // proceed on lock failure
    }
}

fn unregister_watcher(path: &Path) {
    if let Ok(mut set) = active_watchers().lock() {
        set.remove(path);
    }
}

fn is_watched(path: &Path) -> bool {
    active_watchers()
        .lock()
        .map(|set| set.contains(path))
        .unwrap_or(false)
}

/// Per-path content-hash baseline. The watcher only acts when a file's current
/// hash differs from its baseline; downloads/uploads update the baseline so they
/// don't look like user edits.
fn baselines() -> &'static Mutex<std::collections::HashMap<PathBuf, String>> {
    static BASELINES: OnceLock<Mutex<std::collections::HashMap<PathBuf, String>>> = OnceLock::new();
    BASELINES.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn set_baseline(path: &Path, hash: String) {
    if let Ok(mut map) = baselines().lock() {
        map.insert(path.to_path_buf(), hash);
    }
}

fn get_baseline(path: &Path) -> Option<String> {
    baselines().lock().ok().and_then(|map| map.get(path).cloned())
}

fn clear_baseline(path: &Path) {
    if let Ok(mut map) = baselines().lock() {
        map.remove(path);
    }
}

/// Auto-upload coalescing state per watched path. While an upload is in flight,
/// further saves only set `dirty`, so exactly one catch-up upload runs after the
/// current one finishes — turning a burst of N saves into a single upload plus at
/// most one follow-up, instead of N parallel uploads.
struct AutoUploadState {
    in_flight: bool,
    dirty: bool,
}

fn auto_uploads() -> &'static Mutex<std::collections::HashMap<PathBuf, AutoUploadState>> {
    static STATES: OnceLock<Mutex<std::collections::HashMap<PathBuf, AutoUploadState>>> =
        OnceLock::new();
    STATES.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Returns true if the caller should spawn a new uploader thread. Returns false
/// when an upload for this path is already running (the path is marked dirty so a
/// single catch-up upload runs when the current one completes).
fn begin_auto_upload(path: &Path) -> bool {
    if let Ok(mut map) = auto_uploads().lock() {
        let st = map.entry(path.to_path_buf()).or_insert(AutoUploadState {
            in_flight: false,
            dirty: false,
        });
        if st.in_flight {
            st.dirty = true;
            false
        } else {
            st.in_flight = true;
            st.dirty = false;
            true
        }
    } else {
        true
    }
}

/// Called by the uploader thread after each upload. Returns true if another
/// upload should run (the path was marked dirty meanwhile), keeping `in_flight`
/// set; otherwise clears `in_flight` and returns false.
fn finish_auto_upload(path: &Path) -> bool {
    if let Ok(mut map) = auto_uploads().lock() {
        if let Some(st) = map.get_mut(path) {
            if st.dirty {
                st.dirty = false;
                return true;
            }
            st.in_flight = false;
        }
    }
    false
}

fn clear_auto_upload(path: &Path) {
    if let Ok(mut map) = auto_uploads().lock() {
        map.remove(path);
    }
}

/// Upload the current file contents, retrying once per pending "dirty" mark so a
/// burst of saves collapses into a single trailing upload.
fn spawn_auto_upload(
    app: tauri::AppHandle,
    profile: Profile,
    local_path: PathBuf,
    remote_path: String,
) {
    std::thread::spawn(move || loop {
        let local_str = local_path.to_str().unwrap_or_default();
        let upload_result = if is_ftp(&profile) {
            ftp_service::upload_file(&profile, local_str, &remote_path, &|| false, &|_, _, _| {})
        } else {
            sftp_service::upload_file(&profile, local_str, &remote_path, &|| false, &|_, _| {})
        };
        match upload_result {
            Ok(()) => {
                let _ = app.emit("upload-complete", &remote_path);
            }
            Err(e) => {
                eprintln!("[murmurssh] Auto-upload failed: {}", e);
                let _ = app.emit("upload-error", e);
            }
        }
        if !finish_auto_upload(&local_path) {
            break;
        }
    });
}

/// Opens a remote text file for editing.
pub fn open_for_edit(
    app: tauri::AppHandle,
    profile: &Profile,
    remote_path: &str,
) -> Result<(), String> {
    let local_path = local_cache_path(&profile.id, remote_path);

    // Ensure the workspace directory exists
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create workspace directory: {}", e))?;
    }

    // Download the file (dispatch based on protocol)
    let local_str = local_path.to_str().unwrap_or_default();
    if is_ftp(profile) {
        ftp_service::download_file_to(profile, remote_path, local_str, &|| false, &|_, _, _| {})
    } else {
        sftp_service::download_file(profile, remote_path, local_str, &|| false, &|_, _| {})
    }
    .map_err(|e| format!("Failed to download file for editing: {}", e))?;

    // Reject oversized files
    let file_size = std::fs::metadata(&local_path).map(|m| m.len()).unwrap_or(0);
    if file_size > MAX_EDIT_BYTES {
        return Err(format!(
            "File is too large for editing ({:.1} MB, max 1 MB). Use Download instead.",
            file_size as f64 / (1024.0 * 1024.0)
        ));
    }

    // Reject binary files (null bytes in the first 512 bytes)
    {
        let mut f = std::fs::File::open(&local_path)
            .map_err(|e| format!("Failed to read downloaded file: {}", e))?;
        let mut buf = [0u8; 512];
        let n = f.read(&mut buf).unwrap_or(0);
        if buf[..n].contains(&0u8) {
            return Err(
                "Binary files cannot be opened for editing. Use Download instead.".to_string(),
            );
        }
    }

    // Record the just-downloaded content as the baseline so the watcher does not
    // mistake this (re-)download for a user edit.
    if let Some(h) = file_content_hash(&local_path) {
        set_baseline(&local_path, h);
    }

    // Open in editor
    open_in_editor(profile, &local_path)?;

    // If already being watched, just re-open the editor without spawning a second watcher
    if is_watched(&local_path) {
        return Ok(());
    }

    // Register this path before spawning so concurrent calls don't both pass the check
    if !register_watcher(&local_path) {
        // Another thread just registered it — skip spawning
        return Ok(());
    }

    let profile_clone = profile.clone();
    let remote_path_owned = remote_path.to_string();
    let local_path_clone = local_path.clone();

    std::thread::spawn(move || {
        watch_and_upload(app, profile_clone, local_path_clone, remote_path_owned);
    });

    Ok(())
}

fn open_in_editor(profile: &Profile, local_path: &Path) -> Result<(), String> {
    let path_str = local_path.to_string_lossy();

    if let Some(editor) = &profile.editor_command {
        let mut parts = editor.split_whitespace();
        let cmd = parts.next().ok_or("editor_command is empty")?;
        let extra_args: Vec<&str> = parts.collect();

        std::process::Command::new(cmd)
            .args(&extra_args)
            .arg(path_str.as_ref())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch editor '{}': {}", editor, e))
    } else {
        std::process::Command::new("xdg-open")
            .arg(path_str.as_ref())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open file with xdg-open: {}. Is xdg-utils installed?", e))
    }
}

fn watch_and_upload(
    app: tauri::AppHandle,
    profile: Profile,
    local_path: PathBuf,
    remote_path: String,
) {
    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();

    let mut watcher = match RecommendedWatcher::new(tx, notify::Config::default()) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[murmurssh] Failed to create file watcher: {}", e);
            unregister_watcher(&local_path);
            clear_baseline(&local_path);
            return;
        }
    };

    if let Err(e) = watcher.watch(&local_path, RecursiveMode::NonRecursive) {
        eprintln!(
            "[murmurssh] Failed to watch {}: {}",
            local_path.display(),
            e
        );
        unregister_watcher(&local_path);
        clear_baseline(&local_path);
        return;
    }

    // Seed the baseline from the file as it exists now (open_for_edit normally
    // sets it already, but this guards the watch-before-download race).
    if get_baseline(&local_path).is_none() {
        if let Some(h) = file_content_hash(&local_path) {
            set_baseline(&local_path, h);
        }
    }

    for result in rx {
        let event = match result {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[murmurssh] File watcher error: {}", e);
                break;
            }
        };

        match event.kind {
            EventKind::Remove(_) => {
                break;
            }
            EventKind::Modify(_) | EventKind::Create(_) => {
                std::thread::sleep(DEBOUNCE_DELAY);

                let current_hash = match file_content_hash(&local_path) {
                    Some(h) => h,
                    None => continue, // file vanished mid-write; ignore
                };

                // Only a genuine content change (differs from the recorded
                // baseline) counts. This ignores re-downloads and collapses
                // multi-write editor saves into a single upload.
                if get_baseline(&local_path).as_deref() == Some(current_hash.as_str()) {
                    continue;
                }
                // Update the baseline immediately so the duplicate event from a
                // temp+rename save does not fire a second upload/prompt.
                set_baseline(&local_path, current_hash);

                match profile.upload_mode {
                    UploadMode::Auto => {
                        // Coalesce bursts: only spawn an uploader when one is not
                        // already running for this path; otherwise the path is
                        // marked dirty and a single catch-up upload follows.
                        if begin_auto_upload(&local_path) {
                            spawn_auto_upload(
                                app.clone(),
                                profile.clone(),
                                local_path.clone(),
                                remote_path.clone(),
                            );
                        }
                    }
                    UploadMode::Confirm => {
                        let payload = UploadReadyPayload {
                            profile_id: profile.id.clone(),
                            local_path: local_path.to_string_lossy().to_string(),
                            remote_path: remote_path.clone(),
                        };
                        let _ = app.emit("upload-ready", payload);
                    }
                }
            }
            _ => {}
        }
    }

    // Cleanup: remove from active watchers registry and drop the baseline
    unregister_watcher(&local_path);
    clear_baseline(&local_path);
    clear_auto_upload(&local_path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_file(name: &str, contents: &[u8]) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("murmurssh_test_{}", name));
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(contents).unwrap();
        p
    }

    #[test]
    fn hash_is_stable_for_same_content() {
        let p = tmp_file("stable", b"hello world");
        let a = file_content_hash(&p).unwrap();
        let b = file_content_hash(&p).unwrap();
        assert_eq!(a, b);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn hash_differs_for_different_content() {
        let p1 = tmp_file("diff_a", b"content A");
        let p2 = tmp_file("diff_b", b"content B");
        assert_ne!(
            file_content_hash(&p1).unwrap(),
            file_content_hash(&p2).unwrap()
        );
        let _ = std::fs::remove_file(&p1);
        let _ = std::fs::remove_file(&p2);
    }

    #[test]
    fn hash_none_for_missing_file() {
        let p = PathBuf::from("/nonexistent/murmurssh/definitely/missing");
        assert!(file_content_hash(&p).is_none());
    }

    #[test]
    fn baseline_set_get_clear_roundtrip() {
        let p = PathBuf::from("/tmp/murmurssh_baseline_roundtrip");
        clear_baseline(&p);
        assert_eq!(get_baseline(&p), None);
        set_baseline(&p, "deadbeef".to_string());
        assert_eq!(get_baseline(&p), Some("deadbeef".to_string()));
        set_baseline(&p, "feedface".to_string());
        assert_eq!(get_baseline(&p), Some("feedface".to_string()));
        clear_baseline(&p);
        assert_eq!(get_baseline(&p), None);
    }
}
