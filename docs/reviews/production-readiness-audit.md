# MurmurSSH — Production Readiness Audit

Scope: Rust backend (`src-tauri/src`) and TypeScript frontend (`src/`).
Date: 2026-04-21.
Baseline: branch `main` @ `ed4d14b` (v1.4.2).

Severity scale:

- **Critical** — data loss, code execution, or authentication bypass path.
- **High** — correctness or safety breakage likely to hit a real user.
- **Medium** — degraded UX or edge-case failure with practical impact.
- **Low** — cosmetic, documentation, or unlikely-in-practice risk.

Every finding below is evidence-based and references exact file:line locations.

---

## Findings

### F1. [High] Panics on missing `HOME` — guaranteed abort (panic=abort)

`Cargo.toml` sets `panic = "abort"` in the release profile. The following call sites use `expect()` to unwrap `$HOME`:

- `src-tauri/src/services/workspace_service.rs:33`
- `src-tauri/src/services/runtime_key_service.rs:21`
- `src-tauri/src/services/settings_service.rs:7`
- `src-tauri/src/services/profile_service.rs:8`

If `$HOME` is unset (containerised launches, `sudo -E` misconfig, some launcher scripts) any invocation of these paths **aborts the entire app with no error to the user**. Two of the four (`secrets_service`, `local_service`, `known_hosts_service`) already use `unwrap_or_else(|_| "/tmp".to_string())` — the fix is to unify.

**Fix:** fall back to a sensible default (`/tmp` or `/`) like the other services and bubble any write failure up as a user-facing error.

### F2. [High] Mutex unwrap in ssh_session_service — guaranteed abort on poison

`src-tauri/src/services/ssh_session_service.rs:74, 96, 116, 233` call `sessions().lock().unwrap()`. With `panic = "abort"` a poisoned mutex (which can only happen after another panic in the same lock scope, but see F1) aborts the app. Other services (`credentials_store`, `session_trust_store`, etc.) already guard with `if let Ok(mut m) = store().lock()` — unify.

**Fix:** replace with `.ok()` / `if let Ok(...)` handling; treat a poisoned session store as "no session cached", which is safe.

### F3. [High] Transfer progress is incorrect for recursive folder ops

Backend (`sftp_service::download_directory_recursive`, `upload_directory_recursive`, and FTP equivalents) invokes `on_progress(*bytes_done, file_total, name)` where `bytes_done` is **cumulative across all files** but `bytes_total` is **only the current file**.

`src-tauri/src/services/sftp_service.rs:430, 446` and `:522, 538`
`src-tauri/src/services/ftp_service.rs:267, 272, 316, 324`

Frontend (`src/components/file-browser.ts:908-909`) computes `bytePct = bytesDone / bytesTotal`, then clamps to 100%. Result: after the first file, the byte bar pins at 100% permanently for the rest of the folder op.

**Fix:** Send per-file progress in the `bytes_done` field (reset per file), or expose a clean per-file percent. Smallest safe change: send `bytes_done_for_current_file` and keep `bytes_total` per file — the frontend already understands that shape for single-file ops.

### F4. [High] Cancel button does not cancel in-flight transfers

`file-browser.ts:823-833` wires the cancel button to set `this.transferProgress.cancelled = true`. That flag is only checked **between items** in the JS-level loop (e.g. `:1144`, `:1217`, `:1434`, `:1519`). The running Rust `invoke("upload_file", …)` keeps executing to completion, so:

- Single-file upload/download: cancel never takes effect. User sees "Cancelling…" forever until the transfer finishes.
- Recursive folder upload/download via `api.uploadDirectory` / `api.downloadDirectory`: likewise — the whole directory is one backend call.

Fixing cancellation inside `ssh2` blocking reads is a larger effort (needs cooperative cancel tokens plumbed through each callback). A minimum-viable mitigation is to **make the UI state honest**: hide/disable the cancel button for single-file and recursive-directory backend calls, and keep it visible only for JS-level batch loops where the flag actually works.

**Fix (this PR):** cap the cancel button's presence to cases where cancel actually works (N>1 JS loop), OR document-as-known-limitation. Leave real mid-transfer cancel for a later phase.

### F5. [High] Partial-file residue on failed/cancelled transfer

