use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::Emitter;

use crate::models::{Profile, UploadMode};
use crate::services::sftp_service;

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
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    PathBuf::from(home)
        .join(".config")
        .join("murmurssh")
        .join("workspace")
}

/// Returns the local cache path for a given profile + remote file.
/// Example: ~/.config/murmurssh/workspace/my-server/config.yaml
fn local_cache_path(profile_id: &str, remote_path: &str) -> PathBuf {
    let filename = Path::new(remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    workspace_base().join(profile_id).join(filename)
}

/// Opens a remote text file for editing.
///
/// Flow:
/// 1. Download to local workspace cache
/// 2. Validate (size + binary check)
/// 3. Open in configured editor or via xdg-open
/// 4. Spawn a background thread that watches for saves and triggers upload
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

    // Download the file
    sftp_service::download_file(profile, remote_path, local_path.to_str().unwrap_or_default())
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

    // Open in editor
    open_in_editor(profile, &local_path)?;

    // Spawn background watcher thread
    let profile_clone = profile.clone();
    let remote_path_owned = remote_path.to_string();
    let local_path_clone = local_path.clone();

    std::thread::spawn(move || {
        watch_and_upload(app, profile_clone, local_path_clone, remote_path_owned);
    });

    Ok(())
}

/// Launches the editor for the given local file.
///
/// Uses profile.editor_command if set, otherwise falls back to xdg-open
/// which opens the file with the system default application.
fn open_in_editor(profile: &Profile, local_path: &Path) -> Result<(), String> {
    let path_str = local_path.to_string_lossy();

    if let Some(editor) = &profile.editor_command {
        // editor_command may include flags, e.g. "code --wait" or "gedit"
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

/// Watches the local cached file and triggers upload on save.
///
/// Runs on a background thread for the lifetime of the file.
/// Stops when the file is deleted or the channel closes (app exits).
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
            return;
        }
    };

    if let Err(e) = watcher.watch(&local_path, RecursiveMode::NonRecursive) {
        eprintln!(
            "[murmurssh] Failed to watch {}: {}",
            local_path.display(),
            e
        );
        return;
    }

    // Record initial mtime so we only act on actual changes
    let mut last_mtime = std::fs::metadata(&local_path)
        .and_then(|m| m.modified())
        .ok();

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
                // File deleted; stop watching
                break;
            }
            EventKind::Modify(_) | EventKind::Create(_) => {
                // Brief pause to let the editor finish writing
                std::thread::sleep(DEBOUNCE_DELAY);

                // Confirm the mtime changed (deduplicates rapid events)
                let current_mtime = std::fs::metadata(&local_path)
                    .and_then(|m| m.modified())
                    .ok();

                if current_mtime == last_mtime {
                    continue;
                }
                last_mtime = current_mtime;

                match profile.upload_mode {
                    UploadMode::Auto => {
                        match sftp_service::upload_file(
                            &profile,
                            local_path.to_str().unwrap_or_default(),
                            &remote_path,
                        ) {
                            Ok(()) => {
                                let _ = app.emit("upload-complete", &remote_path);
                            }
                            Err(e) => {
                                eprintln!("[murmurssh] Auto-upload failed: {}", e);
                                let _ = app.emit("upload-error", e);
                            }
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
}
