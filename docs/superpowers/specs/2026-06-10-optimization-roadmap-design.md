# MurmurSSH Optimization Roadmap — Design Spec

**Date:** 2026-06-10
**Status:** Approved (brainstorming complete) → ready for implementation planning
**Guiding principle:** Production-ready at every step. Each logical change is committed; README + CHANGELOG updated; version bumped + tagged + pushed at phase boundaries; gitnexus index and vault/Obsidian tracking note kept current.

---

## 1. Goal

Deliver a set of feature optimizations and reliability fixes for MurmurSSH (Tauri 2 + Rust SSH/SFTP client), executed in dependency-ordered phases via subagent-driven development, with per-task model assignment for token efficiency.

## 2. Locked decisions (from brainstorming)

| Topic | Decision |
|---|---|
| Transfer architecture | **In-app background queue** with configurable concurrency (FileZilla-style parallel transfers). No separate sidecar process. |
| Permission UI | **rwx checkbox grid (User/Group/Other) synced to an octal text field** — both update each other. |
| Profile grouping | **`group` field + collapsible grouped tree view**; per-group sort: alphabetical or creation date (new `created_at` field). |
| Editor config | **Global default editor + per-file-extension map** in Settings; per-profile `editor_command` still overrides. |
| "Creation date" in file list | Not available over SFTP v3 (`ssh2` exposes only `mtime`/`atime`). Implemented as a **modification-date column**. |
| FileZilla import credentials | **Saved passwords are NOT imported** by default; credentials prompted at connect. Only host/port/user/name/group/protocol imported. |

## 3. Architectural constraints (from codebase map)

- `FileEntry` (`src-tauri/src/models/sftp.rs`) has only `name, is_dir, size, modified`. `stat.perm` is read internally for symlink detection but never surfaced.
- SFTP ops open a **fresh TCP+SSH+SFTP session per call**; no pool. Transfers run on Tauri command threads (not main) but block per-call; large/many transfers degrade responsiveness.
- `transfer_cancel.rs` already provides per-`profile_id` cancellation; progress flows via a Tauri `Channel<TransferProgress>`.
- `workspace_service.rs`: `open_for_edit` skips re-download when `is_watched()` is true; watcher uses **mtime-only** comparison (no content hash). This is the source of the three edit-flow bugs.
- No Tauri window-close hook in `lib.rs`; cleanup only runs on explicit Disconnect / Quit button.
- `Profile` (`models/profile.rs`) is a flat list, sorted alphabetically by name in `profile_service::list_profiles`. No group/created_at.
- `ProfileSelector` renders a plain `<select>` dropdown.
- `Settings` (`models/settings.rs`) has `last_used_profile_id, profiles_path, theme, local_browser_position`. No editor or concurrency fields.
- Two distinct context-menu implementations: `FileBrowser` (typed `CtxMenuItem[]`) and `LocalFileBrowser` (innerHTML + `data-action`).
- i18n: each language needs `<lang>.js` + `<lang>.ts`; current locales: en, de, fr, nl, pl, ru. All new user-facing strings must be added to **all** locales (en/de authored, others at minimum English-fallback or translated).

---

## 4. Phases

### Phase 0 — Bug fixes (ship first, lowest risk)

**B1. Edit / re-save flow** (`workspace_service.rs`, `main.ts`, `file-browser.ts`)
- **Actual root cause (corrected after reading the code):** `open_for_edit` already re-downloads unconditionally on every Edit click (workspace_service.rs:93-100). That re-download **rewrites the local cache file, bumping its mtime**. The watcher thread from the first edit (still alive) sees the mtime change and fires `upload-ready` even though the user made no edits — this is the "spurious upload prompt". Multi-write editor saves (temp+rename) bump mtime twice → "double confirmation". The mtime-only comparison (watcher `last_mtime`) is the flaw.
- *Fix:* replace mtime-only detection with a **content-hash baseline registry** shared between `open_for_edit` and the watcher: a `OnceLock<Mutex<HashMap<PathBuf,String>>>` mapping local path → last-accounted-for content hash. `open_for_edit` sets the baseline after each download; the watcher only emits/uploads when the current file hash **differs** from the baseline, then updates the baseline. This makes re-downloads invisible to the watcher and dedups multi-write saves to a single upload.
- *Cannot re-edit after save:* a symptom of the confusing prompts; once hash-baseline logic lands the open→edit→save→re-edit cycle is clean. Verify manually.
- **Acceptance:** open→edit→save→(auto/confirm upload once)→close editor→re-click Edit opens fresh remote content; opening without editing produces **no** upload prompt; a single save produces exactly **one** confirmation/upload.

