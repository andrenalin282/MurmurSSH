# Phase 2 — Transfer Background Queue + Multi-Connection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move all user-initiated file-browser uploads/downloads off the frontend's sequential per-file `invoke` loop and into a backend background queue with a worker pool that runs a configurable number of transfers concurrently (FileZilla-style), keeping the UI responsive and surfacing every job in a queue panel.

**Architecture:** A new backend `transfer_queue` service owns a job list and a single dispatcher thread (Mutex + Condvar). The dispatcher spawns up to `max_concurrent_transfers` worker threads; each worker opens its own SFTP/FTP session (independent connections = real parallelism) and runs one job to completion, emitting `transfer-update` events to the frontend. Per-job cancellation replaces the old profile-keyed flag: the low-level transfer functions are refactored to take a `cancel: &dyn Fn() -> bool` closure, and each job carries an `Arc<AtomicBool>` token. Overwrite resolution stays in the frontend (pre-enqueue) so that polished UX is preserved; the frontend resolves overwrites, enqueues jobs, and renders a queue panel driven entirely by backend events. The workspace remote-edit upload path stays direct (un-queued) by passing a no-op cancel closure.

**Tech Stack:** Rust (std `thread`, `sync::{Mutex, Condvar, Arc, atomic::AtomicBool, OnceLock}`, `ssh2`, `suppaftp`, Tauri 2 `AppHandle::emit`), vanilla TypeScript frontend, Tauri IPC events.

---

## Key design decisions (read before starting)

1. **Queue is the sole engine for file-browser transfers.** The old `Channel`-based transfer commands (`upload_file`, `download_file`, `download_file_to`, `upload_directory`, `download_directory`, `upload_path`) are **removed** and replaced by `enqueue_transfer`. `upload_file_bytes` (in-memory New File / small content) stays direct. The workspace edit-flow stays direct.
2. **Per-job cancellation.** The profile-keyed `transfer_cancel` module is **deleted**; transfer functions take `cancel: &dyn Fn() -> bool`. The `CANCELLED_ERROR` sentinel (`"TRANSFER_CANCELLED"`) moves to `models/transfer.rs` so the frontend's `isCancelErr` keeps working.
3. **Overwrite resolution stays frontend-side, pre-enqueue.** No backend overwrite dialog. The frontend calls `resolveOverwrite` (existing) before enqueuing each job, exactly as today.
4. **Events, not return values.** Jobs report progress/state via the global `transfer-update` Tauri event (payload = one `TransferJobView`). The frontend keeps a `Map<id, TransferJobView>` and renders the queue panel from it.
5. **Concurrency is dynamic.** The dispatcher reads `max_concurrent_transfers` from Settings on every dispatch decision (default 2, clamped 1..=8). Changing it in Settings affects the next dispatch without restart.
6. **Folder ops are single jobs.** A recursive folder upload/download is one queue job (kind `uploadDir`/`downloadDir`); its byte counters reflect the current file within the folder, matching today's per-file progress shape.

---

## File structure

**Backend — new:**
- `src-tauri/src/models/transfer.rs` — `TransferKind`, `TransferState`, `TransferJobView` (serde), `CANCELLED_ERROR`, `clamp_concurrency()`.
- `src-tauri/src/services/transfer_queue.rs` — queue state, dispatcher, workers, public API.
- `src-tauri/src/commands/transfer.rs` — `enqueue_transfer`, `cancel_transfer`, `cancel_all_transfers`, `list_transfers`, `clear_finished_transfers`.

**Backend — modified:**
- `src-tauri/src/models/mod.rs` — export transfer model.
- `src-tauri/src/models/settings.rs` — add `max_concurrent_transfers: Option<u32>`.
- `src-tauri/src/services/mod.rs` — register `transfer_queue`, drop `transfer_cancel`.
- `src-tauri/src/commands/mod.rs` — register `transfer`.
- `src-tauri/src/services/sftp_service.rs` — transfer fns take `cancel: &dyn Fn() -> bool`; drop `transfer_cancel`.
- `src-tauri/src/services/ftp_service.rs` — same refactor.
- `src-tauri/src/services/workspace_service.rs` — pass `&|| false` to the two direct upload/download calls.
- `src-tauri/src/commands/sftp.rs` — remove queued transfer commands + `cancel_transfer`; keep `upload_file_bytes`, `local_file_exists`, `remote_file_exists`, listing, delete, rename, set_permissions, create_directory.
- `src-tauri/src/services/transfer_cancel.rs` — **deleted**.
- `src-tauri/src/lib.rs` — `transfer_queue::init(&app)` at startup; `cancel_all()` in `cleanup_on_exit`; register the new commands; drop the removed ones.

**Frontend — new:**
- `src/components/transfer-queue.ts` — `TransferQueuePanel` component (renders jobs map, per-job + cancel-all buttons, listens to `transfer-update`).

**Frontend — modified:**
- `src/types.ts` — `TransferJobView` type.
- `src/api/index.ts` — `enqueueTransfer`, `cancelTransfer(jobId)`, `cancelAllTransfers`, `listTransfers`, `clearFinishedTransfers`; remove old transfer wrappers + `TransferChannel`.
- `src/components/file-browser.ts` — upload/download flows resolve overwrites then enqueue; remove single-bar `transferProgress` machinery (`startTransfer`/`updateTransfer`/`updateFromChannel`/`_flushTransferDom`/`endTransfer`/`makeProgressChannel`) and the in-render progress bar.
- `src/components/settings-dialog.ts` — concurrency number input.
- `src/main.ts` — instantiate `TransferQueuePanel`, wire browser refresh on terminal job events.
- `src/i18n/{en,de,fr,nl,pl,ru}.{ts,js}` — new strings.

**Frontend build rule (MANDATORY, from CLAUDE.md):** `tsc` emits `.js` next to each `.ts`, and the `.js` are git-tracked. For every `.ts` edit: run `npx tsc`, then `git add` BOTH the `.ts` and its sibling `.js`. Before committing, `git checkout --` any unrelated regenerated `.js` (the recurring `src/i18n/index.js` whitespace noise in particular).

---

## Task 1: Settings field + concurrency clamp helper

**Files:**
- Modify: `src-tauri/src/models/settings.rs`
- Create: `src-tauri/src/models/transfer.rs`
- Modify: `src-tauri/src/models/mod.rs`

- [ ] **Step 1: Add the Settings field**

In `src-tauri/src/models/settings.rs`, add after `local_browser_position`:

```rust
    /// Maximum number of transfers the background queue runs concurrently.
    /// When None, defaults to 2. Clamped to 1..=8 at use sites.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrent_transfers: Option<u32>,
```

- [ ] **Step 2: Write the failing test for the model + clamp helper**

Create `src-tauri/src/models/transfer.rs`:

```rust
use serde::Serialize;

/// Sentinel returned from transfer functions when a cancel was requested.
/// The frontend recognises this prefix and renders a cancel-friendly status.
pub const CANCELLED_ERROR: &str = "TRANSFER_CANCELLED";

/// Default number of concurrent transfers when the setting is unset.
pub const DEFAULT_CONCURRENCY: u32 = 2;

/// Clamp a requested concurrency to the supported range (1..=8).
/// `None` (setting unset) yields the default.
pub fn clamp_concurrency(requested: Option<u32>) -> u32 {
    requested.unwrap_or(DEFAULT_CONCURRENCY).clamp(1, 8)
}

/// What a queued job does. Serialized to the frontend in camelCase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferKind {
    Upload,
    Download,
    UploadDir,
    DownloadDir,
}

/// Lifecycle state of a queued job. Serialized to the frontend in camelCase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferState {
    Queued,
    Active,
    Done,
    Failed,
    Cancelled,
}

/// Immutable snapshot of a job sent to the frontend via the `transfer-update`
/// event and `list_transfers`. Never holds the cancel token or thread state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferJobView {
    pub id: u64,
    pub profile_id: String,
    pub kind: TransferKind,
    /// Source path (local for uploads, remote for downloads).
    pub src: String,
    /// Destination path (remote for uploads, local for downloads).
    pub dst: String,
    /// Display name of the item (or current file within a folder op).
    pub filename: String,
    pub state: TransferState,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_concurrency_defaults_and_bounds() {
        assert_eq!(clamp_concurrency(None), DEFAULT_CONCURRENCY);
        assert_eq!(clamp_concurrency(Some(0)), 1);
        assert_eq!(clamp_concurrency(Some(1)), 1);
        assert_eq!(clamp_concurrency(Some(4)), 4);
        assert_eq!(clamp_concurrency(Some(8)), 8);
        assert_eq!(clamp_concurrency(Some(99)), 8);
    }

    #[test]
    fn kind_and_state_serialize_camel_case() {
        assert_eq!(
            serde_json::to_string(&TransferKind::UploadDir).unwrap(),
            "\"uploadDir\""
        );
        assert_eq!(
            serde_json::to_string(&TransferState::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }
}
```

- [ ] **Step 3: Export the model**

In `src-tauri/src/models/mod.rs`, add the module + re-exports. Match the existing pattern (other models are `pub mod` + `pub use`). Add:

```rust
pub mod transfer;
pub use transfer::{
    clamp_concurrency, TransferJobView, TransferKind, TransferState, CANCELLED_ERROR,
    DEFAULT_CONCURRENCY,
};
```

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test --lib models::transfer`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/models/transfer.rs src-tauri/src/models/mod.rs src-tauri/src/models/settings.rs
git commit -m "feat(transfer): add transfer job model, concurrency clamp, and settings field"
```

---

## Task 2: Refactor low-level transfer functions to take a cancel closure

This removes `transfer_cancel` (profile-keyed) and threads a `cancel: &dyn Fn() -> bool` through every transfer function and recursive helper in both services, so the queue can cancel individual jobs. Behaviour is otherwise identical.

