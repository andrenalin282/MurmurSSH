# Phase 0 — Bug Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the remote-edit upload-prompt bugs (spurious + double upload confirmations after re-opening a file) and ensure SSH/runtime-key/credential cleanup runs when the app window is closed via the OS title bar.

**Architecture:** B1 introduces a content-hash baseline registry in `workspace_service.rs` shared between `open_for_edit` (sets the baseline after every download) and the watcher thread (acts only when the current hash differs from the baseline). This makes re-downloads invisible to the watcher and collapses multi-write editor saves into one upload. B2 registers a Tauri window `CloseRequested`/`ExitRequested` handler in `lib.rs` that invokes the same teardown the explicit Disconnect path uses.

**Tech Stack:** Rust (Tauri 2, `notify`, `std::collections::hash_map::DefaultHasher`), TypeScript frontend.

---

## File Structure

- `src-tauri/src/services/workspace_service.rs` — **modify**. Add `file_content_hash()` pure helper, a `baselines()` registry (`OnceLock<Mutex<HashMap<PathBuf,String>>>`) with `set_baseline`/`get_baseline`/`clear_baseline`, set the baseline after download in `open_for_edit`, and switch the watcher from mtime to hash comparison.
- `src-tauri/src/services/workspace_service.rs` tests — **add** an inline `#[cfg(test)] mod tests` for the pure helpers.
- `src-tauri/src/lib.rs` — **modify**. Add `.on_window_event(...)` (and `.build()`+`run` with `RunEvent::ExitRequested`) to run cleanup on close.
- `src-tauri/src/services/mod.rs` — **inspect only**, to confirm the exact public cleanup entry points (`ssh_session_service`, `runtime_key_service`, `credentials_store`).
- `README.md`, `CHANGELOG.md` — **modify**. Document the fixes.

Note: the watcher/inotify/Tauri-event integration is not unit-testable in this harness; TDD covers the pure hash + baseline-registry logic, and the integration is verified manually per the acceptance checklist at the end.

---

## Task 1: Content-hash helper (TDD)

**Files:**
- Modify: `src-tauri/src/services/workspace_service.rs`
- Test: inline `#[cfg(test)] mod tests` in the same file

- [ ] **Step 1: Write the failing test**

Add at the bottom of `src-tauri/src/services/workspace_service.rs`:

```rust
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
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test workspace_service::tests::hash 2>&1 | tail -20`
Expected: FAIL — `cannot find function file_content_hash in this scope`.

- [ ] **Step 3: Write minimal implementation**