**B2. Connection cleanup on window close** (`lib.rs`, ssh/runtime-key services)
- Register a Tauri `on_window_event` handler for `WindowEvent::CloseRequested` (and/or `RunEvent::ExitRequested`) that runs the same teardown as Disconnect: stop ControlMaster session(s), delete runtime key copies, clear session credentials.
- **Acceptance:** closing via OS title-bar X leaves no leftover ControlMaster sockets, no runtime key files in the secrets/runtime dir, and session credential store cleared. Verify with a manual check of the runtime key dir + `ssh -O check` behaviour notes.

### Phase 1 — File list & permissions

**F1. Modification-date column** (`file-browser.ts`, possibly `local-file-browser.ts`)
- Surface `FileEntry.modified` as a formatted date/time column (locale-aware). Sortable optional (nice-to-have, not required).

**F2. Permission bits in model** (`models/sftp.rs`, `sftp_service.rs`)
- Add `perm: Option<u32>` to `FileEntry`; populate from `stat.perm` in `list_directory`. Keep existing symlink-follow logic.
- Surface a **permissions column** rendering both symbolic (`rwxr-xr-x`) and/or octal (`0755`).

**F3. chmod via context menu** (new dialog component, `connection.rs`/`sftp.rs` command, `sftp_service.rs`)
- New backend command `set_permissions(profile_id, remote_path, mode: u32)` → opens session → `sftp.setstat(path, FileStat{ perm: Some(mode), ..default })`.
- New dialog: **rwx checkbox grid (owner/group/other × r/w/x) bidirectionally synced with an octal text field**; live preview; validation (octal 0–7 per digit). Wire a "Permissions…" / "Rechte ändern…" entry into the `FileBrowser` file + folder context menus.
- **Acceptance:** changing checkboxes updates the octal field and vice-versa; applying sets the mode on the remote; the permissions column reflects the new value after refresh; invalid octal rejected.

### Phase 2 — Transfer background queue + multi-connection (largest phase)

**T1. `transfer_queue` service** (new `src-tauri/src/services/transfer_queue.rs`)
- Persistent worker pool; `max_concurrent_transfers` read from Settings (default 2). Job model: `{ id, profile_id, kind(Up/Down/UpDir/DownDir), src, dst, state(Queued/Active/Done/Failed/Cancelled), progress, error }`.
- Workers each open their own SFTP session (multi-connection parallelism). Reuse existing `transfer_cancel` for per-job cancel; emit job lifecycle + progress events to the frontend.
- Graceful shutdown integrates with Phase 0 B2 close handler (drain/cancel active jobs).

**T2. Commands + API** (`commands/`, `src/api/index.ts`)
- `enqueue_transfer`, `cancel_transfer(job_id)`, `cancel_all_transfers`, `list_transfers`. Existing upload/download entry points route through the queue.

**T3. Queue/transfer panel UI** (new component or extend transfer-progress UI)
- Shows queued/active/done/failed jobs with per-job progress, speed, and cancel; cancel-all; auto-clear completed (optional). Settings UI control for concurrency count.
- **Acceptance:** queueing many/large files keeps the UI responsive (no "not responding"); N transfers run concurrently per the setting; per-job and cancel-all work; failures surface per-job without aborting the batch.

### Phase 3 — Profile groups & sorting

**P1. Model** (`models/profile.rs`, `profile_service.rs`)
- Add `group: Option<String>` and `created_at: Option<u64>` (epoch secs). Write `created_at` at creation; for existing profiles fall back to the JSON file's mtime. Backward-compatible deserialization (serde defaults).

**P2. Selector tree view** (`profile-selector.ts`, `profile-form.ts`)
- Replace `<select>` with a collapsible grouped tree (group headers + profiles). Group field editable in the profile form (free-text or datalist of existing groups). Per-group sort toggle: **alphabetical | creation date**, persisted in Settings.
- **Acceptance:** profiles render under their group; ungrouped profiles in a default group; collapse/expand persists per session; sort toggle works; connect/edit/delete still gated by connection state.

### Phase 4 — FileZilla import (depends on Phase 3)

**I1. Importer** (new service + command, new UI entry, e.g. Settings or profile-selector button)
- Parse `~/.config/filezilla/sitemanager.xml` (allow custom path picker). Map `<Folder>` nesting → `group` (flattened path, e.g. `Parent/Child`), `<Server>` → Profile (host/port/user/name/protocol). **Do not import saved passwords**; `credential_storage_mode = Never`.
- Conflict handling: skip or suffix duplicates by id; report a summary (imported / skipped).
- **Acceptance:** importing a real FileZilla sitemanager.xml creates profiles grouped by their FileZilla folders; no credentials persisted; summary shown; malformed XML handled gracefully.