**Files:**
- Modify: `src-tauri/src/services/sftp_service.rs`
- Modify: `src-tauri/src/services/ftp_service.rs`
- Modify: `src-tauri/src/services/workspace_service.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Delete: `src-tauri/src/services/transfer_cancel.rs`

- [ ] **Step 1: sftp_service — change the public + inner signatures**

In `src-tauri/src/services/sftp_service.rs`:

- Change the import line `use crate::services::{credentials_store, known_hosts_service, transfer_cancel};` to:

```rust
use crate::models::CANCELLED_ERROR;
use crate::services::{credentials_store, known_hosts_service};
```

- For each of `upload_file`, `download_file`, `upload_directory`, `download_directory` (the public wrappers): remove the `transfer_cancel::clear(&profile.id);` lines and the wrapper indirection. Replace the wrapper bodies so they call the `_inner`/recursive function directly, adding a `cancel: &dyn Fn() -> bool` parameter. Example for `upload_file`:

```rust
pub fn upload_file(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    upload_file_inner(profile, local_path, remote_path, cancel, on_progress)
}
```

- In every `*_inner` and `*_recursive` function, add `cancel: &dyn Fn() -> bool` as a parameter (place it right before `on_progress`), and replace each:
  - `if transfer_cancel::is_cancelled(&profile.id) {` and `if transfer_cancel::is_cancelled(profile_id) {` → `if cancel() {`
  - `break Err(transfer_cancel::CANCELLED_ERROR.to_string());` → `break Err(CANCELLED_ERROR.to_string());`
  - `return Err(transfer_cancel::CANCELLED_ERROR.to_string());` → `return Err(CANCELLED_ERROR.to_string());`
- For the recursive helpers (`download_directory_recursive`, `upload_directory_recursive`) the `profile_id: &str` param is no longer needed for cancellation, but it is unused elsewhere — drop it and pass `cancel` through the recursion instead. Update the recursive call sites to pass `cancel`.
- `upload_bytes`, `remote_file_exists`, `delete_file`, `rename_file`, `set_permissions`, `create_directory`, `delete_directory`, `list_directory`, `get_sftp_home` are **unchanged** (no cancel param).

- [ ] **Step 2: ftp_service — same refactor**

In `src-tauri/src/services/ftp_service.rs`:
- Change `use crate::services::{credentials_store, transfer_cancel};` → `use crate::models::CANCELLED_ERROR;` plus `use crate::services::credentials_store;`.
- Add `cancel: &dyn Fn() -> bool` to `upload_file`, `download_file_to`, `upload_directory`, `download_directory`, and the helpers `retr_stream_to_file`, `upload_dir_recursive`, `download_dir_recursive` (before `on_progress`).
- Remove all `transfer_cancel::clear(&profile.id);` lines (including the ones in the error arms like `Err(e) => { transfer_cancel::clear(&profile.id); return Err(e); }` → `Err(e) => return Err(e),`).
- Replace `transfer_cancel::is_cancelled(profile_id)` → `cancel()` and `transfer_cancel::CANCELLED_ERROR` → `CANCELLED_ERROR`.
- The helpers currently take `profile_id: &str` only for cancellation — drop that param and thread `cancel` instead; update call sites.

- [ ] **Step 3: workspace_service — pass no-op cancel to the direct edit-flow calls**

In `src-tauri/src/services/workspace_service.rs`, update the two call sites:
- Line ~131/133 (download in `open_for_edit`):

```rust
        ftp_service::download_file_to(profile, remote_path, local_str, &|| false, &|_, _, _| {})
        // ...
        sftp_service::download_file(profile, remote_path, local_str, &|| false, &|_, _| {})
```

- Line ~285/287 (auto-upload in `watch_and_upload`):

```rust
            ftp_service::upload_file(&profile, local_str, &remote_path, &|| false, &|_, _, _| {})
            // ...
            sftp_service::upload_file(&profile, local_str, &remote_path, &|| false, &|_, _| {})
```

(Add the `cancel` arg positionally before the progress closure — match whichever order Step 1/2 used: `cancel` then `on_progress`.)

- [ ] **Step 4: Delete transfer_cancel and deregister it**

```bash
git rm src-tauri/src/services/transfer_cancel.rs
```

In `src-tauri/src/services/mod.rs`, remove the `pub mod transfer_cancel;` line (and any `pub use transfer_cancel::...`).

- [ ] **Step 5: Verify it compiles (commands still reference old signatures — expected to fail here)**

Run: `cd src-tauri && cargo check`
Expected: errors **only** in `commands/sftp.rs` (old call sites missing the `cancel` arg) and possibly `lib.rs`. The services + workspace must compile clean. Task 5 fixes the command layer. If errors appear in `sftp_service.rs`/`ftp_service.rs`/`workspace_service.rs`, fix them before moving on.

- [ ] **Step 6: Commit**

```bash
git add -A src-tauri/src/services/
git commit -m "refactor(transfer): thread per-call cancel closure through transfer fns; remove profile-keyed transfer_cancel"
```

(The tree won't fully build until Task 5; that's acceptable for this WIP commit since it's an internal refactor landing alongside the queue.)

---

## Task 3: transfer_queue service — state, API, dispatcher, workers

**Files:**
- Create: `src-tauri/src/services/transfer_queue.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Write the service with unit tests**

Create `src-tauri/src/services/transfer_queue.rs`:

```rust
//! Background transfer queue with a worker pool.
//!
//! A single dispatcher thread owns the job list (behind a Mutex + Condvar).
//! When jobs are queued or a worker finishes, the dispatcher promotes Queued
//! jobs to Active and spawns a worker thread per job, up to the configured
//! concurrency. Each worker opens its own SFTP/FTP session, so transfers run
//! on independent connections in parallel. Progress and lifecycle are reported
//! to the frontend via the `transfer-update` Tauri event.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::models::{
    clamp_concurrency, Profile, TransferJobView, TransferKind, TransferState,
};
use crate::services::{ftp_service, profile_service, settings_service, sftp_service};

/// Tauri event name carrying a single `TransferJobView` snapshot.
pub const EVENT_TRANSFER_UPDATE: &str = "transfer-update";

/// Internal job record. The `view` mirrors what the frontend sees; `cancel` is
/// the per-job token polled by the transfer functions.
struct Job {
    view: TransferJobView,
    cancel: Arc<AtomicBool>,
}

struct QueueState {
    next_id: u64,
    jobs: Vec<Job>,
    active: usize,
}

struct Queue {
    state: Mutex<QueueState>,
    cv: Condvar,
}

fn queue() -> &'static Queue {
    static Q: OnceLock<Queue> = OnceLock::new();
    Q.get_or_init(|| Queue {
        state: Mutex::new(QueueState {
            next_id: 1,
            jobs: Vec::new(),
            active: 0,
        }),
        cv: Condvar::new(),
    })
}

fn app_handle() -> &'static Mutex<Option<AppHandle>> {
    static H: OnceLock<Mutex<Option<AppHandle>>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(None))
}

fn emit(view: &TransferJobView) {
    if let Ok(guard) = app_handle().lock() {
        if let Some(app) = guard.as_ref() {
            let _ = app.emit(EVENT_TRANSFER_UPDATE, view.clone());
        }
    }
}

/// Store the app handle and start the dispatcher thread (idempotent-ish: call once at startup).
pub fn init(app: &AppHandle) {
    if let Ok(mut guard) = app_handle().lock() {
        *guard = Some(app.clone());
    }
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_ok() {
        std::thread::spawn(dispatcher_loop);
    }
}

/// Read the current concurrency from settings, clamped to 1..=8.
fn current_concurrency() -> usize {
    let requested = settings_service::get_settings()
        .ok()
        .and_then(|s| s.max_concurrent_transfers);
    clamp_concurrency(requested) as usize
}

/// Enqueue a new job and wake the dispatcher. Returns the job id.
pub fn enqueue(
    profile_id: String,
    kind: TransferKind,
    src: String,
    dst: String,
    filename: String,
) -> u64 {
    let view = {
        let mut st = queue().state.lock().unwrap();
        let id = st.next_id;
        st.next_id += 1;
        let view = TransferJobView {
            id,
            profile_id,
            kind,
            src,
            dst,
            filename,
            state: TransferState::Queued,
            bytes_done: 0,
            bytes_total: 0,
            error: None,
        };
        st.jobs.push(Job {
            view: view.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
        });
        view
    };
    emit(&view);
    queue().cv.notify_all();
    view.id
}

/// Snapshot of all jobs for the frontend.
pub fn list() -> Vec<TransferJobView> {
    queue()
        .state
        .lock()
        .unwrap()
        .jobs
        .iter()
        .map(|j| j.view.clone())
        .collect()
}

/// Cancel one job. If still Queued, mark Cancelled immediately and emit.
pub fn cancel(job_id: u64) {
    let mut emit_view: Option<TransferJobView> = None;
    {
        let mut st = queue().state.lock().unwrap();
        if let Some(job) = st.jobs.iter_mut().find(|j| j.view.id == job_id) {
            job.cancel.store(true, Ordering::Relaxed);
            if job.view.state == TransferState::Queued {
                job.view.state = TransferState::Cancelled;
                emit_view = Some(job.view.clone());
            }
        }
    }
    if let Some(v) = emit_view {
        emit(&v);
    }
    queue().cv.notify_all();
}

/// Cancel every job that is not already finished.
pub fn cancel_all() {
    let mut to_emit: Vec<TransferJobView> = Vec::new();
    {
        let mut st = queue().state.lock().unwrap();
        for job in st.jobs.iter_mut() {
            match job.view.state {
                TransferState::Queued => {
                    job.cancel.store(true, Ordering::Relaxed);
                    job.view.state = TransferState::Cancelled;
                    to_emit.push(job.view.clone());
                }
                TransferState::Active => {
                    job.cancel.store(true, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    }
    for v in &to_emit {
        emit(v);
    }
    queue().cv.notify_all();
}

/// Remove finished jobs (Done/Failed/Cancelled) from the list.
pub fn clear_finished() {
    let mut st = queue().state.lock().unwrap();
    st.jobs.retain(|j| {
        matches!(j.view.state, TransferState::Queued | TransferState::Active)
    });
}

/// Dispatcher: promote Queued → Active up to the concurrency limit, spawning a
/// worker per promoted job. Blocks on the condvar otherwise.
fn dispatcher_loop() {
    loop {
        let mut st = queue().state.lock().unwrap();
        loop {
            let limit = current_concurrency();
            if st.active >= limit {
                break;
            }
            // Find the next Queued job (FIFO).
            let idx = st.jobs.iter().position(|j| j.view.state == TransferState::Queued);
            let Some(idx) = idx else { break };
            st.jobs[idx].view.state = TransferState::Active;
            st.active += 1;
            let view = st.jobs[idx].view.clone();
            let cancel = st.jobs[idx].cancel.clone();
            emit(&view);
            std::thread::spawn(move || run_worker(view, cancel));
        }
        // Wait until something changes (enqueue / cancel / worker finished).
        let _unused = queue().cv.wait(st).unwrap();
    }
}

/// Update a job's view in place (used by progress callbacks and finalisation).
fn with_job<F: FnOnce(&mut TransferJobView)>(job_id: u64, f: F) -> Option<TransferJobView> {
    let mut st = queue().state.lock().unwrap();
    let job = st.jobs.iter_mut().find(|j| j.view.id == job_id)?;
    f(&mut job.view);
    Some(job.view.clone())
}

/// Run one job to completion on this worker thread, then release the slot.
fn run_worker(view: TransferJobView, cancel: Arc<AtomicBool>) {
    let job_id = view.id;
    let result = run_job(&view, &cancel);

    // Finalise state.
    let final_view = with_job(job_id, |v| {
        match &result {
            Ok(()) => {
                v.state = TransferState::Done;
                if v.bytes_total > 0 {
                    v.bytes_done = v.bytes_total;
                }
            }
            Err(e) if e == crate::models::CANCELLED_ERROR => {
                v.state = TransferState::Cancelled;
            }
            Err(e) => {
                v.state = TransferState::Failed;
                v.error = Some(e.clone());
            }
        }
    });
    if let Some(v) = final_view {
        emit(&v);
    }

    // Release the slot and wake the dispatcher.
    {
        let mut st = queue().state.lock().unwrap();
        st.active = st.active.saturating_sub(1);
    }
    queue().cv.notify_all();
}

/// Resolve the profile and dispatch to the right service function.
/// Progress callbacks update the job view and emit a throttled `transfer-update`.
fn run_job(view: &TransferJobView, cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let profile = profile_service::get_profile(&view.profile_id)?;
    let is_ftp = profile.protocol.as_ref() == Some(&crate::models::Protocol::Ftp);
    let job_id = view.id;
    let cancel_fn = {
        let c = cancel.clone();
        move || c.load(Ordering::Relaxed)
    };

    // Throttle progress emission to ~10/s per job.
    let last = Mutex::new(Instant::now());
    let emit_progress = |done: u64, total: u64, name: &str| {
        let should = {
            let mut l = last.lock().unwrap();
            if l.elapsed() >= Duration::from_millis(100) {
                *l = Instant::now();
                true
            } else {
                false
            }
        };
        if should {
            if let Some(v) = with_job(job_id, |v| {
                v.bytes_done = done;
                v.bytes_total = total;
                if !name.is_empty() {
                    v.filename = name.to_string();
                }
            }) {
                emit(&v);
            }
        }
    };

    match view.kind {
        TransferKind::Upload => {
            let fname = view.filename.clone();
            if is_ftp {
                ftp_service::upload_file(&profile, &view.src, &view.dst, &cancel_fn, &|d, t, n| {
                    emit_progress(d, t, n);
                })
            } else {
                sftp_service::upload_file(&profile, &view.src, &view.dst, &cancel_fn, &|d, t| {
                    emit_progress(d, t, &fname);
                })
            }
        }
        TransferKind::Download => {
            let fname = view.filename.clone();
            if is_ftp {
                ftp_service::download_file_to(&profile, &view.src, &view.dst, &cancel_fn, &|d, t, n| {
                    emit_progress(d, t, n);
                })
            } else {
                sftp_service::download_file(&profile, &view.src, &view.dst, &cancel_fn, &|d, t| {
                    emit_progress(d, t, &fname);
                })
            }
        }
        TransferKind::UploadDir => {
            if is_ftp {
                ftp_service::upload_directory(&profile, &view.src, &view.dst, &cancel_fn, &|d, t, n| {
                    emit_progress(d, t, n);
                })
            } else {
                sftp_service::upload_directory(&profile, &view.src, &view.dst, &cancel_fn, &|d, t, n| {
                    emit_progress(d, t, n);
                })
            }
        }
        TransferKind::DownloadDir => {
            if is_ftp {
                ftp_service::download_directory(&profile, &view.src, &view.dst, &cancel_fn, &|d, t, n| {
                    emit_progress(d, t, n);
                })
            } else {
                sftp_service::download_directory(&profile, &view.src, &view.dst, &cancel_fn, &|d, t, n| {
                    emit_progress(d, t, n);
                })
            }
        }
    }
}

// Silence unused import warning when Profile is only used via profile_service.
#[allow(unused_imports)]
use Profile as _ProfileMarker;

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        let mut st = queue().state.lock().unwrap();
        st.jobs.clear();
        st.active = 0;
        // next_id intentionally not reset — ids are monotonic across the process.
    }

    #[test]
    fn enqueue_adds_queued_job_and_list_returns_it() {
        reset();
        let id = enqueue(
            "p1".into(),
            TransferKind::Upload,
            "/local/a".into(),
            "/remote/a".into(),
            "a".into(),
        );
        let jobs = list();
        let job = jobs.iter().find(|j| j.id == id).expect("job present");
        assert_eq!(job.state, TransferState::Queued);
        assert_eq!(job.filename, "a");
        assert_eq!(job.profile_id, "p1");
    }

    #[test]
    fn cancel_queued_marks_cancelled() {
        reset();
        let id = enqueue(
            "p1".into(),
            TransferKind::Download,
            "/remote/b".into(),
            "/local/b".into(),
            "b".into(),
        );
        cancel(id);
        let jobs = list();
        let job = jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(job.state, TransferState::Cancelled);
    }

    #[test]
    fn clear_finished_drops_terminal_jobs_only() {
        reset();
        let a = enqueue("p".into(), TransferKind::Upload, "s".into(), "d".into(), "a".into());
        let b = enqueue("p".into(), TransferKind::Upload, "s".into(), "d".into(), "b".into());
        cancel(a); // a -> Cancelled (terminal), b stays Queued
        clear_finished();
        let ids: Vec<u64> = list().iter().map(|j| j.id).collect();
        assert!(!ids.contains(&a));
        assert!(ids.contains(&b));
    }
}
```

> NOTE for the implementer: the `_ProfileMarker` shim is a guard against an unused-import error — if `cargo check` shows `Profile` *is* used (it is referenced in `run_job` via `crate::models::Protocol` only, not `Profile` directly), **delete the `use crate::models::{... Profile ...}` entry and the `_ProfileMarker` block** and import just what's used (`clamp_concurrency, TransferJobView, TransferKind, TransferState, Protocol`). Resolve the actual import set so there are **zero warnings** — do not leave the shim if it isn't needed.

- [ ] **Step 2: Register the module**

In `src-tauri/src/services/mod.rs`, add `pub mod transfer_queue;` (alphabetical position near the other services).

- [ ] **Step 3: Run the unit tests**

Run: `cd src-tauri && cargo test --lib services::transfer_queue`
Expected: PASS (3 tests). Fix any warnings (`cargo check` should be warning-clean for this file).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/transfer_queue.rs src-tauri/src/services/mod.rs
git commit -m "feat(transfer): background queue service with worker pool and dispatcher"
```

---

## Task 4: Commands + lib wiring

**Files:**
- Create: `src-tauri/src/commands/transfer.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/commands/sftp.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create the transfer commands**

Create `src-tauri/src/commands/transfer.rs`:

```rust
use crate::models::{TransferJobView, TransferKind};
use crate::services::transfer_queue;

/// Enqueue a transfer job. `kind` is one of "upload" | "download" | "uploadDir"
/// | "downloadDir". For uploads: src = local path, dst = remote path. For
/// downloads: src = remote path, dst = local path. Returns the new job id.
#[tauri::command]
pub fn enqueue_transfer(
    profile_id: String,
    kind: String,
    src: String,
    dst: String,
    filename: String,
) -> Result<u64, String> {
    let kind = match kind.as_str() {
        "upload" => TransferKind::Upload,
        "download" => TransferKind::Download,
        "uploadDir" => TransferKind::UploadDir,
        "downloadDir" => TransferKind::DownloadDir,
        other => return Err(format!("Unknown transfer kind: {}", other)),
    };
    Ok(transfer_queue::enqueue(profile_id, kind, src, dst, filename))
}

#[tauri::command]
pub fn cancel_transfer(job_id: u64) {
    transfer_queue::cancel(job_id);
}

#[tauri::command]
pub fn cancel_all_transfers() {
    transfer_queue::cancel_all();
}

#[tauri::command]
pub fn list_transfers() -> Vec<TransferJobView> {
    transfer_queue::list()
}

#[tauri::command]
pub fn clear_finished_transfers() {
    transfer_queue::clear_finished();
}
```

- [ ] **Step 2: Register the command module**

In `src-tauri/src/commands/mod.rs`, add `pub mod transfer;`.

- [ ] **Step 3: Remove the obsolete queued transfer commands from sftp.rs**

In `src-tauri/src/commands/sftp.rs`, **delete** these functions: `upload_file`, `download_file_to`, `download_file`, `upload_directory`, `upload_path`, `download_directory`, and `cancel_transfer`. Also delete the now-unused `TransferProgress` struct, the `use tauri::ipc::Channel;` import, the `transfer_cancel` import, and `downloads_dir()` if it is now unreferenced.

**Keep:** `get_sftp_home`, `list_directory`, `remote_file_exists`, `upload_file_bytes`, `delete_file`, `rename_file`, `set_permissions`, `create_directory`, `delete_directory`, `local_file_exists`, and the `is_ftp` helper.

- [ ] **Step 4: Wire lib.rs**

In `src-tauri/src/lib.rs`:
- In `cleanup_on_exit()`, add as the first line:

```rust
    services::transfer_queue::cancel_all();
```

- Change the builder to capture the app handle and call `transfer_queue::init`. Replace the `.build(tauri::generate_context!())` chain with a `.setup` hook:

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            services::transfer_queue::init(&app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ... existing handlers ...
        ])
        .build(tauri::generate_context!())
        .expect("error while building MurmurSSH")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                cleanup_on_exit();
            }
        });