Add near the top of `workspace_service.rs` (after the existing `use` lines, add the hasher imports), and add the function below `local_cache_path`:

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
```

```rust
/// Computes a fast content hash of a file. Returns None if the file can't be read.
/// Used to detect *real* edits and ignore re-downloads / duplicate save events.
fn file_content_hash(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(format!("{:016x}", hasher.finish()))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test workspace_service::tests::hash 2>&1 | tail -20`
Expected: PASS (3 passed).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/workspace_service.rs
git commit -m "feat(workspace): add file_content_hash helper for edit-flow change detection"
```

---

## Task 2: Baseline registry (TDD)

**Files:**
- Modify: `src-tauri/src/services/workspace_service.rs`
- Test: inline `#[cfg(test)] mod tests` (same module as Task 1)

- [ ] **Step 1: Write the failing test**

Append inside the existing `mod tests`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test workspace_service::tests::baseline 2>&1 | tail -20`
Expected: FAIL — `cannot find function set_baseline`.

- [ ] **Step 3: Write minimal implementation**

Add below the `active_watchers()` block in `workspace_service.rs`:

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test workspace_service::tests::baseline 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/workspace_service.rs
git commit -m "feat(workspace): add content-hash baseline registry"
```

---

## Task 3: Set baseline after download in `open_for_edit`

**Files:**
- Modify: `src-tauri/src/services/workspace_service.rs:124-125` (the area just before `open_in_editor`)

- [ ] **Step 1: Implement**

In `open_for_edit`, immediately AFTER the binary-check block and BEFORE `// Open in editor` / `open_in_editor(profile, &local_path)?;`, insert:

```rust
    // Record the just-downloaded content as the baseline so the watcher does not
    // mistake this (re-)download for a user edit.
    if let Some(h) = file_content_hash(&local_path) {
        set_baseline(&local_path, h);
    }
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -15`
Expected: Finished, no errors (a `dead_code` warning for `get_baseline`/`clear_baseline` is acceptable here — resolved in Task 4).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/services/workspace_service.rs
git commit -m "fix(workspace): set hash baseline after download so re-downloads don't trigger upload"
```

---

## Task 4: Switch watcher from mtime to hash comparison

**Files:**
- Modify: `src-tauri/src/services/workspace_service.rs:199-262` (`watch_and_upload`)

- [ ] **Step 1: Replace the mtime baseline init**

Replace these lines (currently ~199-201):

```rust
    let mut last_mtime = std::fs::metadata(&local_path)
        .and_then(|m| m.modified())
        .ok();
```

with:

```rust
    // Seed the baseline from the file as it exists now (open_for_edit normally
    // sets it already, but this guards the watch-before-download race).
    if get_baseline(&local_path).is_none() {
        if let Some(h) = file_content_hash(&local_path) {
            set_baseline(&local_path, h);
        }
    }
```

- [ ] **Step 2: Replace the mtime change-check inside the Modify/Create arm**

Replace this block (currently ~219-226):

```rust
                let current_mtime = std::fs::metadata(&local_path)
                    .and_then(|m| m.modified())
                    .ok();

                if current_mtime == last_mtime {
                    continue;
                }
                last_mtime = current_mtime;
```

with:

```rust
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
```

- [ ] **Step 3: Clear the baseline on watcher exit**

Replace the final cleanup (currently ~260-261):

```rust
    // Cleanup: remove from active watchers registry
    unregister_watcher(&local_path);
```

with:

```rust
    // Cleanup: remove from active watchers registry and drop the baseline
    unregister_watcher(&local_path);
    clear_baseline(&local_path);
```

- [ ] **Step 4: Verify it compiles cleanly (no dead-code warnings now)**

Run: `cd src-tauri && cargo check 2>&1 | tail -15`
Expected: Finished, no errors, no `dead_code` warnings for the baseline functions.

- [ ] **Step 5: Run the full workspace test module**

Run: `cd src-tauri && cargo test workspace_service::tests 2>&1 | tail -20`
Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/workspace_service.rs
git commit -m "fix(workspace): use content hash (not mtime) for edit change detection

Fixes spurious upload prompt after re-opening a file (re-download no
longer looks like an edit) and double upload confirmation from
multi-write editor saves."
```

---

## Task 5: Cleanup on window close (B2)

Verified entry points: `runtime_key_service::cleanup_all_runtime_keys()` already exists (no-arg). `ssh_session_service::stop_session(profile_id)` and `credentials_store::clear(profile_id)` are per-profile only — this task adds `stop_all_sessions()` and `clear_all()` wrappers, then wires the exit handler.

**Files:**
- Modify: `src-tauri/src/services/ssh_session_service.rs` (add `stop_all_sessions`)
- Modify: `src-tauri/src/services/credentials_store.rs` (add `clear_all`)
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `stop_all_sessions()` to ssh_session_service.rs**

Immediately after the existing `stop_session` function (ends at line ~128), add:

```rust
/// Stop and clean up ALL SSH sessions (called on app exit). Mirrors
/// `stop_session` but drains the whole registry.
pub fn stop_all_sessions() {
    let Ok(mut map) = sessions().lock() else { return; };
    for (_id, mut session) in map.drain() {
        let _ = session.child.kill();
        let _ = session.child.wait(); // reap to avoid zombies
        let SessionKind::ControlMaster { socket_path } = &session.kind;
        let _ = fs::remove_file(socket_path);
    }
}
```

- [ ] **Step 2: Add `clear_all()` to credentials_store.rs**

After the existing `clear` function (ends at line ~65), add:

```rust
/// Remove ALL runtime credentials (called on app exit).
pub fn clear_all() {
    if let Ok(mut map) = store().lock() {
        map.clear();
    }
}
```

- [ ] **Step 3: Add the exit cleanup function + wire the handler in lib.rs**

In `lib.rs`, add this free function above `pub fn run()`:

```rust
/// Best-effort teardown run on app exit: stop SSH control sessions, remove any
/// runtime key copies, and clear in-memory session credentials. Mirrors the
/// explicit Disconnect path so closing via the OS title bar leaves no residue.
fn cleanup_on_exit() {
    services::ssh_session_service::stop_all_sessions();
    services::runtime_key_service::cleanup_all_runtime_keys();
    services::credentials_store::clear_all();
}
```

Then replace the builder tail. Change:

```rust
        .run(tauri::generate_context!())
        .expect("error while running MurmurSSH");
```

to:

```rust
        .build(tauri::generate_context!())
        .expect("error while building MurmurSSH")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                cleanup_on_exit();
            }
        });
```

> Note: `services` items must be reachable from `lib.rs`. The modules are declared `mod services;` at the top; ensure `ssh_session_service`, `runtime_key_service`, and `credentials_store` are `pub` (or at least `pub(crate)`) in `services/mod.rs`. If a privacy error occurs in Step 4, widen their visibility in `services/mod.rs` and re-run.

- [ ] **Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -25`
Expected: Finished, no errors. Fix any visibility error per the note above and re-run.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/services/ssh_session_service.rs src-tauri/src/services/credentials_store.rs
git commit -m "fix(app): run SSH/runtime-key/credential cleanup on window close (ExitRequested)"
```

---

## Task 6: Build verification

- [ ] **Step 1: Full backend build**

Run: `cd src-tauri && cargo build 2>&1 | tail -20`
Expected: Finished `dev` profile, no errors.

- [ ] **Step 2: Frontend typecheck (no TS changes in Phase 0, but confirm nothing broke)**

Run: `npm run build 2>&1 | tail -20` (or the project's tsc step)
Expected: builds clean.

- [ ] **Step 3: Commit only if a fix was required** (otherwise skip).

---

## Task 7: Docs + release

**Files:**
- Modify: `README.md`, `CHANGELOG.md`

- [ ] **Step 1: Add CHANGELOG entry**

Under the top/Unreleased section of `CHANGELOG.md`:

```markdown
### Fixed
- Remote edit: re-opening a file no longer triggers a spurious "upload?" prompt, and a single save no longer asks for upload confirmation twice. Change detection now compares file content (hash) instead of modification time.
- App now stops SSH control sessions, removes runtime key copies, and clears session credentials when the window is closed via the OS title bar (previously only on explicit Disconnect/Quit).
```

- [ ] **Step 2: Update README** if it lists known limitations or the edit flow — note the fixes briefly. (If no relevant section exists, skip.)

- [ ] **Step 3: Commit docs**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: changelog + readme for Phase 0 edit-flow and close-cleanup fixes"
```

- [ ] **Step 4: Version bump + tag + push (phase boundary)**

Bump the patch version consistently across `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and `package.json` (current 1.4.6 → 1.4.7). Then:

```bash
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json package.json
git commit -m "release: v1.4.7 — edit-flow and close-cleanup bug fixes"
git tag -a v1.4.7 -m "v1.4.7 — Phase 0 bug fixes"
git push origin main --follow-tags
```

- [ ] **Step 5: Refresh knowledge graph + vault**

Run: `npx gitnexus analyze` (refresh stale index). Update the Obsidian/vault tracking note with the Phase 0 outcome.

---

## Manual Acceptance Checklist (run after Task 6, before release)

Connect to a real server with a small text file, then verify:

1. **Re-edit cleanliness:** Open a file via Edit → make a change → save → confirm exactly **one** upload/prompt. Close editor. Click Edit again → editor reopens with current remote content → **no** upload prompt appears until you actually edit and save.
2. **No double confirm:** With an editor that saves via temp+rename (e.g. gedit/VS Code), make one change and save → exactly **one** `upload-ready` confirmation (Confirm mode) or one `upload-complete` (Auto mode).
3. **Close cleanup:** Connect, then close the window via the OS title-bar X. Verify no leftover runtime key files in `~/.config/murmurssh/secrets/` (runtime copies) and that ControlMaster sockets are gone. Re-launch confirms a clean state.

If any check fails, treat it as a new debugging task (use superpowers:systematic-debugging) before releasing.