### Phase 5 — Editor configuration

**E1. Settings model + resolution** (`models/settings.rs`, `workspace_service.rs::open_in_editor`)
- Add `default_editor: Option<String>` and `editor_by_extension: HashMap<String,String>` to Settings. Resolution order in `open_in_editor`: per-profile `editor_command` → per-extension map (by lowercased extension) → global default → `xdg-open`.

**E2. Settings UI** (`settings-dialog.ts`)
- Fields for global default editor + an editable extension→editor list (add/remove rows). Used by both remote edit and local-browser edit.
- **Acceptance:** a `.conf` file opens in the configured per-extension editor; profile override still wins; unmapped types fall back to global then xdg-open.

---

## 5. Per-phase subagent task breakdown & model assignment

Legend — **H**=Haiku (mechanical/cheap), **S**=Sonnet (standard implementation), **O**=Opus (architecture / root-cause / integration & review).

### Phase 0
| Task | Layer | Model |
|---|---|---|
| B1 root-cause confirm + fix watcher (hash + re-download + dedup) | Rust + TS, shared state | **O** |
| B1 frontend upload-ready/edit wiring adjustments | TS | S |
| B2 window-close cleanup handler | Rust (lib.rs + services) | S |
| Phase 0 review + manual verification notes | cross-layer | O |
| README/CHANGELOG entries | docs | H |

### Phase 1
| Task | Layer | Model |
|---|---|---|
| F1 mtime column render (remote + local) | TS | S |
| F2 `perm` in FileEntry + populate in list_directory | Rust | S |
| F2 permissions column render (symbolic+octal) | TS | S |
| F3 `set_permissions` command + service | Rust (commands + sftp_service) | S |
| F3 chmod dialog (grid↔octal sync) + context-menu wiring | TS | S |
| i18n strings (all locales) | TS | H |
| Phase 1 review | cross-layer | O |
| README/CHANGELOG | docs | H |

### Phase 2 (largest — Opus-led)
| Task | Layer | Model |
|---|---|---|
| T1 transfer_queue architecture + worker pool + job model | Rust (new service, shared state) | **O** |
| T2 commands + API wrappers + route existing ops through queue | Rust + TS | S |
| T3 queue panel UI + concurrency setting | TS | S |
| Integrate close-handler drain (ties to B2) | Rust | S |
| Phase 2 review + responsiveness/perf verification | cross-layer | O |
| README/CHANGELOG | docs | H |

### Phase 3
| Task | Layer | Model |
|---|---|---|
| P1 Profile group/created_at fields + backward-compat load/sort | Rust | S |
| P2 grouped tree-view selector + form group field + sort toggle | TS | S |
| i18n strings | TS | H |
| Phase 3 review | cross-layer | O |
| README/CHANGELOG | docs | H |

### Phase 4
| Task | Layer | Model |
|---|---|---|
| I1 FileZilla XML parser + import service + command | Rust | S |
| I1 import UI entry + summary dialog | TS | S |
| i18n strings | TS | H |
| Phase 4 security/correctness review (no creds, path safety) | cross-layer | O |
| README/CHANGELOG | docs | H |

### Phase 5
| Task | Layer | Model |
|---|---|---|
| E1 Settings fields + open_in_editor resolution | Rust | S |
| E2 settings-dialog editor UI | TS | S |
| i18n strings | TS | H |
| Phase 5 review | cross-layer | O |
| README/CHANGELOG | docs | H |

## 6. Definition of done (per phase)

1. `cargo check` (and build where relevant) clean; TypeScript compiles; no new warnings left unresolved.
2. Acceptance criteria for the phase met and manually verified where UI/behavior is involved.
3. README + CHANGELOG updated for user-facing changes.
4. Each logical step committed separately with clear messages.
5. At phase boundary: version bump across `Cargo.toml` / `tauri.conf.json` / `package.json`, annotated git tag, push to GitHub main.
6. gitnexus index refreshed; vault/Obsidian tracking note updated.

## 7. Out of scope (YAGNI)

- Separate sidecar transfer process (revisit only if the in-app queue proves insufficient).
- True file creation/birth time (not available over SFTP v3).
- chown / ownership editing (only chmod/permissions).
- Importing FileZilla saved passwords (intentionally excluded for security).
- Nested multi-level group hierarchy beyond the single `group` field (flattened path strings only).