```

- In the `generate_handler!` list: **remove** `commands::sftp::upload_file`, `commands::sftp::download_file`, `commands::sftp::download_file_to`, `commands::sftp::upload_directory`, `commands::sftp::upload_path`, `commands::sftp::download_directory`, `commands::sftp::cancel_transfer`. **Add**:

```rust
            commands::transfer::enqueue_transfer,
            commands::transfer::cancel_transfer,
            commands::transfer::cancel_all_transfers,
            commands::transfer::list_transfers,
            commands::transfer::clear_finished_transfers,
```

- [ ] **Step 5: Full backend build**

Run: `cd src-tauri && cargo build 2>&1 | tail -30`
Expected: builds clean, no warnings. Fix anything outstanding (unused imports in sftp.rs, etc.).

- [ ] **Step 6: Run all backend tests**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -20`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add -A src-tauri/src/
git commit -m "feat(transfer): queue commands, route transfers through queue, drain on exit"
```

---

## Task 5: Frontend types + API wrappers

**Files:**
- Modify: `src/types.ts`
- Modify: `src/api/index.ts`

- [ ] **Step 1: Add the TransferJobView type**

In `src/types.ts`, add:

```typescript
/** A transfer queue job snapshot, mirrors Rust TransferJobView. */
export interface TransferJobView {
  id: number;
  profileId: string;
  kind: "upload" | "download" | "uploadDir" | "downloadDir";
  src: string;
  dst: string;
  filename: string;
  state: "queued" | "active" | "done" | "failed" | "cancelled";
  bytesDone: number;
  bytesTotal: number;
  error: string | null;
}
```

- [ ] **Step 2: Replace transfer wrappers in the API**

In `src/api/index.ts`:
- Remove `TransferProgress`, `TransferChannel`, `makeProgressChannel` (if present here), and the wrappers `uploadPath`, `uploadDirectory`, `uploadFile`, `downloadFile`, `downloadFileTo`, `downloadDirectory`, and the old `cancelTransfer(profileId)`. Remove the `Channel` import if no longer used.
- Add:

```typescript
import type { TransferJobView } from "../types";