`sftp_service::upload_file` creates the remote file via `sftp.create(remote_path)`; on error mid-loop the partial file remains. Same for `download_file` locally. Folder ops leave partial trees on error.

**Fix (minimum):** on `upload_file` error, attempt `sftp.unlink(remote_path)`; on `download_file` error, attempt `std::fs::remove_file(local_path)`. Non-fatal — log but do not escalate the cleanup failure.

### F6. [Medium] 15-second session timeout applied to every SFTP operation

`sftp_service::connect` calls `session.set_timeout(15_000)` before authentication. In libssh2 this setting persists for the session, so every subsequent SFTP `read`/`write` chunk can time out after 15 s. On a slow link a 256 KB chunk at <17 KB/s stalls a large upload/download with "operation timed out".

**Fix:** lower the timeout to e.g. 10 s during handshake, then bump to a generous value (60 s or 120 s) before SFTP work begins. ssh2-rs supports mutating the timeout on the same session.

### F7. [Medium] Excessive IPC chatter — one `on_progress` event per 256 KB

For a 1 GB upload that is ~4 000 Channel sends at ~0.25 s intervals; the throttle exists only on the frontend (`makeProgressChannel` 80 ms) after the fact. Backpressure-wise this is acceptable, but cheap to improve.

**Fix:** also throttle in Rust — emit at most every `N` chunks OR every 100 ms (`Instant::now()` diff). Non-blocking fix; defer unless a user complains.

### F8. [Medium] FTP loads entire file into memory for upload and download

`ftp_service::upload_file` calls `std::fs::read(local_path)` — whole file in RAM.
`ftp_service::download_file_to` calls `retr_as_buffer` — whole file in RAM.
A user uploading a 5 GB file over FTP will OOM-kill the app.

**Fix (short term):** cap accepted FTP transfer size, or stream via `put_file_from_reader` / `retr`. Lower priority because FTP is a secondary protocol.

### F9. [Medium] Download path collisions never checked — local overwrite silent

`sftp_service::download_file` opens with `std::fs::File::create(local_path)` which **truncates** any pre-existing file silently. `handleDownloadFile` in `file-browser.ts:1343` uses `localPath` auto-destination when configured, bypassing the save dialog that would otherwise ask. Users can therefore silently clobber their own files by downloading same-name items twice.

**Fix:** check `Path::new(local_path).exists()` before the write. Prompt via an overwrite dialog analogous to `showOverwriteDialog`.

### F10. [Medium] Session credentials persist after cancel/auth-failure for the profile id

`commands/connection.rs:28-33`:

```rust
if password.is_some() || passphrase.is_some() {
    credentials_store::set(&profile_id, Credentials { password, passphrase });
}
```

The credential is written into the session store **before** authentication is attempted. On auth failure the mistake is corrected (`connect()` calls `credentials_store::clear`), but if the *user cancels* between host-key accept and the next `connectSftp` retry, the password remains in the session store for the lifetime of the app.

Observed in `main.ts:verifyConnection`: on cancel it flips `connectionCancelled` and returns `false`, but the credential stored during the final successful-connect call stays cached. This is a minor information-retention leak (in-memory only, never touches disk), but it means a **closed-but-cancelled** connection attempt leaves credentials available for any future `connect_sftp` without explicit user re-entry.

**Fix:** also clear session credentials on frontend cancel (equivalent to the disconnect cleanup path).

### F11. [Medium] `open_local_file` treats entire editor string as a single argv[0]

`src-tauri/src/services/local_service.rs:182-190`: an editor setting like `"code --new-window"` is passed unchanged to `Command::new(cmd)`, so execvp tries to find an executable literally named `code --new-window`. Only the workspace service splits editor by whitespace (`workspace_service.rs:150-153`).

Two parallel code paths split or do not split. Inconsistent and will confuse users.

**Fix:** apply `split_whitespace` in `open_local_file` like `workspace_service::open_in_editor` does.

### F12. [Low] Workspace auto-upload error is logged to stderr as `eprintln`

`workspace_service.rs:239` prints `"Auto-upload failed: {}"` to stderr. In production this is acceptable for diagnostic purposes but the error is also emitted as a Tauri event. Ensure no secret ever enters the error string — grepped, all error paths only include filenames and libssh2 error codes, so this is clean.

**No action required.** Record as "intentional, confirmed safe."

