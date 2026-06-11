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
    clamp_concurrency, Protocol, TransferJobView, TransferKind, TransferState, CANCELLED_ERROR,
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

/// Store the app handle and start the dispatcher thread (call once at startup).
pub fn init(app: &AppHandle) {
    if let Ok(mut guard) = app_handle().lock() {
        *guard = Some(app.clone());
    }
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_ok() {
        std::thread::spawn(dispatcher_loop);
    }
}

/// Read the current concurrency from settings, clamped to the supported range.
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
    st.jobs
        .retain(|j| matches!(j.view.state, TransferState::Queued | TransferState::Active));
}

/// Dispatcher: promote Queued -> Active up to the concurrency limit, spawning a
/// worker per promoted job. Blocks on the condvar otherwise.
fn dispatcher_loop() {
    loop {
        let mut st = queue().state.lock().unwrap();
        loop {
            let limit = current_concurrency();
            if st.active >= limit {
                break;
            }
            let idx = st
                .jobs
                .iter()
                .position(|j| j.view.state == TransferState::Queued);
            let Some(idx) = idx else { break };
            st.jobs[idx].view.state = TransferState::Active;
            st.active += 1;
            let view = st.jobs[idx].view.clone();
            let cancel = st.jobs[idx].cancel.clone();
            emit(&view);
            std::thread::spawn(move || run_worker(view, cancel));
        }
        let _unused = queue().cv.wait(st).unwrap();
    }
}

/// Update a job's view in place. Returns the updated snapshot if the job exists.
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

    let final_view = with_job(job_id, |v| match &result {
        Ok(()) => {
            v.state = TransferState::Done;
            if v.bytes_total > 0 {
                v.bytes_done = v.bytes_total;
            }
        }
        Err(e) if e == CANCELLED_ERROR => {
            v.state = TransferState::Cancelled;
        }
        Err(e) => {
            v.state = TransferState::Failed;
            v.error = Some(e.clone());
        }
    });
    if let Some(v) = final_view {
        emit(&v);
    }

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
    let is_ftp = profile.protocol.as_ref() == Some(&Protocol::Ftp);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        let mut st = queue().state.lock().unwrap();
        st.jobs.clear();
        st.active = 0;
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
        cancel(a);
        clear_finished();
        let ids: Vec<u64> = list().iter().map(|j| j.id).collect();
        assert!(!ids.contains(&a));
        assert!(ids.contains(&b));
    }
}