export async function enqueueTransfer(
  profileId: string,
  kind: TransferJobView["kind"],
  src: string,
  dst: string,
  filename: string
): Promise<number> {
  return invoke("enqueue_transfer", { profileId, kind, src, dst, filename });
}

export async function cancelTransfer(jobId: number): Promise<void> {
  return invoke("cancel_transfer", { jobId });
}

export async function cancelAllTransfers(): Promise<void> {
  return invoke("cancel_all_transfers");
}

export async function listTransfers(): Promise<TransferJobView[]> {
  return invoke("list_transfers");
}

export async function clearFinishedTransfers(): Promise<void> {
  return invoke("clear_finished_transfers");
}
```

- [ ] **Step 3: Compile (frontend will not fully build until file-browser is updated)**

Run: `npx tsc --noEmit 2>&1 | head -30`
Expected: errors only in `file-browser.ts` (uses removed wrappers). `types.ts`/`api/index.ts` must be clean. Those file-browser errors are fixed in Task 7. Do not run plain `npx tsc` (which emits) yet — wait until Task 7 so `.js` siblings are regenerated together.

- [ ] **Step 4: Commit (after Task 7 compiles cleanly)** — defer the commit; bundle types+api+file-browser+panel in Task 7's commit so the tree never has broken `.js`. Mark this step done once Task 7 lands.

---

## Task 6: Transfer queue panel component

**Files:**
- Create: `src/components/transfer-queue.ts`

- [ ] **Step 1: Write the panel component**

Create `src/components/transfer-queue.ts`:

```typescript
import { listen } from "@tauri-apps/api/event";
import * as api from "../api/index";
import type { TransferJobView } from "../types";
import { t } from "../i18n";

function escHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function fmtPct(j: TransferJobView): number {
  if (j.state === "done") return 100;
  if (j.bytesTotal > 0) return Math.min(100, (j.bytesDone / j.bytesTotal) * 100);
  return j.state === "active" ? 100 : 0; // indeterminate -> full bar styled separately
}

/**
 * Bottom queue panel. Holds a map of jobs keyed by id, updated from the
 * `transfer-update` Tauri event, and re-renders on each change. Hidden when
 * there are no jobs.
 */
export class TransferQueuePanel {
  private container: HTMLElement;
  private jobs = new Map<number, TransferJobView>();
  /** Called when any job reaches a terminal state, with the finished job. */
  private onJobFinished: ((job: TransferJobView) => void) | null = null;

  constructor(containerId: string) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;
    this.render();
    void this.subscribe();
  }

  setOnJobFinished(cb: (job: TransferJobView) => void): void {
    this.onJobFinished = cb;
  }

  private async subscribe(): Promise<void> {
    // Seed from any jobs already in the backend (e.g. after a reload).
    try {
      for (const j of await api.listTransfers()) this.jobs.set(j.id, j);
      this.render();
    } catch {
      /* ignore */
    }
    await listen<TransferJobView>("transfer-update", (e) => {
      const job = e.payload;
      this.jobs.set(job.id, job);
      this.render();
      if (job.state === "done" || job.state === "failed" || job.state === "cancelled") {
        this.onJobFinished?.(job);
      }
    });
  }

  private activeOrQueued(): boolean {
    for (const j of this.jobs.values()) {
      if (j.state === "queued" || j.state === "active") return true;
    }
    return false;
  }

  private render(): void {
    if (this.jobs.size === 0) {
      this.container.innerHTML = "";
      this.container.classList.add("hidden");
      return;
    }
    this.container.classList.remove("hidden");

    const rows = [...this.jobs.values()]
      .sort((a, b) => a.id - b.id)
      .map((j) => {
        const pct = fmtPct(j);
        const indeterminate = j.state === "active" && j.bytesTotal === 0;
        const stateLabel = t(`transferQueue.state_${j.state}`);
        const cancellable = j.state === "queued" || j.state === "active";
        const arrow = j.kind === "upload" || j.kind === "uploadDir" ? "↑" : "↓";
        return `
          <div class="tq-row tq-row--${j.state}">
            <span class="tq-arrow">${arrow}</span>
            <span class="tq-name" title="${escHtml(j.dst)}">${escHtml(j.filename)}</span>
            <span class="tq-state">${escHtml(stateLabel)}</span>
            <div class="tq-track">
              <div class="tq-fill ${indeterminate ? "tq-fill--indeterminate" : ""}" style="width:${pct.toFixed(1)}%"></div>
            </div>
            ${j.error ? `<span class="tq-error" title="${escHtml(j.error)}">${escHtml(j.error)}</span>` : ""}
            ${cancellable ? `<button class="tq-cancel" data-job="${j.id}" title="${escHtml(t("transferQueue.cancel"))}">✕</button>` : ""}
          </div>`;
      })
      .join("");

    this.container.innerHTML = `
      <div class="tq-header">
        <span class="tq-title">${escHtml(t("transferQueue.title"))}</span>
        ${this.activeOrQueued() ? `<button class="tq-cancel-all" id="tq-cancel-all">${escHtml(t("transferQueue.cancelAll"))}</button>` : ""}
        <button class="tq-clear" id="tq-clear">${escHtml(t("transferQueue.clearFinished"))}</button>
      </div>
      <div class="tq-rows">${rows}</div>`;

    this.container.querySelectorAll<HTMLButtonElement>(".tq-cancel").forEach((btn) => {
      btn.addEventListener("click", () => {
        const id = Number(btn.dataset.job);
        if (!Number.isNaN(id)) api.cancelTransfer(id).catch(() => {});
      });
    });
    this.container.querySelector("#tq-cancel-all")?.addEventListener("click", () => {
      api.cancelAllTransfers().catch(() => {});
    });
    this.container.querySelector("#tq-clear")?.addEventListener("click", () => {
      for (const [id, j] of [...this.jobs.entries()]) {
        if (j.state === "done" || j.state === "failed" || j.state === "cancelled") this.jobs.delete(id);
      }
      api.clearFinishedTransfers().catch(() => {});
      this.render();
    });
  }
}
```

- [ ] **Step 2: Add the panel container + styles**

- In `index.html`, add a container `<div id="transfer-queue" class="transfer-queue hidden"></div>` in the appropriate spot (below the browsers pane / above the status bar — match the existing layout; inspect `index.html` for the right sibling).
- In the stylesheet (find the existing `.transfer-progress` rules and place these alongside), add `.transfer-queue`, `.tq-header`, `.tq-rows`, `.tq-row`, `.tq-track`, `.tq-fill`, `.tq-fill--indeterminate` (a CSS animation), `.tq-row--failed`/`--cancelled`/`--done` accent colors, and `.hidden { display: none; }` if not already defined. Reuse existing CSS custom properties (`var(--...)`) for colors to stay theme-consistent.

- [ ] **Step 3: Compile** — included in Task 7's tsc run.

---

## Task 7: Rewrite file-browser transfer flows to enqueue jobs

This is the largest frontend change. The principle: **resolve overwrites exactly as today, then enqueue a job per item instead of awaiting a Channel-based transfer.** Remove the single-progress-bar machinery; the queue panel now shows progress.

**Files:**
- Modify: `src/components/file-browser.ts`

- [ ] **Step 1: Remove the single-bar machinery**

Delete from `file-browser.ts`:
- The `transferProgress` field (lines ~163-177) and its uses.
- Methods `startTransfer`, `updateTransfer`, `updateFromChannel`, `_flushTransferDom`, `endTransfer` (~923-1010).
- The `makeProgressChannel` helper (~28-46) and the `Channel`/`TransferProgress` imports.
- The in-`render()` `transferProgressHtml` block (~522-540, 582) and the `#transfer-cancel-btn` listener (~863-880).
- In `disconnect()` (~1184-1201), remove the `this.transferProgress` references; replace the "cancel in-flight transfer" call with nothing (the queue persists across browser disconnect by design — do NOT cancel all transfers on a browser disconnect, since other transfers may be unrelated). If a per-profile cancel-on-disconnect is desired, leave a TODO comment — out of scope for this task.

Keep `isCancelErr` / the cancel sentinel recognition only if still referenced; otherwise remove.

- [ ] **Step 2: Add an enqueue helper**

Add a private helper to `FileBrowser`:

```typescript
  /** Enqueue a single transfer job in the background queue. */
  private async enqueue(
    kind: TransferJobView["kind"],
    src: string,
    dst: string,
    filename: string
  ): Promise<void> {
    if (!this.profileId) return;
    await api.enqueueTransfer(this.profileId, kind, src, dst, filename);
  }
```