### F13. [Low] `downloads_dir()` falls back to `/tmp` if `$HOME` is unset

`commands/sftp.rs:282-290`: if `$HOME` is unset, downloads go to `/tmp`, which is world-writable, periodically purged, and not what the user wants. Combined with F1/F2, `$HOME` unset is already a disaster; but in the few places it's guarded, `/tmp` is a subtle footgun.

**Fix:** return an error instead of `/tmp`.

### F14. [Low] `editor_command` not validated on save

Profile JSON can contain any `editor_command`; since MurmurSSH spawns the command verbatim, the user is effectively granting the command execution rights to anything they save into the profile. A **portable profile** (carried between machines) that embeds `editor_command: "rm -rf ~"` would run that command on any machine that opens it.

Local-machine profiles are user-owned, so this is not a privilege boundary. Portable profiles, however, are designed to be *imported*. Today we store the editor unmodified and run it on open-for-edit.

**Fix:** on import (load) of a portable profile, mark `editor_command` as "requires user confirmation" and prompt on first use. Out-of-scope for this release; track as future hardening.

### F15. [Low] No input validation on `host`, `username`, `port` in Profile

Host is passed into `TcpStream::connect`, username into `libssh2`. These APIs handle malformed input safely — libssh2 does not pass through the shell. No injection vector observed, but missing validation means the profile form accepts garbage (e.g. `host = "; rm -rf /"`). Cosmetic, no exploit path.

**No action required** for this release.

### F16. [Low] Disconnect flow leaves runtime-key file briefly present after app crash

`runtime_key_service::cleanup_all_runtime_keys` is called on startup, so crash residue is wiped at next launch. The window of exposure is only while the app is dead. Given 0600 perms and the user-private config directory, the practical risk is nil.

**No action required.**

### F17. [Low] SSH ControlMaster socket uses `std::env::temp_dir()` — per-user but not per-UID checked

`ssh_session_service.rs:53-55`: socket name is `.murmurssh-ctrl-{profile_id}.sock` in `$TMPDIR` or `/tmp`. On a multi-user system the path is predictable, but the socket file itself is created by the spawned `ssh` process with its own umask. An attacker with local shell on the same box could, in principle, race to create a file at that name (symlink-to-secret attack). The impact is limited to DoS-of-SSO.

**Fix (low priority):** put the socket in `$HOME/.config/murmurssh/run/` with 0700 perms. Defer.

### F18. [Informational] `panic = "abort"` in release profile — documented for context

`src-tauri/Cargo.toml` uses `panic = "abort"` for smaller binary + no unwinding. This is a reasonable production choice but escalates all `.unwrap()` / `.expect()` to application crashes. Findings F1 and F2 gain weight because of this.

**No action required.** Highlighted to inform fix prioritisation.

---

## Recommended fixes for this PR (small, justified, low blast-radius)

1. **F1 fix** — replace `.expect("HOME …")` with `unwrap_or_else(|_| "/".to_string())` in the four services, matching the pattern already established in `secrets_service` and `local_service`.
2. **F2 fix** — switch `ssh_session_service` to `.lock().ok()` + graceful skip on `None`.
3. **F3 fix** — make `bytes_done` in recursive folder progress **per-file** (reset at start of each file); keep `bytes_total` as current file total. Frontend already handles this shape.
4. **F4 fix** — hide the transfer-cancel button during single-file and recursive-directory backend calls where cancel cannot take effect mid-transfer; keep it for JS-level batch loops. UX stays honest.
5. **F5 fix** — on `upload_file` / `download_file` error paths, attempt best-effort cleanup of the partial destination; ignore cleanup errors.
6. **F10 fix** — clear session credentials from the backend when the user cancels a connection attempt (call `clear_session_credentials` from `main.ts`'s connectionCancelled branch).
7. **F11 fix** — split `editor` by whitespace in `local_service::open_local_file`, matching `workspace_service::open_in_editor`.

Deliberately **deferred** (larger design work, or lower value for this gate):

- F6 (session timeout split)
- F7 (Rust-side progress throttling)
- F8 (FTP streaming)
- F9 (local download overwrite confirmation)
- F14 (portable profile editor confirmation)
- F17 (ControlMaster socket relocation)

Each deferred item is either larger than "small and safe" or requires UX work that is out of scope for a production gate.