Import `TransferJobView` from `../types`.

- [ ] **Step 3: Rewrite `uploadFileList`**

Replace the body with: resolve overwrite per file (unchanged), then enqueue instead of awaiting. No per-file progress loop; the queue panel shows progress. Example:

```typescript
  async uploadFileList(localPaths: string[]): Promise<void> {
    if (!this.profileId) return;
    let queued = 0;
    let skipped = 0;
    for (const localFilePath of localPaths) {
      const filename = localFilePath.replace(/\\/g, "/").split("/").pop() ?? localFilePath;
      const remotePath = joinPath(this.currentPath, filename);
      try {
        const proceed = await this.resolveOverwrite(remotePath, filename);
        if (!proceed) { skipped++; continue; }
      } catch (err) {
        if (String(err) === "Error: UPLOAD_CANCELLED") break;
      }
      await this.enqueue("upload", localFilePath, remotePath, filename);
      queued++;
    }
    const parts: string[] = [];
    if (queued > 0) parts.push(t("fileBrowser.queuedCount", { count: queued }));
    if (skipped > 0) parts.push(t("fileBrowser.skippedCount", { count: skipped }));
    if (parts.length) this.status(parts.join(", "), false);
  }
```

- [ ] **Step 4: Rewrite `uploadPathList`**

Same pattern, but `kind` depends on whether the local path is a directory. Since the frontend cannot stat the local path type directly, use the existing convention: `uploadPathList` is fed local paths that may be files or folders. Add a backend-agnostic decision: enqueue kind `"uploadDir"` if the path has no file extension *and* — better: add a small helper that asks the backend. **Simpler and correct:** keep a single `"upload"`-vs-`"uploadDir"` decision by checking `await api.localFileExists(path)` is insufficient for type. Use the existing local browser's knowledge if available; otherwise enqueue kind based on a new lightweight check.

Decision for this task: route folder-or-file uploads through a new helper `enqueueLocalPath(localPath, remotePath, name)` that calls a backend `local_path_is_dir` check. **Add that command** in this task:
- Backend: in `commands/sftp.rs` add `#[tauri::command] pub fn local_path_is_dir(path: String) -> bool { std::path::Path::new(&path).is_dir() }`, register in `lib.rs`, add `localPathIsDir` API wrapper.
- Frontend helper:

```typescript
  private async enqueueLocalPath(localPath: string, remotePath: string, name: string): Promise<void> {
    const isDir = await api.localPathIsDir(localPath);
    await this.enqueue(isDir ? "uploadDir" : "upload", localPath, remotePath, name);
  }
```

Use it in `uploadPathList` and folder-drop flows. Rewrite `uploadPathList` like `uploadFileList` but calling `enqueueLocalPath`.

- [ ] **Step 5: Rewrite `handleUploadFolder`**

Resolve overwrite (unchanged), then `await this.enqueue("uploadDir", localFolderPath, remotePath, folderName);` and show a "queued" status. Remove `startTransfer`/`endTransfer`/Channel.

- [ ] **Step 6: Rewrite download flows**

For `handleDownload` (single file), `handleDownloadMulti`, folder download, and drag-to-local: resolve the local destination path exactly as today (configured `local_path` or save dialog → `dst`), then enqueue `"download"` (file) or `"downloadDir"` (folder) with `src` = remote path, `dst` = local path. Remove the Channel and single-bar usage. Keep the destination-resolution logic (save dialog, local_path, overwrite-of-local prompt via `local_file_exists`) — only the transfer execution changes from "await api.downloadX(channel)" to "await this.enqueue(...)".

- [ ] **Step 7: Compile the whole frontend and regenerate .js**

Run: `npx tsc 2>&1 | head -40`
Expected: clean (0 errors). This regenerates `.js` siblings for every changed `.ts`.

- [ ] **Step 8: Revert unrelated regenerated .js**

Run: `git status --porcelain | grep '\.js$'`
Inspect with `git diff` — revert any `.js` whose `.ts` you did not change (esp. `src/i18n/index.js` whitespace noise): `git checkout -- <file>`.

- [ ] **Step 9: Commit (bundles Tasks 5, 6, 7)**

```bash
git add src/types.ts src/types.js src/api/index.ts src/api/index.js \
        src/components/transfer-queue.ts src/components/transfer-queue.js \
        src/components/file-browser.ts src/components/file-browser.js \
        src-tauri/src/commands/sftp.rs src-tauri/src/lib.rs index.html <stylesheet>
git commit -m "feat(transfer): queue panel UI; route file-browser uploads/downloads through background queue"
```

---

## Task 8: Wire the panel in main.ts + refresh-on-finish

**Files:**
- Modify: `src/main.ts`

- [ ] **Step 1: Instantiate the panel and connect refresh**

In `src/main.ts`, after the file browser is created, add:

```typescript
import { TransferQueuePanel } from "./components/transfer-queue";

const transferQueue = new TransferQueuePanel("transfer-queue");
transferQueue.setOnJobFinished((job) => {
  if (job.state !== "done") return;
  // Refresh the remote browser after an upload into the current dir;
  // refresh the local browser after a download.
  if (job.kind === "upload" || job.kind === "uploadDir") {
    fileBrowser.refresh().catch(() => {});
  } else {
    localBrowser?.refresh?.();
  }
});
```

(Match the actual variable names for `fileBrowser` / `localBrowser` in `main.ts`. If `refresh` is private on `FileBrowser`, add a public `refreshIfConnected()` or make `refresh` callable — inspect and use the cleanest existing public method.)

- [ ] **Step 2: Compile + revert noise**

Run: `npx tsc 2>&1 | head -20` then revert unrelated `.js`.

- [ ] **Step 3: Commit**

```bash
git add src/main.ts src/main.js
git commit -m "feat(transfer): mount queue panel and refresh browsers on job completion"
```

---

## Task 9: Settings UI for concurrency

**Files:**
- Modify: `src/components/settings-dialog.ts`

- [ ] **Step 1: Add a number input**

In `settings-dialog.ts`, add a labelled `<input type="number" min="1" max="8">` bound to `max_concurrent_transfers`. On load, populate from settings (default 2). On save, parse, clamp 1..=8, and include it in the settings object passed to `api.saveSettings`. Use the existing read-merge-write pattern (the settings save bug fix from Phase 5.5 — read current settings, merge the field, write).

- [ ] **Step 2: Compile + revert noise + commit**

```bash
npx tsc 2>&1 | head -20
# revert unrelated .js
git add src/components/settings-dialog.ts src/components/settings-dialog.js
git commit -m "feat(settings): configurable max concurrent transfers (1-8)"
```

---

## Task 10: i18n strings (all 6 locales)

**Files:**
- Modify: `src/i18n/{en,de,fr,nl,pl,ru}.ts` (+ regenerate `.js`)

- [ ] **Step 1: Add keys to every locale**

Add a `transferQueue` section and the new `fileBrowser.queuedCount` key. English values:

```
transferQueue.title = "Transfers"
transferQueue.cancel = "Cancel"
transferQueue.cancelAll = "Cancel all"
transferQueue.clearFinished = "Clear finished"
transferQueue.state_queued = "Queued"
transferQueue.state_active = "Transferring"
transferQueue.state_done = "Done"
transferQueue.state_failed = "Failed"
transferQueue.state_cancelled = "Cancelled"
fileBrowser.queuedCount = "{count} queued"
settings.maxConcurrentTransfers = "Concurrent transfers"
settings.maxConcurrentTransfersHint = "How many uploads/downloads run at once (1–8)"
```

German values:

```
transferQueue.title = "Übertragungen"
transferQueue.cancel = "Abbrechen"
transferQueue.cancelAll = "Alle abbrechen"
transferQueue.clearFinished = "Abgeschlossene entfernen"
transferQueue.state_queued = "Warteschlange"
transferQueue.state_active = "Überträgt"
transferQueue.state_done = "Fertig"
transferQueue.state_failed = "Fehlgeschlagen"
transferQueue.state_cancelled = "Abgebrochen"
fileBrowser.queuedCount = "{count} eingereiht"
settings.maxConcurrentTransfers = "Gleichzeitige Übertragungen"
settings.maxConcurrentTransfersHint = "Wie viele Uploads/Downloads gleichzeitig laufen (1–8)"
```

For fr/nl/pl/ru: provide translations consistent with the existing translated keys in those files (translate, do not leave English) — match the tone/format of neighbouring keys. Keep the exact key paths/placeholders (`{count}`).

- [ ] **Step 2: Regenerate, revert noise, compile**

Run: `npx tsc 2>&1 | head -20`
Then revert any `.js` not corresponding to the locale files you edited.

- [ ] **Step 3: Commit**

```bash
git add src/i18n/en.ts src/i18n/en.js src/i18n/de.ts src/i18n/de.js \
        src/i18n/fr.ts src/i18n/fr.js src/i18n/nl.ts src/i18n/nl.js \
        src/i18n/pl.ts src/i18n/pl.js src/i18n/ru.ts src/i18n/ru.js
git commit -m "i18n(transfer): queue panel + concurrency setting strings (6 locales)"
```

---

## Task 11: Full build + Phase 2 review

**Files:** none (verification)

- [ ] **Step 1: Backend build + tests**

Run: `cd src-tauri && cargo build 2>&1 | tail -20 && cargo test --lib 2>&1 | tail -15 && cargo clippy 2>&1 | tail -20`
Expected: clean build, all tests pass, no clippy warnings.

- [ ] **Step 2: Frontend build**

Run: `npm run build 2>&1 | tail -25`
Expected: `tsc && vite build` succeed.

- [ ] **Step 3: Confirm git scope**

Run: `git status --porcelain` — only intended files changed; no stray `.js`.

- [ ] **Step 4: Manual verification checklist (real server — by the user)**

1. Queue 10+ files: UI stays responsive; up to `max_concurrent_transfers` run at once; the rest show "Queued" and start as slots free.
2. Per-job ✕ cancels just that job (others continue); "Cancel all" stops everything; cancelled jobs show "Cancelled", not "Failed".
3. A failing transfer (e.g. permission denied) shows "Failed" with its error and does not abort sibling jobs.
4. Folder upload/download appears as a single job with the current file name updating.
5. Overwrite prompt still appears before enqueue (Yes/No/Cancel + apply-to-all).
6. On completion, the remote browser refreshes after uploads; local after downloads.
7. Closing the app via the OS X with active transfers tears down cleanly (no orphan threads/sockets) — `cancel_all()` runs on exit.
8. Changing "Concurrent transfers" in Settings changes how many run simultaneously on the next batch.

- [ ] **Step 5: Dispatch the Phase 2 review subagent (Opus)** — spec compliance + code quality + responsiveness/perf, per subagent-driven-development.

---

## Task 12: Docs + release (v1.5.0)

This is a feature release (new transfer engine) → minor bump **1.4.8 → 1.5.0**.

**Files:**
- Modify: `README.md`, `CHANGELOG.md`, `CLAUDE.md`, `vault/MurmurSSH Optimization Roadmap.md`
- Modify: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`

- [ ] **Step 1: CHANGELOG** — add a `## [1.5.0]` section: background transfer queue with configurable concurrency (1–8), per-job + cancel-all controls, queue panel, parallel multi-connection transfers; note that the single inline progress bar was replaced by the queue panel.

- [ ] **Step 2: README** — update the "Real-time transfer progress" bullet to describe the queue panel + concurrency setting; add a Settings note for "Concurrent transfers"; update the file-browser table if transfer rows changed.

- [ ] **Step 3: CLAUDE.md** — add a Phase 11.0 (v1.5.0) entry under "Phases Complete" summarizing the queue architecture (dispatcher + worker pool, per-job cancel closure refactor, `transfer_cancel` removed, queue events).

- [ ] **Step 4: Vault** — mark Phase 2 done in `vault/MurmurSSH Optimization Roadmap.md` with the manual-verification checklist.

- [ ] **Step 5: Version bump** — set `1.5.0` in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.

- [ ] **Step 6: Commit, tag, push**

```bash
git add -A
git commit -m "release: v1.5.0 — background transfer queue with configurable concurrency"
git tag -a v1.5.0 -m "v1.5.0 — transfer queue + multi-connection"
git push origin main --follow-tags
```

- [ ] **Step 7: Refresh gitnexus index**

Run: `npx gitnexus analyze` (check `.gitnexus/meta.json` `stats.embeddings`; add `--embeddings` if > 0).

---

## Self-review notes (author)

- **Spec coverage:** T1 (queue service) → Tasks 1–3; T2 (commands + API + routing) → Tasks 4,5,7; T3 (panel UI + concurrency setting) → Tasks 6,9; close-handler drain → Task 4 Step 4; review → Task 11; docs → Task 12. All covered.
- **Type consistency:** `TransferKind` strings (`upload`/`download`/`uploadDir`/`downloadDir`) and `TransferState` strings (`queued`/`active`/`done`/`failed`/`cancelled`) match between Rust serde (`rename_all = camelCase`) and the TS union types and the i18n `state_*` keys.
- **Cancellation:** per-job `Arc<AtomicBool>` replaces profile-keyed flag; `CANCELLED_ERROR` sentinel preserved in `models/transfer.rs` so frontend cancel recognition survives; workspace edit-flow passes `&|| false`.
- **Risk note for reviewer:** the `transfer_queue` worker/dispatcher threading is not unit-tested (real network); only pure logic + state transitions are. Manual verification (Task 11 Step 4) is the gate for the threaded/networked behaviour. Flag if a deterministic injectable-runner test harness is warranted.
- **Out of scope (unchanged):** `upload_file_bytes` (New File), workspace remote-edit uploads stay direct; no backend overwrite dialog (kept frontend-side).
