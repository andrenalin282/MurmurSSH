# Copy-to, Folder DnD, Terminal Selection, Safe Upload, Keyboard — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Safe (non-destructive) uploads, Enter/Esc in every dialog, a terminal switch that works beyond Debian/Ubuntu, multi-select + folder drag & drop in the local panel with drop-on-folder targets, a background "Copy to…" action, and a fully documented keyboard layer.

**Architecture:** Backend gains three small services (`transfer_paths`, `terminal_service`, `remote_copy`) plus a new queue job kind `remoteCopy`; SFTP/FTP upload paths switch to temp-file + rename. Frontend gains four small modules (`modal-keys.ts`, `shortcuts.ts`, `panel-focus.ts`, `list-selection.ts`) that both file browsers and all dialogs consume; the help dialog renders its shortcut table from `shortcuts.ts`.

**Tech Stack:** Tauri 2, Rust (`ssh2` 0.9, `suppaftp` 8), vanilla TypeScript (no framework), Vite.

**Spec:** `docs/superpowers/specs/2026-09-13-copy-dnd-terminal-keyboard-design.md`

## Global Constraints

- Frontend `.ts` files have tracked `.js` siblings: edit `.ts`, run `npx tsc`, commit both.
- i18n: every new key goes into all 6 locales `src/i18n/{en,de,fr,nl,pl,ru}.ts` (tsc enforces key parity via `DeepStringify<typeof en>`). EN and DE texts are given; translate the EN text into FR/NL/PL/RU.
- No new crates, no new npm packages.
- New Settings fields: `Option<_>` + `#[serde(skip_serializing_if = "Option::is_none")]` (backward compatible JSON).
- Local path commands: reject null bytes, require absolute paths (same checks as `local_service::rename_local_file`).
- Commit messages short, no `Co-Authored-By` trailer.
- Every keyboard shortcut in the app MUST be listed in the help dialog (rendered from `src/shortcuts.ts`).
- Before editing an existing Rust/TS symbol run `gitnexus_impact` on it (project rule); run `gitnexus_detect_changes` before each commit.
- Validation per task: `cd src-tauri && cargo build && cargo clippy && cargo test --lib` (backend) / `npx tsc && npx vite build` (frontend).

---

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/services/transfer_paths.rs` (new) | pure helpers: `.murmur-part` name, symlink loop guard, error summary |
| `src-tauri/src/services/sftp_service.rs` | safe upload (single + dir), `exec_command`, `exec_available`, `is_remote_dir` |
| `src-tauri/src/services/ftp_service.rs` | safe upload (single + dir), `is_remote_dir` |
| `src-tauri/src/services/terminal_service.rs` (new) | terminal table, detection, resolution |
| `src-tauri/src/services/ssh_service.rs` | uses `terminal_service::resolve` |
| `src-tauri/src/services/remote_copy.rs` (new) | copy orchestration: exec probe cache, `cp`, fallback via temp dir |
| `src-tauri/src/models/transfer.rs` | `TransferKind::RemoteCopy` |
| `src-tauri/src/services/transfer_queue.rs` | dispatch `RemoteCopy` |
| `src-tauri/src/services/local_service.rs` + `commands/local.rs` | `create_local_dir`, `delete_local_path` |
| `src/components/modal-keys.ts` (new) | central Enter/Esc handler for the topmost modal |
| `src/shortcuts.ts` (new) | single shortcut registry + matcher + help table HTML |
| `src/panel-focus.ts` (new) | active panel state (`local`/`remote`) |
| `src/components/list-selection.ts` (new) | pure keyboard cursor/selection math shared by both browsers |
| `src/components/local-file-browser.ts` | selection, folder drag, drop-on-folder, keys |
| `src/components/file-browser.ts` | drop-on-folder, Copy to…, clipboard, keys |
| `src/main.ts` | install modal keys, help table, global keys, wiring |

---

### Task 1: Pure transfer helpers (`transfer_paths.rs`)

**Files:**
- Create: `src-tauri/src/services/transfer_paths.rs`
- Modify: `src-tauri/src/services/mod.rs` (add `pub mod transfer_paths;`)

**Interfaces:**
- Produces:
  - `pub fn part_path(remote_path: &str) -> String`
  - `pub struct LoopGuard` with `pub fn new() -> Self`, `pub fn enter(&mut self, dir: &Path) -> bool` (false = already on stack / unresolvable), `pub fn leave(&mut self, dir: &Path)`
  - `pub fn summarize_failures(failures: &[String], total: usize) -> Result<(), String>`

- [ ] **Step 1: Write the module with failing tests first**

```rust
//! Pure helpers shared by SFTP and FTP upload paths.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Temporary upload name next to the final target: `/a/b/file.txt` → `/a/b/.file.txt.murmur-part`.
/// The original file stays untouched until the upload completed and is renamed over it.
pub fn part_path(remote_path: &str) -> String {
    let trimmed = remote_path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(idx) => format!("{}/.{}.murmur-part", &trimmed[..idx], &trimmed[idx + 1..]),
        None => format!(".{}.murmur-part", trimmed),
    }
}

/// Tracks canonical directories on the current recursion stack so a symlink that points
/// back to an ancestor does not recurse forever.
pub struct LoopGuard {
    stack: HashSet<PathBuf>,
}

impl LoopGuard {
    pub fn new() -> Self {
        Self { stack: HashSet::new() }
    }

    /// Returns false when `dir` is already being walked (cycle) or cannot be resolved.
    pub fn enter(&mut self, dir: &Path) -> bool {
        match dir.canonicalize() {
            Ok(c) => self.stack.insert(c),
            Err(_) => false,
        }
    }

    pub fn leave(&mut self, dir: &Path) {
        if let Ok(c) = dir.canonicalize() {
            self.stack.remove(&c);
        }
    }
}

/// `Ok(())` when nothing failed, otherwise one error line listing up to 3 failures.
pub fn summarize_failures(failures: &[String], total: usize) -> Result<(), String> {
    if failures.is_empty() {
        return Ok(());
    }
    let shown: Vec<&str> = failures.iter().take(3).map(|s| s.as_str()).collect();
    let more = if failures.len() > 3 { "; …" } else { "" };
    Err(format!(
        "{} of {} entries failed: {}{}",
        failures.len(),
        total,
        shown.join("; "),
        more
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_path_places_hidden_part_next_to_target() {
        assert_eq!(part_path("/a/b/file.txt"), "/a/b/.file.txt.murmur-part");
        assert_eq!(part_path("/file"), "/.file.murmur-part");
        assert_eq!(part_path("file"), ".file.murmur-part");
    }

    #[test]
    fn loop_guard_detects_symlink_cycle() {
        let root = std::env::temp_dir().join(format!("murmur-loopguard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::os::unix::fs::symlink(&root, root.join("sub").join("back")).unwrap();

        let mut g = LoopGuard::new();
        assert!(g.enter(&root));
        assert!(g.enter(&root.join("sub")));
        assert!(!g.enter(&root.join("sub").join("back")), "cycle must be rejected");
        g.leave(&root.join("sub"));
        assert!(g.enter(&root.join("sub")), "re-enter after leave is allowed");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn summarize_failures_formats() {
        assert!(summarize_failures(&[], 5).is_ok());
        let f: Vec<String> = (1..=4).map(|i| format!("f{}: denied", i)).collect();
        let err = summarize_failures(&f, 10).unwrap_err();
        assert_eq!(err, "4 of 10 entries failed: f1: denied; f2: denied; f3: denied; …");
    }
}
```

- [ ] **Step 2: Register module, run tests**

Add `pub mod transfer_paths;` to `src-tauri/src/services/mod.rs`.
Run: `cd src-tauri && cargo test --lib transfer_paths`
Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/services/transfer_paths.rs src-tauri/src/services/mod.rs
git commit -m "feat: transfer path helpers (part file, loop guard, failure summary)"
```

---

### Task 2: Safe SFTP uploads (single file + directory)

**Files:**
- Modify: `src-tauri/src/services/sftp_service.rs` — `upload_file_inner` (~L274-L317), `upload_directory_inner` (~L637-L651), `upload_directory_recursive` (~L670-L741)

**Interfaces:**
- Consumes: `transfer_paths::{part_path, LoopGuard, summarize_failures}`
- Produces: `fn write_via_part(sftp: &ssh2::Sftp, local: &Path, remote_path: &str, name: &str, cancel: &dyn Fn() -> bool, on_progress: &dyn Fn(u64, u64, &str)) -> Result<(), String>` (private); public signatures of `upload_file` / `upload_directory` unchanged.

- [ ] **Step 1: Add the part-file writer + finalize helper**

Insert above `upload_file`:

```rust
/// Upload `local` to `<dir>/.<name>.murmur-part`, then rename over `remote_path`.
/// On error/cancel only the part file is removed — an existing target stays intact.
fn write_via_part(
    sftp: &ssh2::Sftp,
    local: &Path,
    remote_path: &str,
    name: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let mut local_file = std::fs::File::open(local)
        .map_err(|e| format!("Failed to open '{}': {}", local.display(), e))?;
    let total = local_file.metadata().map(|m| m.len()).unwrap_or(0);
    on_progress(0, total, name);

    let part = crate::services::transfer_paths::part_path(remote_path);
    let mut remote_file = sftp
        .create(Path::new(&part))
        .map_err(|e| format!("Failed to create remote file '{}': {}", part, e))?;

    let mut buf = vec![0u8; TRANSFER_CHUNK];
    let mut done = 0u64;
    let write_result: Result<(), String> = loop {
        if cancel() {
            break Err(CANCELLED_ERROR.to_string());
        }
        let n = match local_file.read(&mut buf) {
            Ok(v) => v,
            Err(e) => break Err(format!("Read '{}' failed: {}", local.display(), e)),
        };
        if n == 0 {
            break Ok(());
        }
        if let Err(e) = remote_file.write_all(&buf[..n]) {
            break Err(format!("Upload '{}' failed: {}", remote_path, e));
        }
        done += n as u64;
        on_progress(done, total, name);
    };
    drop(remote_file);
    if let Err(e) = write_result {
        let _ = sftp.unlink(Path::new(&part));
        return Err(e);
    }
    finalize_part(sftp, &part, remote_path)
}

/// Rename the finished part file over the target. SFTP v3 servers (OpenSSH) refuse to
/// rename onto an existing file, so fall back to unlink + rename; the old content is only
/// removed after the new content is completely on the server.
fn finalize_part(sftp: &ssh2::Sftp, part: &str, target: &str) -> Result<(), String> {
    use ssh2::RenameFlags;
    let flags = RenameFlags::OVERWRITE | RenameFlags::ATOMIC | RenameFlags::NATIVE;
    if sftp.rename(Path::new(part), Path::new(target), Some(flags)).is_ok() {
        return Ok(());
    }
    if sftp.stat(Path::new(target)).is_ok() {
        sftp.unlink(Path::new(target))
            .map_err(|e| format!("Cannot replace '{}': {}", target, e))?;
    }
    sftp.rename(Path::new(part), Path::new(target), None).map_err(|e| {
        let _ = sftp.unlink(Path::new(part));
        format!("Failed to finalize '{}': {}", target, e)
    })
}
```

- [ ] **Step 2: Rewrite `upload_file_inner` to use it**

```rust
fn upload_file_inner(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    // Fail fast on a missing local file before opening a network session.
    std::fs::metadata(local_path)
        .map_err(|e| format!("Failed to open local file '{}': {}", local_path, e))?;
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;
    write_via_part(&sftp, Path::new(local_path), remote_path, "", cancel, &|d, t, _| on_progress(d, t))
}
```

- [ ] **Step 3: Rewrite directory upload (collect failures, loop guard)**

Replace `upload_directory_inner` and `upload_directory_recursive`:

```rust
fn upload_directory_inner(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    mkdir_ok_if_exists(&sftp, Path::new(remote_path))?;
    let mut guard = crate::services::transfer_paths::LoopGuard::new();
    let mut failures: Vec<String> = Vec::new();
    let mut total = 0usize;
    upload_directory_recursive(
        cancel, &sftp, Path::new(local_path), remote_path, on_progress,
        &mut guard, &mut failures, &mut total,
    )?;
    crate::services::transfer_paths::summarize_failures(&failures, total)
}

/// Walks `local_dir`. Per-entry failures are pushed to `failures` and the walk continues;
/// only cancellation returns `Err` early.
#[allow(clippy::too_many_arguments)]
fn upload_directory_recursive(
    cancel: &dyn Fn() -> bool,
    sftp: &ssh2::Sftp,
    local_dir: &Path,
    remote_dir: &str,
    on_progress: &dyn Fn(u64, u64, &str),
    guard: &mut crate::services::transfer_paths::LoopGuard,
    failures: &mut Vec<String>,
    total: &mut usize,
) -> Result<(), String> {
    if cancel() {
        return Err(CANCELLED_ERROR.to_string());
    }
    if !guard.enter(local_dir) {
        return Ok(()); // symlink cycle — skip silently
    }
    let read_dir = match std::fs::read_dir(local_dir) {
        Ok(r) => r,
        Err(e) => {
            failures.push(format!("{}: {}", local_dir.display(), e));
            guard.leave(local_dir);
            return Ok(());
        }
    };

    for entry in read_dir {
        if cancel() {
            guard.leave(local_dir);
            return Err(CANCELLED_ERROR.to_string());
        }
        let local_entry = match entry {
            Ok(e) => e.path(),
            Err(e) => { failures.push(format!("{}: {}", local_dir.display(), e)); continue; }
        };
        let entry_name = match local_entry.file_name() {
            Some(n) if !n.is_empty() => n.to_string_lossy().to_string(),
            _ => continue,
        };
        let remote_entry = format!("{}/{}", remote_dir.trim_end_matches('/'), entry_name);

        if local_entry.is_dir() {
            *total += 1;
            if let Err(e) = mkdir_ok_if_exists(sftp, Path::new(&remote_entry)) {
                failures.push(format!("{}: {}", entry_name, e));
                continue;
            }
            upload_directory_recursive(cancel, sftp, &local_entry, &remote_entry, on_progress, guard, failures, total)?;
        } else if local_entry.is_file() {
            *total += 1;
            match write_via_part(sftp, &local_entry, &remote_entry, &entry_name, cancel, on_progress) {
                Ok(()) => {}
                Err(e) if e == CANCELLED_ERROR => { guard.leave(local_dir); return Err(e); }
                Err(e) => failures.push(format!("{}: {}", entry_name, e)),
            }
        }
        // Broken symlinks and special files are skipped without error.
    }
    guard.leave(local_dir);
    Ok(())
}
```

- [ ] **Step 4: Build + tests**

Run: `cd src-tauri && cargo build && cargo clippy && cargo test --lib`
Expected: build + clippy clean, all tests pass.

- [ ] **Step 5: Manual check (test SSH host)**

1. Upload a 200 MB file over an existing remote file, cancel in the queue at ~50% → remote file still has old size/content, no `.murmur-part` left.
2. Upload a folder where one file is `chmod 000` locally → job fails with `1 of N entries failed: …`, all other files present remotely.
3. Local folder containing `ln -s .. loop` → upload finishes, no endless recursion.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/sftp_service.rs
git commit -m "fix: SFTP uploads write to part file and never delete existing targets on failure"
```

---

### Task 3: Safe FTP uploads

**Files:**
- Modify: `src-tauri/src/services/ftp_service.rs` — `upload_file` (~L187-L214), `upload_directory` + `upload_dir_recursive` (~L307-L366)

**Interfaces:**
- Consumes: `transfer_paths::{part_path, LoopGuard, summarize_failures}`
- Produces: private `fn put_via_part(ftp: &mut FtpStream, local: &std::path::Path, remote_path: &str) -> Result<(), String>`; public signatures unchanged. Also `pub fn is_remote_dir(profile: &Profile, path: &str) -> Result<bool, String>` (used in Task 9).

- [ ] **Step 1: Add helpers**

```rust
/// Upload to `.<name>.murmur-part`, then replace the target. On failure only the part file
/// is removed so an existing remote file survives.
fn put_via_part(ftp: &mut FtpStream, local: &std::path::Path, remote_path: &str) -> Result<(), String> {
    let mut file = std::fs::File::open(local)
        .map_err(|e| format!("Cannot read '{}': {}", local.display(), e))?;
    let part = crate::services::transfer_paths::part_path(remote_path);
    if let Err(e) = ftp.put_file(&part, &mut file) {
        let _ = ftp.rm(&part);
        return Err(format!("FTP upload '{}' failed: {}", remote_path, e));
    }
    // RNTO onto an existing file is server-dependent; remove the target first (ignore not-found).
    let _ = ftp.rm(remote_path);
    ftp.rename(&part, remote_path).map_err(|e| {
        let _ = ftp.rm(&part);
        format!("FTP finalize '{}' failed: {}", remote_path, e)
    })
}

/// True when `path` is a directory on the server (CWD succeeds).
pub fn is_remote_dir(profile: &Profile, path: &str) -> Result<bool, String> {
    let mut ftp = connect(profile)?;
    let is_dir = ftp.cwd(path).is_ok();
    let _ = ftp.quit();
    Ok(is_dir)
}
```

- [ ] **Step 2: Use it in `upload_file`**

Replace the body after `let mut ftp = connect(profile)?;`:

```rust
    let mut ftp = connect(profile)?;
    let result = put_via_part(&mut ftp, std::path::Path::new(local_path), remote_path);
    let _ = ftp.quit();
    result?;
    on_progress(total, total, &name);
    Ok(())
```

(Remove the now-unused `file` open; keep `total` via `std::fs::metadata(local_path).map(|m| m.len()).unwrap_or(0)`.)

- [ ] **Step 3: Directory upload with failures + loop guard**

```rust
pub fn upload_directory(
    profile: &Profile,
    local_path: &str,
    remote_path: &str,
    cancel: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, u64, &str),
) -> Result<(), String> {
    let mut ftp = connect(profile)?;
    let mut guard = crate::services::transfer_paths::LoopGuard::new();
    let mut failures = Vec::new();
    let mut total = 0usize;
    let result = upload_dir_recursive(
        cancel, &mut ftp, std::path::Path::new(local_path), remote_path, on_progress,
        &mut guard, &mut failures, &mut total,
    );
    let _ = ftp.quit();
    result?;
    crate::services::transfer_paths::summarize_failures(&failures, total)
}

#[allow(clippy::too_many_arguments)]
fn upload_dir_recursive(
    cancel: &dyn Fn() -> bool,
    ftp: &mut FtpStream,
    local_dir: &std::path::Path,
    remote_dir: &str,
    on_progress: &dyn Fn(u64, u64, &str),
    guard: &mut crate::services::transfer_paths::LoopGuard,
    failures: &mut Vec<String>,
    total: &mut usize,
) -> Result<(), String> {
    if cancel() {
        return Err(CANCELLED_ERROR.to_string());
    }
    if !guard.enter(local_dir) {
        return Ok(());
    }
    // Create the remote directory; ignore error if it already exists.
    let _ = ftp.mkdir(remote_dir);

    let read_dir = match std::fs::read_dir(local_dir) {
        Ok(r) => r,
        Err(e) => {
            failures.push(format!("{}: {}", local_dir.display(), e));
            guard.leave(local_dir);
            return Ok(());
        }
    };

    for entry in read_dir {
        if cancel() {
            guard.leave(local_dir);
            return Err(CANCELLED_ERROR.to_string());
        }
        let local_entry = match entry {
            Ok(e) => e.path(),
            Err(e) => { failures.push(format!("{}: {}", local_dir.display(), e)); continue; }
        };
        let name = local_entry.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let remote_entry = format!("{}/{}", remote_dir.trim_end_matches('/'), name);

        if local_entry.is_dir() {
            *total += 1;
            upload_dir_recursive(cancel, ftp, &local_entry, &remote_entry, on_progress, guard, failures, total)?;
        } else if local_entry.is_file() {
            *total += 1;
            let size = local_entry.metadata().map(|m| m.len()).unwrap_or(0);
            on_progress(0, size, &name);
            match put_via_part(ftp, &local_entry, &remote_entry) {
                Ok(()) => on_progress(size, size, &name),
                Err(e) => failures.push(format!("{}: {}", name, e)),
            }
        }
    }
    guard.leave(local_dir);
    Ok(())
}
```

- [ ] **Step 4: Build + tests**

Run: `cd src-tauri && cargo build && cargo clippy && cargo test --lib`
Expected: clean, all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/ftp_service.rs
git commit -m "fix: FTP uploads via part file, per-file failures no longer abort folder upload"
```

---

### Task 4: Enter/Esc for every dialog (`modal-keys.ts`)

**Files:**
- Create: `src/components/modal-keys.ts`
- Modify: `src/components/dialog.ts`, `src/components/credential-dialog.ts`, `src/components/profile-form.ts`, `src/components/settings-dialog.ts`, `src/components/update-dialog.ts`, `src/main.ts` (help dialog + install call)

**Interfaces:**
- Produces: `export function installModalKeyHandler(): void`; attribute contract `data-modal-primary` / `data-modal-cancel` on buttons inside `.modal-overlay`.

- [ ] **Step 1: Create the handler**

```ts
/**
 * Central keyboard handling for in-app modals.
 *
 * The topmost `.modal-overlay` (last in DOM order) receives:
 *  - Escape → click its `[data-modal-cancel]` button
 *  - Enter  → click its `[data-modal-primary]` button, unless focus is in a textarea/select
 *             or on a button (a focused button keeps native Enter behaviour)
 * Handled events are stopped so file-browser shortcuts never see them.
 */
let installed = false;

export function installModalKeyHandler(): void {
  if (installed) return;
  installed = true;

  document.addEventListener(
    "keydown",
    (e) => {
      if (e.key !== "Escape" && e.key !== "Enter") return;
      const overlays = document.querySelectorAll<HTMLElement>(".modal-overlay");
      if (overlays.length === 0) return;
      const top = overlays[overlays.length - 1];

      if (e.key === "Escape") {
        const cancel = top.querySelector<HTMLButtonElement>("[data-modal-cancel]");
        if (cancel && !cancel.disabled) {
          e.preventDefault();
          e.stopPropagation();
          cancel.click();
        }
        return;
      }

      // Enter
      const active = document.activeElement as HTMLElement | null;
      const tag = active?.tagName?.toLowerCase();
      if (tag === "textarea" || tag === "select" || tag === "button") return;
      if (e.isComposing) return;
      const primary = top.querySelector<HTMLButtonElement>("[data-modal-primary]");
      if (primary && !primary.disabled) {
        e.preventDefault();
        e.stopPropagation();
        primary.click();
      }
    },
    true, // capture: run before component-level handlers
  );
}
```

- [ ] **Step 2: Install in `main.ts`**

Add `import { installModalKeyHandler } from "./components/modal-keys";` and call `installModalKeyHandler();` directly after the imports/theme setup, before components are constructed.

- [ ] **Step 3: Mark buttons in every modal**

Apply these exact attribute additions:

| File | Button | Attribute |
|---|---|---|
| `dialog.ts` showPrompt | `#modal-cancel` / `#modal-confirm` | `data-modal-cancel` / `data-modal-primary` |
| `dialog.ts` showOverwriteDialog | `#overwrite-cancel` / `#overwrite-yes` | `data-modal-cancel` / `data-modal-primary` |
| `dialog.ts` showPermissionsDialog | `#perm-cancel` / `#perm-apply` | `data-modal-cancel` / `data-modal-primary` |
| `dialog.ts` showConfirm | `#modal-cancel` / `#modal-confirm` | `data-modal-cancel` / `data-modal-primary` |
| `credential-dialog.ts` password | `#cred-cancel` / its submit button | `data-modal-cancel` / `data-modal-primary` |
| `credential-dialog.ts` passphrase | `#pp-cancel` / its submit button | `data-modal-cancel` / `data-modal-primary` |
| `credential-dialog.ts` host key | `#hk-cancel` | `data-modal-cancel` only (NO primary — trust must be clicked) |
| `profile-form.ts` | `#pf-cancel` / `#pf-save` | `data-modal-cancel` / `data-modal-primary` |
| `profile-form.ts` import result | `#ssh-import-result-ok` | `data-modal-cancel data-modal-primary` |
| `profile-form.ts` import | `#ssh-import-cancel` / `#ssh-import-confirm` | `data-modal-cancel` / `data-modal-primary` |
| `settings-dialog.ts` | `#settings-cancel` / `#settings-apply` | `data-modal-cancel` / `data-modal-primary` |
| `update-dialog.ts` | `#update-later` / `#update-open` | `data-modal-cancel` / `data-modal-primary` |
| `main.ts` help | `#help-close` | `data-modal-cancel data-modal-primary` |
| `file-browser.ts` | any other `.modal-overlay` built there (grep `modal-overlay`) | same rule: cancel + primary |

Forms with `<button type="submit">` (`#cred-form`, `#pp-form`, `#pf-form`): the credential submit buttons have no id — add `data-modal-primary` directly to `<button type="submit">`. The handler's `preventDefault()` on the capture-phase keydown suppresses the browser's implicit form submission, and `primary.click()` on a submit button fires the form's `submit` event once — no double submit. Verify in Step 6 that each form submits exactly once.

- [ ] **Step 4: Remove the prompt's own key handler**

In `showPrompt` delete the `input.addEventListener("keydown", …)` block (central handler covers it).

- [ ] **Step 5: Focus rule**

- `showConfirm`: after `appendChild` add `setTimeout(() => overlay.querySelector<HTMLButtonElement>("#modal-confirm")?.focus(), 10);` (a focused button handles Enter natively = confirm; Esc still goes through the central handler).
- Help dialog (`main.ts`): after `appendChild` add `setTimeout(() => overlay.querySelector<HTMLButtonElement>("#help-close")?.focus(), 10);`.
- Settings dialog / profile form: focus the first `input` of the overlay the same way if they do not already.

- [ ] **Step 6: Build + manual check**

Run: `npx tsc && npx vite build` → no errors.
`npm run tauri dev`: open each dialog (confirm delete, overwrite, permissions, rename prompt, password prompt, passphrase prompt, host key, profile form, SSH import, settings, update, help) → Esc closes as cancel; Enter triggers the primary action; host-key dialog ignores Enter. File browser does not react (no refresh/delete) while a dialog handles the key.

- [ ] **Step 7: Commit**

```bash
git add src/components/*.ts src/components/*.js src/main.ts src/main.js
git commit -m "feat: all dialogs close with Esc and confirm with Enter"
```

---

### Task 5: `terminal_service` (detection + resolution)

**Files:**
- Create: `src-tauri/src/services/terminal_service.rs`
- Modify: `src-tauri/src/services/mod.rs`, `src-tauri/src/models/settings.rs`

**Interfaces:**
- Produces:
  - `pub struct TerminalSpec { pub id: &'static str, pub binary: &'static str, pub prefix: &'static [&'static str] }`
  - `pub const KNOWN_TERMINALS: &[TerminalSpec]`
  - `pub fn detect() -> Vec<String>`
  - `pub fn resolve(settings: &Settings) -> Result<(String, Vec<String>), String>` — `(program, prefix_args)`
  - `pub const NO_TERMINAL_ERROR: &str = "NO_TERMINAL"`
  - Settings fields `terminal: Option<String>`, `terminal_custom_command: Option<String>`

- [ ] **Step 1: Settings fields**

Append to `Settings` in `models/settings.rs`:

```rust
    /// Terminal used for SSH sessions: "auto" (default when None), a known terminal id
    /// from `terminal_service::KNOWN_TERMINALS`, or "custom".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
    /// Program + prefix args when `terminal == "custom"`, e.g. "wezterm start --".
    /// The `bash -c … ssh …` invocation is appended after these tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_custom_command: Option<String>,
```

- [ ] **Step 2: Service with tests (write tests first, then the functions)**

```rust
//! Terminal emulator selection for SSH sessions.

use std::path::Path;

use crate::models::Settings;

pub const NO_TERMINAL_ERROR: &str = "NO_TERMINAL";

pub struct TerminalSpec {
    pub id: &'static str,
    pub binary: &'static str,
    /// Args placed between the binary and the command to run.
    pub prefix: &'static [&'static str],
}

/// Auto-detection order. `x-terminal-emulator` first keeps Debian/Ubuntu behaviour unchanged.
pub const KNOWN_TERMINALS: &[TerminalSpec] = &[
    TerminalSpec { id: "x-terminal-emulator", binary: "x-terminal-emulator", prefix: &["-e"] },
    TerminalSpec { id: "gnome-terminal", binary: "gnome-terminal", prefix: &["--"] },
    TerminalSpec { id: "ptyxis", binary: "ptyxis", prefix: &["--"] },
    TerminalSpec { id: "kgx", binary: "kgx", prefix: &["--"] },
    TerminalSpec { id: "konsole", binary: "konsole", prefix: &["-e"] },
    TerminalSpec { id: "xfce4-terminal", binary: "xfce4-terminal", prefix: &["-x"] },
    TerminalSpec { id: "kitty", binary: "kitty", prefix: &[] },
    TerminalSpec { id: "alacritty", binary: "alacritty", prefix: &["-e"] },
    TerminalSpec { id: "foot", binary: "foot", prefix: &[] },
    TerminalSpec { id: "wezterm", binary: "wezterm", prefix: &["start", "--"] },
    TerminalSpec { id: "xterm", binary: "xterm", prefix: &["-e"] },
];

fn on_path(binary: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| is_executable(&dir.join(binary)))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// Ids of known terminals installed on this machine, in detection order.
pub fn detect() -> Vec<String> {
    detect_with(&on_path)
}

fn detect_with(exists: &dyn Fn(&str) -> bool) -> Vec<String> {
    KNOWN_TERMINALS.iter().filter(|t| exists(t.binary)).map(|t| t.id.to_string()).collect()
}

fn spec_to_pair(t: &TerminalSpec) -> (String, Vec<String>) {
    (t.binary.to_string(), t.prefix.iter().map(|s| s.to_string()).collect())
}

pub fn resolve(settings: &Settings) -> Result<(String, Vec<String>), String> {
    let env_terminal = std::env::var("TERMINAL").ok();
    resolve_with(settings, env_terminal.as_deref(), &on_path)
}

fn resolve_with(
    settings: &Settings,
    env_terminal: Option<&str>,
    exists: &dyn Fn(&str) -> bool,
) -> Result<(String, Vec<String>), String> {
    let choice = settings.terminal.as_deref().unwrap_or("auto");

    if choice == "custom" {
        // lc-debt: whitespace split, no shell quoting; upgrade to a shell-words parser if users need quoted args.
        let raw = settings.terminal_custom_command.as_deref().unwrap_or("").trim();
        let mut parts = raw.split_whitespace().map(str::to_string);
        return match parts.next() {
            Some(program) => Ok((program, parts.collect())),
            None => Err(NO_TERMINAL_ERROR.to_string()),
        };
    }

    if choice != "auto" {
        if let Some(t) = KNOWN_TERMINALS.iter().find(|t| t.id == choice) {
            return Ok(spec_to_pair(t));
        }
    }

    if let Some(term) = env_terminal.map(str::trim).filter(|s| !s.is_empty()) {
        let base = Path::new(term).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let known = KNOWN_TERMINALS.iter().find(|t| t.binary == base);
        if Path::new(term).is_absolute() || exists(term) {
            let prefix = known.map(|t| t.prefix.iter().map(|s| s.to_string()).collect()).unwrap_or_else(|| vec!["-e".to_string()]);
            return Ok((term.to_string(), prefix));
        }
    }

    KNOWN_TERMINALS
        .iter()
        .find(|t| exists(t.binary))
        .map(spec_to_pair)
        .ok_or_else(|| NO_TERMINAL_ERROR.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(terminal: Option<&str>, custom: Option<&str>) -> Settings {
        Settings {
            terminal: terminal.map(str::to_string),
            terminal_custom_command: custom.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn auto_on_arch_picks_first_installed_known_terminal() {
        let exists = |b: &str| b == "konsole" || b == "kitty";
        let (p, a) = resolve_with(&settings(None, None), None, &exists).unwrap();
        assert_eq!(p, "konsole");
        assert_eq!(a, vec!["-e"]);
    }

    #[test]
    fn auto_prefers_x_terminal_emulator() {
        let exists = |b: &str| b == "x-terminal-emulator" || b == "gnome-terminal";
        let (p, a) = resolve_with(&settings(Some("auto"), None), None, &exists).unwrap();
        assert_eq!((p.as_str(), a), ("x-terminal-emulator", vec!["-e".to_string()]));
    }

    #[test]
    fn env_terminal_known_basename_uses_its_flag() {
        let exists = |b: &str| b == "wezterm";
        let (p, a) = resolve_with(&settings(None, None), Some("wezterm"), &exists).unwrap();
        assert_eq!(p, "wezterm");
        assert_eq!(a, vec!["start", "--"]);
    }

    #[test]
    fn env_terminal_unknown_defaults_to_dash_e() {
        let exists = |b: &str| b == "myterm";
        let (p, a) = resolve_with(&settings(None, None), Some("myterm"), &exists).unwrap();
        assert_eq!((p.as_str(), a), ("myterm", vec!["-e".to_string()]));
    }

    #[test]
    fn explicit_known_choice_wins() {
        let exists = |_: &str| true;
        let (p, a) = resolve_with(&settings(Some("gnome-terminal"), None), Some("xterm"), &exists).unwrap();
        assert_eq!((p.as_str(), a), ("gnome-terminal", vec!["--".to_string()]));
    }

    #[test]
    fn custom_command_is_split() {
        let exists = |_: &str| false;
        let (p, a) = resolve_with(&settings(Some("custom"), Some("  foot --app-id murmur ")), None, &exists).unwrap();
        assert_eq!(p, "foot");
        assert_eq!(a, vec!["--app-id", "murmur"]);
    }

    #[test]
    fn nothing_found_is_no_terminal() {
        let exists = |_: &str| false;
        assert_eq!(resolve_with(&settings(None, None), None, &exists).unwrap_err(), NO_TERMINAL_ERROR);
        assert_eq!(resolve_with(&settings(Some("custom"), Some("  ")), None, &exists).unwrap_err(), NO_TERMINAL_ERROR);
    }

    #[test]
    fn detect_filters_installed() {
        let exists = |b: &str| b == "xterm" || b == "foot";
        assert_eq!(detect_with(&exists), vec!["foot", "xterm"]);
    }
}
```

- [ ] **Step 3: Register + run tests**

Add `pub mod terminal_service;` to `services/mod.rs`.
Run: `cd src-tauri && cargo test --lib terminal_service`
Expected: 8 passed.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/terminal_service.rs src-tauri/src/services/mod.rs src-tauri/src/models/settings.rs
git commit -m "feat: terminal emulator detection and resolution service"
```

---

### Task 6: Wire terminal selection (launch + command + Settings UI)

**Files:**
- Modify: `src-tauri/src/services/ssh_service.rs` (`launch_ssh`), `src-tauri/src/commands/ssh.rs`, `src-tauri/src/lib.rs`, `src/api/index.ts`, `src/types.ts`, `src/components/settings-dialog.ts`, `src/components/file-browser.ts` (`handleTerminal` error text), `src/i18n/*.ts`

**Interfaces:**
- Consumes: `terminal_service::{resolve, detect, NO_TERMINAL_ERROR}`, `settings_service::get_settings`
- Produces: Tauri command `list_terminals() -> Vec<String>`; TS `api.listTerminals(): Promise<string[]>`; TS `Settings.terminal?: string | null`, `Settings.terminal_custom_command?: string | null`

- [ ] **Step 1: `launch_ssh` uses the resolver**

In `ssh_service::launch_ssh` replace `let mut cmd = Command::new("x-terminal-emulator");` with:

```rust
    let settings = crate::services::settings_service::get_settings().unwrap_or_default();
    let (program, prefix) = crate::services::terminal_service::resolve(&settings)?;
    let mut cmd = Command::new(&program);
    cmd.args(&prefix);
```

and replace the `cmd.arg("-e")\n        .arg("bash")` chain start with `cmd.arg("bash")` (the prefix now carries the exec flag). Update the error mapping:

```rust
        .map_err(|e| format!("Failed to launch terminal '{}': {}", program, e))
```

- [ ] **Step 2: Command + registration**

`commands/ssh.rs`:

```rust
/// Known terminal emulators installed on this machine (ids for the Settings dropdown).
#[tauri::command]
pub fn list_terminals() -> Vec<String> {
    crate::services::terminal_service::detect()
}
```

Register `commands::ssh::list_terminals,` in `lib.rs` next to `commands::ssh::launch_ssh`.

- [ ] **Step 3: TS api + types**

`src/api/index.ts`:

```ts
/** Ids of terminal emulators detected on this machine. */
export async function listTerminals(): Promise<string[]> {
  return invoke("list_terminals");
}
```

`src/types.ts` in `Settings`:

```ts
  /** "auto" | known terminal id | "custom". Absent = auto. */
  terminal?: string | null;
  /** Program + prefix args for terminal === "custom". */
  terminal_custom_command?: string | null;
```

- [ ] **Step 4: Settings UI**

In `settings-dialog.ts` `show()`: `const detectedTerminals = await api.listTerminals().catch(() => [] as string[]);` before building HTML; `const currentTerminal = settings.terminal ?? "auto";`. Insert after the editor-by-extension field:

```ts
        <div class="form-field">
          <label for="terminal-select">${t("settings.terminal")}</label>
          <select id="terminal-select" style="width:100%">
            <option value="auto" ${currentTerminal === "auto" ? "selected" : ""}>${t("settings.terminalAuto")}</option>
            ${[...new Set([...detectedTerminals, ...(currentTerminal !== "auto" && currentTerminal !== "custom" ? [currentTerminal] : [])])]
              .map((id) => `<option value="${escHtml(id)}" ${currentTerminal === id ? "selected" : ""}>${escHtml(id)}</option>`)
              .join("")}
            <option value="custom" ${currentTerminal === "custom" ? "selected" : ""}>${t("settings.terminalCustom")}</option>
          </select>
          <input id="terminal-custom-input" type="text" spellcheck="false"
            value="${escHtml(settings.terminal_custom_command ?? "")}"
            placeholder="${t("settings.terminalCustomPlaceholder")}"
            style="margin-top:6px;${currentTerminal === "custom" ? "" : "display:none"}">
          <div class="form-field__hint">${t("settings.terminalHint")}</div>
        </div>
```

Wire: `change` on `#terminal-select` toggles `#terminal-custom-input` display. In the apply handler (read-merge-write, same as other fields): `merged.terminal = select.value === "auto" ? null : select.value; merged.terminal_custom_command = customInput.value.trim() || null;`.

- [ ] **Step 5: Error message in `handleTerminal`**

```ts
    } catch (err) {
      const msg = String(err) === "NO_TERMINAL"
        ? t("fileBrowser.noTerminalFound")
        : t("fileBrowser.terminalFailed", { error: String(err) });
      this.status(msg, true);
    }
```

- [ ] **Step 6: i18n (all 6 locales)**

| Key | EN | DE |
|---|---|---|
| `settings.terminal` | Terminal for SSH sessions | Terminal für SSH-Sitzungen |
| `settings.terminalAuto` | Automatic | Automatisch |
| `settings.terminalCustom` | Custom command… | Eigener Befehl… |
| `settings.terminalCustomPlaceholder` | e.g. wezterm start -- | z. B. wezterm start -- |
| `settings.terminalHint` | Automatic tries $TERMINAL, x-terminal-emulator and common terminals. A custom command gets the SSH command appended. | Automatisch probiert $TERMINAL, x-terminal-emulator und gängige Terminals. An einen eigenen Befehl wird der SSH-Aufruf angehängt. |
| `fileBrowser.noTerminalFound` | No terminal emulator found. Choose one in Settings → Terminal. | Kein Terminal gefunden. Bitte unter Einstellungen → Terminal auswählen. |

- [ ] **Step 7: Build + manual**

`cd src-tauri && cargo build && cargo clippy && cargo test --lib`; `npx tsc && npx vite build`.
Manual: Settings shows detected terminals; choose `gnome-terminal` → F11 opens it; key+passphrase profile prompts for passphrase inside the terminal; choose Custom `xterm -e` → xterm opens; Custom empty → status "No terminal emulator found…".
Arch simulation: `PATH=/usr/local/bin:/bin npm run tauri dev` is not reliable — instead unit tests cover resolution; if an Arch VM/container with a desktop is available, test there.

- [ ] **Step 8: Commit**

```bash
git add -A src-tauri/src src/api src/types.ts src/types.js src/components src/i18n
git commit -m "feat: selectable terminal emulator (auto-detect, known list, custom command)"
```

---

### Task 7: Local browser selection + folder dragging

**Files:**
- Create: `src/components/list-selection.ts`
- Modify: `src/components/local-file-browser.ts`, `src/main.ts` (`localBrowser.onUpload` wiring), `src/components/file-browser.ts` (make `uploadPathList` public with `targetDir`), `src/styles.css` (`.lb-entry--selected`)

**Interfaces:**
- Produces:
  - `export interface SelectionState { selected: Set<string>; anchor: string | null; cursor: string | null }`
  - `export function clickSelect(names: string[], state: SelectionState, name: string, mods: { ctrl: boolean; shift: boolean }): SelectionState`
  - `export function moveCursor(names: string[], state: SelectionState, delta: number | "start" | "end", extend: boolean): SelectionState`
  - `FileBrowser.uploadPathList(localPaths: string[], targetDir?: string): Promise<void>` (now public)
  - `LocalFileBrowser.getSelectedPaths(): string[]`

- [ ] **Step 1: Pure selection module**

```ts
/** Pure selection math shared by the local and remote file browsers. */

export interface SelectionState {
  selected: Set<string>;
  anchor: string | null;
  cursor: string | null;
}

export function clickSelect(
  names: string[],
  state: SelectionState,
  name: string,
  mods: { ctrl: boolean; shift: boolean },
): SelectionState {
  const selected = new Set(state.selected);
  if (mods.ctrl) {
    if (selected.has(name)) selected.delete(name);
    else selected.add(name);
    return { selected, anchor: name, cursor: name };
  }
  if (mods.shift && state.anchor) {
    const a = names.indexOf(state.anchor);
    const b = names.indexOf(name);
    if (a >= 0 && b >= 0) {
      for (let i = Math.min(a, b); i <= Math.max(a, b); i++) selected.add(names[i]);
      return { selected, anchor: state.anchor, cursor: name };
    }
  }
  return { selected: new Set([name]), anchor: name, cursor: name };
}

export function moveCursor(
  names: string[],
  state: SelectionState,
  delta: number | "start" | "end",
  extend: boolean,
): SelectionState {
  if (names.length === 0) return state;
  const cur = state.cursor ? names.indexOf(state.cursor) : -1;
  let next: number;
  if (delta === "start") next = 0;
  else if (delta === "end") next = names.length - 1;
  else if (cur < 0) next = delta > 0 ? 0 : names.length - 1;
  else next = Math.max(0, Math.min(names.length - 1, cur + delta));
  const target = names[next];
  if (!extend) return { selected: new Set([target]), anchor: target, cursor: target };
  const anchor = state.anchor ?? state.cursor ?? target;
  const a = names.indexOf(anchor);
  const selected = new Set<string>();
  for (let i = Math.min(a, next); i <= Math.max(a, next); i++) selected.add(names[i]);
  return { selected, anchor, cursor: target };
}
```

- [ ] **Step 2: Selection in `LocalFileBrowser`**

Add fields `private selectedNames = new Set<string>(); private anchorName: string | null = null; private cursorName: string | null = null;`. Clear them in `clear()`, `navigateTo()` success and `refresh()`.
In `render()` rows: add `lb-entry--selected` class when selected, `draggable="true"` for every row except `..`.
In `wireEvents()` add:

```ts
    tbody.addEventListener("click", (e) => {
      const row = (e.target as HTMLElement).closest<HTMLElement>("tr.lb-entry");
      if (!row) { this.selectedNames.clear(); this.anchorName = null; this.render(); return; }
      const name = row.dataset.name;
      if (!name || name === "..") return;
      const me = e as MouseEvent;
      const st = clickSelect(this.entries.map((x) => x.name),
        { selected: this.selectedNames, anchor: this.anchorName, cursor: this.cursorName },
        name, { ctrl: me.ctrlKey || me.metaKey, shift: me.shiftKey });
      this.applySelection(st);
    });
```

with

```ts
  private applySelection(st: SelectionState): void {
    this.selectedNames = st.selected;
    this.anchorName = st.anchor;
    this.cursorName = st.cursor;
    this.container.querySelectorAll<HTMLElement>("tr.lb-entry").forEach((r) => {
      r.classList.toggle("lb-entry--selected", this.selectedNames.has(r.dataset.name ?? ""));
      r.classList.toggle("lb-entry--cursor", r.dataset.name === this.cursorName);
    });
    this.container.querySelector<HTMLElement>(`tr.lb-entry[data-name="${CSS.escape(this.cursorName ?? "")}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }

  getSelectedPaths(): string[] {
    return this.entries.filter((e) => this.selectedNames.has(e.name)).map((e) => joinPath(this.currentPath, e.name));
  }
```

(Selection updates classes in place — no `render()` — so a subsequent drag is not interrupted.)

- [ ] **Step 3: Drag source carries selection incl. folders**

Replace the `dragstart` handler:

```ts
    tbody.addEventListener("dragstart", (e) => {
      const row = (e.target as HTMLElement).closest<HTMLElement>("tr.lb-entry");
      const name = row?.dataset.name;
      if (!row || !name || name === "..") { e.preventDefault(); return; }
      const paths = this.selectedNames.has(name)
        ? this.getSelectedPaths()
        : [joinPath(this.currentPath, name)];
      this.dragSourceNames = new Set(paths);
      setDragSource({ type: "local", paths });
      e.dataTransfer!.effectAllowed = "copy";
      e.dataTransfer!.setData("text/plain", "local-to-remote");
    });
```

- [ ] **Step 4: Remote side accepts folders**

In `file-browser.ts` change `private async uploadPathList(localPaths: string[])` to `async uploadPathList(localPaths: string[], targetDir: string = this.currentPath)` and use `joinPath(targetDir, name)` for `remotePath`. In the scroll-area local drop handler replace `void this.uploadFileList(src.paths);` with `void this.uploadPathList(src.paths);`.
In `main.ts` `localBrowser.onUpload` replace `await fileBrowser.uploadFileList(localPaths);` with `await fileBrowser.uploadPathList(localPaths);` (context-menu upload of a folder now works).

- [ ] **Step 5: CSS**

In `src/styles.css`, next to `.file-entry--selected`, add `.lb-entry--selected` with the same rules (copy the `.file-entry--selected` block and rename the selector), and `.lb-entry--cursor, .file-entry--cursor { outline: 1px dashed var(--accent); outline-offset: -1px; }`.

- [ ] **Step 6: Build + manual**

`npx tsc && npx vite build`. Manual: Ctrl/Shift-select 2 files + 1 folder locally, drag to remote → 3 queue jobs (folder = uploadDir), folder contents complete remotely; right-click folder → Upload → works.

- [ ] **Step 7: Commit**

```bash
git add src/components src/main.ts src/main.js src/styles.css
git commit -m "feat: multi-select and folder drag in local browser"
```

---

### Task 8: Drop onto folder rows (local→remote, remote→local, OS→remote)

**Files:**
- Modify: `src/components/file-browser.ts` (scroll-area local drop, `setupDragDrop`), `src/components/local-file-browser.ts` (drop handler)

**Interfaces:**
- Consumes: `FileBrowser.uploadPathList(paths, targetDir)`, `LocalFileBrowser.onDownloadCallback(names, destDir)` (existing signature), `FileBrowser.setDropTarget(name)`
- Produces: `private dropDirFromEvent(target: EventTarget | null): string` in both browsers.

- [ ] **Step 1: Remote — folder under cursor for local drags**

Add to `FileBrowser`:

```ts
  /** Remote directory for a drop at `target`: a folder row → that folder, ".." → parent, else current dir. */
  private dropDirFromElement(el: Element | null): string {
    const row = el?.closest<HTMLElement>("tr.file-entry");
    const name = row?.dataset.name;
    if (!row || !name || row.dataset.isdir !== "true") return this.currentPath;
    return name === ".." ? parentPath(this.currentPath) : joinPath(this.currentPath, name);
  }
```

In the scroll-area `dragover` for `src.type === "local"`, after `preventDefault`, highlight: `const row = (e.target as HTMLElement).closest<HTMLElement>("tr.file-entry"); this.setDropTarget(row?.dataset.isdir === "true" ? row.dataset.name ?? null : null);`. In `dragleave` (leaving the area) and `drop`, call `this.setDropTarget(null)`. In `drop`: `void this.uploadPathList(src.paths, this.dropDirFromElement(e.target as Element));`.

- [ ] **Step 2: Remote — OS drops use drop position**

In `setupDragDrop` `drop` branch:

```ts
          const payload = event.payload as { type: "drop"; paths: string[]; position: { x: number; y: number } };
          if (payload.paths.length > 0) {
            const dpr = window.devicePixelRatio || 1;
            const el = document.elementFromPoint(payload.position.x / dpr, payload.position.y / dpr);
            const insideRemote = el ? this.container.contains(el) : true;
            if (insideRemote) void this.uploadPathList(payload.paths, this.dropDirFromElement(el));
          }
```

(When the OS drop lands on the local panel it is ignored — previously it uploaded to the remote current dir regardless of where it was dropped; keep ignoring, the local panel is not an upload target.) In the `over` branch, also highlight: compute `el` the same way and `this.setDropTarget(...)` for folder rows; clear on `leave`/`drop`.

- [ ] **Step 3: Local — remote drags onto local folder rows**

Add to `LocalFileBrowser`:

```ts
  private dropDirFromElement(el: Element | null): string {
    const row = el?.closest<HTMLElement>("tr.lb-entry");
    if (!row || row.dataset.isdir !== "true" || !row.dataset.path) return this.currentPath;
    return row.dataset.path; // ".." row carries the parent path already
  }

  private setLocalDropTarget(path: string | null): void {
    this.container.querySelectorAll<HTMLElement>("tr.lb-entry").forEach((r) => {
      r.classList.toggle("lb-entry--drop-target", path !== null && r.dataset.path === path && r.dataset.isdir === "true");
    });
  }
```

In the local `dragover`: `const dir = this.dropDirFromElement(e.target as Element); this.setLocalDropTarget(dir === this.currentPath ? null : dir);`. In `dragleave`/`drop`: `this.setLocalDropTarget(null)`. In `drop`: `await this.onDownloadCallback(src.names, this.dropDirFromElement(e.target as Element));`.
CSS: add `.lb-entry--drop-target` mirroring `.file-entry--drop-target`.

- [ ] **Step 4: Build + manual**

`npx tsc && npx vite build`. Manual: local file dropped on remote folder row → lands inside that folder; on `..` → parent; on empty area → current dir. Remote file dropped on local folder row → downloaded into it. Nautilus drop on remote folder row → inside that folder; row highlights during drag.

- [ ] **Step 5: Commit**

```bash
git add src/components src/styles.css
git commit -m "feat: drop onto folder rows targets that folder in both panels"
```

---

### Task 9: Backend remote copy (`remote_copy` + `RemoteCopy` job)

**Files:**
- Create: `src-tauri/src/services/remote_copy.rs`
- Modify: `src-tauri/src/services/sftp_service.rs` (exec + is_remote_dir), `src-tauri/src/models/transfer.rs`, `src-tauri/src/services/transfer_queue.rs`, `src-tauri/src/commands/transfer.rs`, `src-tauri/src/commands/ssh.rs` (`stop_ssh_session`), `src-tauri/src/lib.rs`, `src-tauri/src/services/mod.rs`, `src/types.ts`

**Interfaces:**
- Consumes: `ftp_service::is_remote_dir` (Task 3), safe uploads (Tasks 2/3), existing download fns.
- Produces:
  - `TransferKind::RemoteCopy` (serde `"remoteCopy"`)
  - `sftp_service::exec_command(profile: &Profile, cmd: &str, cancel: &dyn Fn() -> bool) -> Result<(i32, String, String), String>` — (exit, stdout, stderr)
  - `sftp_service::is_remote_dir(profile: &Profile, path: &str) -> Result<bool, String>`
  - `remote_copy::shell_quote(s: &str) -> String`, `remote_copy::cp_command(src: &str, dst: &str, is_dir: bool) -> String`
  - `remote_copy::run(profile: &Profile, job_id: u64, src: &str, dst: &str, cancel: &dyn Fn() -> bool, on_progress: &dyn Fn(u64, u64, &str)) -> Result<(), String>`
  - `remote_copy::forget_profile(profile_id: &str)`, `remote_copy::wipe_tmp_root()`

- [ ] **Step 1: `TransferKind::RemoteCopy`**

Add `RemoteCopy,` to the enum in `models/transfer.rs`; extend the existing `kind_and_state_serialize_camel_case` test with `assert_eq!(serde_json::to_string(&TransferKind::RemoteCopy).unwrap(), "\"remoteCopy\"");`. In `commands/transfer.rs` add `"remoteCopy" => TransferKind::RemoteCopy,` and update the doc comment. In `src/types.ts` extend `kind` with `| "remoteCopy"`.

- [ ] **Step 2: SFTP exec + is_remote_dir**

Append to `sftp_service.rs`:

```rust
/// True when `path` is a directory (stat follows symlinks).
pub fn is_remote_dir(profile: &Profile, path: &str) -> Result<bool, String> {
    let session = connect(profile)?;
    let sftp = session.sftp().map_err(|e| format!("Failed to open SFTP channel: {}", e))?;
    let stat = sftp.stat(Path::new(path)).map_err(|e| format!("Cannot stat '{}': {}", path, e))?;
    Ok(stat.is_dir())
}

/// Run `cmd` on the server over an exec channel. Polls non-blocking so `cancel` is honoured
/// and long commands are not cut by the per-op timeout. Returns (exit status, stdout, stderr).
pub fn exec_command(
    profile: &Profile,
    cmd: &str,
    cancel: &dyn Fn() -> bool,
) -> Result<(i32, String, String), String> {
    let session = connect(profile)?;
    let mut channel = session
        .channel_session()
        .map_err(|e| format!("Cannot open exec channel: {}", e))?;
    channel.exec(cmd).map_err(|e| format!("Exec failed: {}", e))?;

    session.set_blocking(false);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if cancel() {
            session.set_blocking(true);
            let _ = channel.close();
            return Err(CANCELLED_ERROR.to_string());
        }
        let mut progressed = false;
        match channel.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => { out.extend_from_slice(&buf[..n]); progressed = true; }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => { session.set_blocking(true); return Err(format!("Exec read failed: {}", e)); }
        }
        match channel.stderr().read(&mut buf) {
            Ok(0) => {}
            Ok(n) => { err.extend_from_slice(&buf[..n]); progressed = true; }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => { session.set_blocking(true); return Err(format!("Exec read failed: {}", e)); }
        }
        if channel.eof() {
            break;
        }
        if !progressed {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    session.set_blocking(true);
    let _ = channel.wait_close();
    let status = channel.exit_status().unwrap_or(-1);
    Ok((status, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned()))
}
```

- [ ] **Step 3: `remote_copy.rs` (tests first for the pure parts)**

```rust
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
```

Signatures used above are verified: `sftp_service::download_directory` / `ftp_service::download_directory` / `ftp_service::download_file_to` take `&dyn Fn(u64, u64, &str)`; `sftp_service::download_file` / `upload_file` take `&dyn Fn(u64, u64)`.

- [ ] **Step 4: Queue dispatch, lifecycle hooks**

`transfer_queue::run_job` add arm:

```rust
        TransferKind::RemoteCopy => crate::services::remote_copy::run(
            &profile, job_id, &view.src, &view.dst, &cancel_fn, &|d, t, n| emit_progress(d, t, n),
        ),
```

`services/mod.rs`: `pub mod remote_copy;`.
`lib.rs`: in `.setup` before `transfer_queue::init` add `services::remote_copy::wipe_tmp_root();`; in `cleanup_on_exit` after `cancel_all()` add `services::remote_copy::wipe_tmp_root();`.
`commands/ssh.rs::stop_ssh_session`: add `crate::services::remote_copy::forget_profile(&profile_id);` (main.ts calls `stopSshSession` on every disconnect).

- [ ] **Step 5: Build + tests**

Run: `cd src-tauri && cargo build && cargo clippy && cargo test --lib`
Expected: clean; new tests pass (shell_quote, cp_command, file_name, serde remoteCopy).

- [ ] **Step 6: Commit**

```bash
git add -A src-tauri/src src/types.ts src/types.js
git commit -m "feat: remote copy job (server-side cp with download/upload fallback)"
```

---

### Task 10: Frontend "Copy to…"

**Files:**
- Modify: `src/components/file-browser.ts` (action button, context menus, handler), `src/i18n/*.ts`

**Interfaces:**
- Consumes: `enqueue("remoteCopy", src, dst, name)` (kind union from Task 9), `resolveOverwrite`, `resetOverwriteDecisions`
- Produces: `private async handleCopyTo(): Promise<void>`, `private async copyNamesToDir(sourceDir: string, names: string[], targetDir: string): Promise<void>` (used by Task 12 paste)

- [ ] **Step 1: Handler**

Add after `handleMoveTo`:

```ts
  private async handleCopyTo(): Promise<void> {
    if (!this.profileId || this.selectedNames.size === 0) return;
    const names = [...this.selectedNames];
    const label = names.length === 1 ? `"${names[0]}"` : t("fileBrowser.itemsLabel", { count: names.length });
    const targetDir = await showPrompt(
      t("fileBrowser.copyToTitle", { label }),
      t("fileBrowser.moveToPlaceholder"),
      this.currentPath,
    );
    if (!targetDir) return;
    await this.copyNamesToDir(this.currentPath, names, targetDir.replace(/(.)\/+$/, "$1"));
  }

  private async copyNamesToDir(sourceDir: string, names: string[], targetDir: string): Promise<void> {
    if (!this.profileId) return;
    this.resetOverwriteDecisions();
    let queued = 0;
    let skipped = 0;
    for (const name of names) {
      const from = joinPath(sourceDir, name);
      const to = joinPath(targetDir, name);
      if (from === to) { skipped++; continue; }
      if (to.startsWith(from + "/")) {
        this.status(t("fileBrowser.copyIntoItself", { name }), true);
        skipped++;
        continue;
      }
      try {
        if (!(await this.resolveOverwrite(to, name))) { skipped++; continue; }
      } catch (err) {
        if (String(err) === "Error: UPLOAD_CANCELLED") break;
      }
      this.log(t("fileBrowser.logCopying", { name, target: targetDir }));
      await this.enqueue("remoteCopy", from, to, name);
      queued++;
    }
    const parts: string[] = [];
    if (queued > 0) parts.push(t("fileBrowser.queuedCount", { count: queued }));
    if (skipped > 0) parts.push(t("fileBrowser.skippedCount", { count: skipped }));
    if (parts.length) this.status(parts.join(", "), false);
  }
```

- [ ] **Step 2: UI entries**

Add icon to `ICONS`: `copyTo: \`<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>\``.
Action bar after `#move-btn`: `<button id="copy-btn" ${!hasAny || this.busy ? "disabled" : ""} title="${t("fileBrowser.copyTo")}">${ICONS.copyTo}</button>` + listener `document.getElementById("copy-btn")?.addEventListener("click", () => this.handleCopyTo());`.
In `buildFileContextItems` and `buildFolderContextItems`, directly after the `moveTo` item: `{ icon: ICONS.copyTo, label: t("fileBrowser.copyTo"), action: () => this.handleCopyTo() },`.

- [ ] **Step 3: i18n**

| Key | EN | DE |
|---|---|---|
| `fileBrowser.copyTo` | Copy to… | Kopieren nach… |
| `fileBrowser.copyToTitle` | Copy {label} to directory | {label} in Verzeichnis kopieren |
| `fileBrowser.copyIntoItself` | Cannot copy "{name}" into itself | "{name}" kann nicht in sich selbst kopiert werden |
| `fileBrowser.logCopying` | Copying {name} → {target} | Kopiere {name} → {target} |
| `fileBrowser.itemsLabel` | {count} items | {count} Elemente |

In `src/components/transfer-queue.ts` (~L142) replace the arrow expression with:

```ts
        const arrow = j.kind === "remoteCopy" ? "⧉" : j.kind === "upload" || j.kind === "uploadDir" ? "↑" : "↓";
```

- [ ] **Step 4: Build + manual**

`npx tsc && npx vite build`. Manual on SSH host: copy file to other dir (fast, `cp`), copy folder into existing folder with same name → overwrite prompt → merged; copy folder into its own subfolder → error status; on an SFTP-only (chroot `internal-sftp`) account → fallback copies; on FTP profile → fallback copies; cancel a large fallback copy → no `~/.config/murmurssh/workspace/.copy-tmp/<id>` left.

- [ ] **Step 5: Commit**

```bash
git add src/components src/i18n
git commit -m "feat: Copy to… action for remote files and folders"
```

---

### Task 11: Shortcut registry, panel focus, help table

**Files:**
- Create: `src/shortcuts.ts`, `src/panel-focus.ts`
- Modify: `src/main.ts` (help dialog, global keys, panel focus wiring), `src/styles.css`, `src/i18n/*.ts` (remove hard-coded table from `app.helpBodyHtml`, add `shortcuts.*`)

**Interfaces:**
- Produces:
  - `export type ShortcutScope = "global" | "panels" | "remote" | "local" | "dialogs"`
  - `export interface Shortcut { id: string; keys: string[]; scope: ShortcutScope; displayOnly?: boolean }`
  - `export const SHORTCUTS: Shortcut[]`
  - `export function matchShortcut(e: KeyboardEvent, scope: ShortcutScope | ShortcutScope[]): string | null`
  - `export function shortcutHelpHtml(): string`
  - `panel-focus.ts`: `export type Panel = "local" | "remote"; export function getActivePanel(): Panel; export function setActivePanel(p: Panel): void; export function onActivePanelChange(cb: (p: Panel) => void): void`

- [ ] **Step 1: `panel-focus.ts`**

```ts
export type Panel = "local" | "remote";

let active: Panel = "remote";
const listeners: Array<(p: Panel) => void> = [];

export function getActivePanel(): Panel {
  return active;
}

export function setActivePanel(p: Panel): void {
  if (active === p) return;
  active = p;
  document.getElementById("file-browser")?.classList.toggle("panel--active", p === "remote");
  document.getElementById("local-file-browser")?.classList.toggle("panel--active", p === "local");
  listeners.forEach((cb) => cb(p));
}

export function onActivePanelChange(cb: (p: Panel) => void): void {
  listeners.push(cb);
}
```

- [ ] **Step 2: `shortcuts.ts`**

```ts
import { t } from "./i18n/index";

export type ShortcutScope = "global" | "panels" | "remote" | "local" | "dialogs";

export interface Shortcut {
  /** i18n key suffix: description is t(`shortcuts.${id}`) */
  id: string;
  /** Alternatives, e.g. ["Backspace", "Alt+ArrowUp"]. Modifiers: Ctrl, Shift, Alt. */
  keys: string[];
  scope: ShortcutScope;
  /** Listed in help but matched elsewhere (type-ahead, dialog Enter/Esc). */
  displayOnly?: boolean;
}

export const SHORTCUTS: Shortcut[] = [
  { id: "help", keys: ["F1", "?"], scope: "global" },
  { id: "switchPanel", keys: ["Tab"], scope: "global" },

  { id: "cursorUp", keys: ["ArrowUp"], scope: "panels" },
  { id: "cursorDown", keys: ["ArrowDown"], scope: "panels" },
  { id: "extendUp", keys: ["Shift+ArrowUp"], scope: "panels" },
  { id: "extendDown", keys: ["Shift+ArrowDown"], scope: "panels" },
  { id: "first", keys: ["Home"], scope: "panels" },
  { id: "last", keys: ["End"], scope: "panels" },
  { id: "pageUp", keys: ["PageUp"], scope: "panels" },
  { id: "pageDown", keys: ["PageDown"], scope: "panels" },
  { id: "open", keys: ["Enter"], scope: "panels" },
  { id: "parent", keys: ["Backspace", "Alt+ArrowUp"], scope: "panels" },
  { id: "focusPath", keys: ["Ctrl+L"], scope: "panels" },
  { id: "refresh", keys: ["F5"], scope: "panels" },
  { id: "rename", keys: ["F2"], scope: "panels" },
  { id: "newFolder", keys: ["F7", "Ctrl+Shift+N"], scope: "panels" },
  { id: "delete", keys: ["Delete"], scope: "panels" },
  { id: "selectAll", keys: ["Ctrl+A"], scope: "panels" },
  { id: "clearSelection", keys: ["Escape"], scope: "panels" },
  { id: "typeAhead", keys: ["a–z, 0–9"], scope: "panels", displayOnly: true },

  { id: "newFile", keys: ["Ctrl+N"], scope: "remote" },
  { id: "moveTo", keys: ["F6"], scope: "remote" },
  { id: "copyTo", keys: ["Ctrl+Shift+C"], scope: "remote" },
  { id: "clipCopy", keys: ["Ctrl+C"], scope: "remote" },
  { id: "clipCut", keys: ["Ctrl+X"], scope: "remote" },
  { id: "clipPaste", keys: ["Ctrl+V"], scope: "remote" },
  { id: "download", keys: ["Ctrl+D"], scope: "remote" },
  { id: "terminal", keys: ["F11"], scope: "remote" },

  { id: "upload", keys: ["Ctrl+U"], scope: "local" },

  { id: "dialogConfirm", keys: ["Enter"], scope: "dialogs", displayOnly: true },
  { id: "dialogCancel", keys: ["Escape"], scope: "dialogs", displayOnly: true },
];

function parse(spec: string): { key: string; ctrl: boolean; shift: boolean; alt: boolean } {
  const parts = spec.split("+");
  const key = parts.pop()!;
  return { key, ctrl: parts.includes("Ctrl"), shift: parts.includes("Shift"), alt: parts.includes("Alt") };
}

function keyMatches(e: KeyboardEvent, spec: string): boolean {
  const p = parse(spec);
  const isSymbol = p.key.length === 1 && !/[a-z0-9]/i.test(p.key);
  const evKey = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  const specKey = p.key.length === 1 ? p.key.toLowerCase() : p.key;
  if (evKey !== specKey) return false;
  if ((e.ctrlKey || e.metaKey) !== p.ctrl) return false;
  if (e.altKey !== p.alt) return false;
  // Symbols like "?" need Shift on most layouts — ignore Shift for them.
  if (!isSymbol && e.shiftKey !== p.shift) return false;
  return true;
}

/** Id of the first non-display-only shortcut in `scope` matching `e`, else null. */
export function matchShortcut(e: KeyboardEvent, scope: ShortcutScope | ShortcutScope[]): string | null {
  const scopes = Array.isArray(scope) ? scope : [scope];
  for (const s of SHORTCUTS) {
    if (s.displayOnly || !scopes.includes(s.scope)) continue;
    if (s.keys.some((k) => keyMatches(e, k))) return s.id;
  }
  return null;
}

function keyLabel(spec: string): string {
  return spec
    .split("+")
    .map((k) => t(`shortcuts.key_${k}`) !== `shortcuts.key_${k}` ? t(`shortcuts.key_${k}`) : k)
    .map((k) => `<kbd>${k}</kbd>`)
    .join("+");
}

/** Help-dialog table generated from SHORTCUTS — the only place shortcuts are documented. */
export function shortcutHelpHtml(): string {
  const order: ShortcutScope[] = ["global", "panels", "remote", "local", "dialogs"];
  return order
    .map((scope) => {
      const rows = SHORTCUTS.filter((s) => s.scope === scope)
        .map((s) => `<tr><td class="help-keys">${s.keys.map(keyLabel).join(" / ")}</td><td>${t(`shortcuts.${s.id}`)}</td></tr>`)
        .join("");
      return `<p><strong>${t(`shortcuts.scope_${scope}`)}</strong></p><table class="help-shortcuts">${rows}</table>`;
    })
    .join("");
}
```

`t()` returns the key string itself for missing keys (verified in `src/i18n/index.ts`), so `keyLabel` falls back to the raw key name (`F5`, `A`, `?`) for keys without a `shortcuts.key_*` entry.

- [ ] **Step 3: Help dialog uses the generated table**

In every locale remove from `app.helpBodyHtml` the `<p><strong>Keyboard shortcuts (file browser):</strong></p>` paragraph and its `<table>`. In `main.ts` `showHelpDialog` insert `${shortcutHelpHtml()}` directly after `${t("app.helpBodyHtml")}` (import from `./shortcuts`). CSS:

```css
.help-shortcuts { border-collapse: collapse; font-size: 13px; width: 100%; margin-bottom: 8px; }
.help-shortcuts td { padding: 2px 8px 2px 0; vertical-align: top; }
.help-keys { white-space: nowrap; }
.panel--active { box-shadow: inset 0 2px 0 var(--accent); }
```

- [ ] **Step 4: Global keys + panel wiring in `main.ts`**

```ts
import { matchShortcut } from "./shortcuts";
import { getActivePanel, setActivePanel } from "./panel-focus";

document.addEventListener("keydown", (e) => {
  if (document.querySelector(".modal-overlay")) return;
  const tag = (document.activeElement as HTMLElement)?.tagName?.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return;
  const id = matchShortcut(e, "global");
  if (id === "help") { e.preventDefault(); void showHelpDialog(); }
  else if (id === "switchPanel" && connectedProfileId && localBrowserEl && !localBrowserEl.hasAttribute("hidden")) {
    e.preventDefault();
    setActivePanel(getActivePanel() === "remote" ? "local" : "remote");
  }
});

document.getElementById("file-browser")?.addEventListener("mousedown", () => setActivePanel("remote"));
document.getElementById("local-file-browser")?.addEventListener("mousedown", () => setActivePanel("local"));
```

In `setLocalBrowserVisible(visible)` (`main.ts` ~L151, toggles the `hidden` attribute) add `if (!visible) setActivePanel("remote");` at the end. Register the global listener BEFORE `new FileBrowser(...)` so Tab/F1 are resolved first; the type-ahead in FileBrowser must ignore `?` when `matchShortcut(e, "global")` is non-null (Task 12).

- [ ] **Step 5: i18n `shortcuts` section (all 6 locales)**

| Key | EN | DE |
|---|---|---|
| `scope_global` | General | Allgemein |
| `scope_panels` | Both file panels | Beide Dateibereiche |
| `scope_remote` | Remote panel | Remote-Bereich |
| `scope_local` | Local panel | Lokaler Bereich |
| `scope_dialogs` | Dialogs | Dialoge |
| `help` | Show help and shortcuts | Hilfe und Tastenkürzel anzeigen |
| `switchPanel` | Switch between local and remote panel | Zwischen lokalem und Remote-Bereich wechseln |
| `cursorUp` | Select previous entry | Vorherigen Eintrag wählen |
| `cursorDown` | Select next entry | Nächsten Eintrag wählen |
| `extendUp` | Extend selection upwards | Auswahl nach oben erweitern |
| `extendDown` | Extend selection downwards | Auswahl nach unten erweitern |
| `first` | Jump to first entry | Zum ersten Eintrag |
| `last` | Jump to last entry | Zum letzten Eintrag |
| `pageUp` | One page up | Eine Seite nach oben |
| `pageDown` | One page down | Eine Seite nach unten |
| `open` | Open folder / edit file | Ordner öffnen / Datei bearbeiten |
| `parent` | Go to parent folder | Zum übergeordneten Ordner |
| `focusPath` | Edit path | Pfad bearbeiten |
| `refresh` | Refresh listing | Liste aktualisieren |
| `rename` | Rename selected entry | Ausgewählten Eintrag umbenennen |
| `newFolder` | New folder | Neuer Ordner |
| `delete` | Delete selected entries | Ausgewählte Einträge löschen |
| `selectAll` | Select all entries | Alle Einträge auswählen |
| `clearSelection` | Close menu / clear selection | Menü schließen / Auswahl aufheben |
| `typeAhead` | Type a name to jump to it | Namen tippen, um hinzuspringen |
| `newFile` | New file | Neue Datei |
| `moveTo` | Move selection to… | Auswahl verschieben nach… |
| `copyTo` | Copy selection to… | Auswahl kopieren nach… |
| `clipCopy` | Mark selection for copying | Auswahl zum Kopieren merken |
| `clipCut` | Mark selection for moving | Auswahl zum Verschieben merken |
| `clipPaste` | Paste marked entries into current folder | Gemerkte Einträge in aktuellen Ordner einfügen |
| `download` | Download selection into the local panel's folder | Auswahl in den Ordner des lokalen Bereichs herunterladen |
| `terminal` | Open SSH terminal | SSH-Terminal öffnen |
| `upload` | Upload selection into the remote panel's folder | Auswahl in den Ordner des Remote-Bereichs hochladen |
| `dialogConfirm` | Confirm dialog | Dialog bestätigen |
| `dialogCancel` | Cancel / close dialog | Dialog abbrechen / schließen |
| `key_Ctrl` | Ctrl | Strg |
| `key_Shift` | Shift | Umschalt |
| `key_Alt` | Alt | Alt |
| `key_Delete` | Delete | Entf |
| `key_Backspace` | Backspace | Rücktaste |
| `key_Escape` | Esc | Esc |
| `key_Enter` | Enter | Enter |
| `key_Tab` | Tab | Tab |
| `key_Home` | Home | Pos1 |
| `key_End` | End | Ende |
| `key_PageUp` | Page Up | Bild ↑ |
| `key_PageDown` | Page Down | Bild ↓ |
| `key_ArrowUp` | ↑ | ↑ |
| `key_ArrowDown` | ↓ | ↓ |

- [ ] **Step 6: Build + manual**

`npx tsc && npx vite build`. Manual: F1 and `?` open help; the table lists all groups; switching language re-renders texts; Tab toggles the accent line between panels (only when local panel visible).

- [ ] **Step 7: Commit**

```bash
git add src/shortcuts.ts src/shortcuts.js src/panel-focus.ts src/panel-focus.js src/main.ts src/main.js src/styles.css src/i18n
git commit -m "feat: shortcut registry, panel focus and generated help table"
```

---

### Task 12: Remote panel keyboard

**Files:**
- Modify: `src/components/file-browser.ts` (`setupKeyboardShortcuts`, `moveNamesToDir`, new clipboard + download-to-local provider), `src/main.ts` (provide local dir)

**Interfaces:**
- Consumes: `matchShortcut`, `getActivePanel`, `moveCursor` (Task 7), `copyNamesToDir` (Task 10)
- Produces: `FileBrowser.setLocalDirProvider(fn: () => string | null): void`; `moveNamesToDir(names, targetDir, sourceDir = this.currentPath)`

- [ ] **Step 1: Generalize move source**

Change signature to `private async moveNamesToDir(names: string[], targetDir: string, sourceDir: string = this.currentPath)` and use `joinPath(sourceDir, name)` for `fromPath`.

- [ ] **Step 2: Replace `setupKeyboardShortcuts`**

```ts
  private clipboard: { mode: "copy" | "cut"; profileId: string; dir: string; names: string[] } | null = null;
  private cursorName: string | null = null;
  private localDirProvider: (() => string | null) | null = null;

  setLocalDirProvider(fn: () => string | null): void {
    this.localDirProvider = fn;
  }

  private applyKeyboardSelection(st: SelectionState): void {
    this.selectedNames = st.selected;
    this.anchorName = st.anchor;
    this.cursorName = st.cursor;
    this.render();
    this.container
      .querySelector<HTMLElement>(`tr.file-entry[data-name="${CSS.escape(this.cursorName ?? "")}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }

  private setupKeyboardShortcuts(): void {
    document.addEventListener("keydown", (e) => {
      if (document.querySelector(".modal-overlay")) return;
      if (getActivePanel() !== "remote") return;
      const tag = (document.activeElement as HTMLElement)?.tagName?.toLowerCase();
      if (tag === "input" || tag === "textarea" || tag === "select") return;
      if (!this.profileId) return;

      const id = matchShortcut(e, ["panels", "remote"]);
      if (!id) {
        if (!e.ctrlKey && !e.altKey && !e.metaKey && e.key.length === 1 && /\S/.test(e.key)
            && !this.busy && !matchShortcut(e, "global")) {
          e.preventDefault();
          this.handleTypeAhead(e.key);
        }
        return;
      }
      if (this.busy && id !== "clearSelection") return;

      const names = this.entries.map((x) => x.name);
      const st = (): SelectionState => ({ selected: this.selectedNames, anchor: this.anchorName, cursor: this.cursorName });
      const page = Math.max(1, Math.floor((this.container.querySelector<HTMLElement>(".file-browser__scroll")?.clientHeight ?? 300) / 24) - 1);
      const one = this.selectedNames.size === 1;
      const any = this.selectedNames.size > 0;

      const run: Record<string, () => void> = {
        cursorUp: () => this.applyKeyboardSelection(moveCursor(names, st(), -1, false)),
        cursorDown: () => this.applyKeyboardSelection(moveCursor(names, st(), 1, false)),
        extendUp: () => this.applyKeyboardSelection(moveCursor(names, st(), -1, true)),
        extendDown: () => this.applyKeyboardSelection(moveCursor(names, st(), 1, true)),
        first: () => this.applyKeyboardSelection(moveCursor(names, st(), "start", false)),
        last: () => this.applyKeyboardSelection(moveCursor(names, st(), "end", false)),
        pageUp: () => this.applyKeyboardSelection(moveCursor(names, st(), -page, false)),
        pageDown: () => this.applyKeyboardSelection(moveCursor(names, st(), page, false)),
        open: () => {
          const entry = this.selectedEntry;
          if (!entry) return;
          if (entry.is_dir) void this.navigateInto(entry.name);
          else void this.handleEdit();
        },
        parent: () => this.navigateUp(),
        focusPath: () => {
          const input = this.container.querySelector<HTMLInputElement>("#path-input");
          input?.focus();
          input?.select();
        },
        refresh: () => void this.refresh(),
        rename: () => { if (one) void this.handleRename(); },
        newFolder: () => void this.handleNewFolder(),
        newFile: () => void this.handleNewFile(),
        delete: () => { if (any) void this.handleDelete(); },
        selectAll: () => { names.forEach((n) => this.selectedNames.add(n)); this.render(); },
        clearSelection: () => {
          const ctx = document.getElementById("ctx-menu");
          if (ctx) { ctx.remove(); return; }
          if (any) { this.clearSelection(); this.render(); }
        },
        moveTo: () => { if (any) void this.handleMoveTo(); },
        copyTo: () => { if (any) void this.handleCopyTo(); },
        clipCopy: () => this.setClipboard("copy"),
        clipCut: () => this.setClipboard("cut"),
        clipPaste: () => void this.pasteClipboard(),
        download: () => {
          if (!any) return;
          const dir = this.localDirProvider?.() ?? null;
          void this.downloadNamesToLocal([...this.selectedNames], dir ?? undefined);
        },
        terminal: () => { if (!this.protocol || this.protocol === "ssh") void this.handleTerminal(); },
      };
      const action = run[id];
      if (!action) return;
      e.preventDefault();
      action();
    });
  }

  private setClipboard(mode: "copy" | "cut"): void {
    if (!this.profileId || this.selectedNames.size === 0) return;
    this.clipboard = { mode, profileId: this.profileId, dir: this.currentPath, names: [...this.selectedNames] };
    this.status(t(mode === "copy" ? "fileBrowser.clipCopied" : "fileBrowser.clipCut", { count: this.clipboard.names.length }), false);
  }

  private async pasteClipboard(): Promise<void> {
    const clip = this.clipboard;
    if (!clip || !this.profileId || clip.profileId !== this.profileId) return;
    if (clip.mode === "copy") {
      await this.copyNamesToDir(clip.dir, clip.names, this.currentPath);
    } else {
      if (clip.dir === this.currentPath) return;
      this.clipboard = null; // a moved item cannot be pasted twice
      await this.moveNamesToDir(clip.names, this.currentPath, clip.dir);
    }
  }
```

Imports: `import { matchShortcut } from "../shortcuts"; import { getActivePanel } from "../panel-focus"; import { moveCursor, type SelectionState } from "./list-selection";`. Set `this.cursorName` on mouse click too (in the existing click handler set `this.cursorName = name`), clear it in `clearSelection()`, and add `file-entry--cursor` class in `render()` rows when `entry.name === this.cursorName`. Clear `this.clipboard` in `handleDisconnect()`.

- [ ] **Step 3: Local dir provider wiring (`main.ts`)**

```ts
fileBrowser.setLocalDirProvider(() => {
  const visible = !!localBrowserEl && !localBrowserEl.hasAttribute("hidden");
  const p = localBrowser.getCurrentPath();
  return visible && p ? p : null;
});
```

After the download completes, the local panel should show the new files: wrap as in the existing `localBrowser.onDownload` wiring — in `run.download` call `void this.downloadNamesToLocal(...).then(() => this.onLocalDownloadDone?.())` and expose `onLocalDownloadDone(cb: () => void)` which `main.ts` sets to `() => void localBrowser.refresh()`.

- [ ] **Step 4: i18n**

| Key | EN | DE |
|---|---|---|
| `fileBrowser.clipCopied` | {count} marked for copying — Ctrl+V pastes | {count} zum Kopieren gemerkt — Strg+V fügt ein |
| `fileBrowser.clipCut` | {count} marked for moving — Ctrl+V pastes | {count} zum Verschieben gemerkt — Strg+V fügt ein |

- [ ] **Step 5: Build + manual**

`npx tsc && npx vite build`. Manual (remote panel active): ↑/↓/Shift/Home/End/PgUp/PgDn move selection and scroll; Enter opens; Backspace + Alt+↑ go up; Ctrl+L focuses path; F6 move prompt; Ctrl+Shift+C copy prompt; Ctrl+C → other folder → Ctrl+V copies; Ctrl+X → Ctrl+V moves; Ctrl+D downloads into the local panel dir (without local panel: save dialog as before); F7/Ctrl+Shift+N new folder; Ctrl+N new file; `?` opens help instead of type-ahead; all keys inert while the local panel is active.

- [ ] **Step 6: Commit**

```bash
git add src/components src/main.ts src/main.js src/i18n
git commit -m "feat: full keyboard navigation and clipboard in remote panel"
```

---

### Task 13: Local panel keyboard + local create/delete

**Files:**
- Modify: `src-tauri/src/services/local_service.rs`, `src-tauri/src/commands/local.rs`, `src-tauri/src/lib.rs`, `src/api/index.ts`, `src/components/local-file-browser.ts`, `src/main.ts`, `src/i18n/*.ts`

**Interfaces:**
- Produces: Rust `local_service::create_local_dir(path: &str) -> Result<(), String>`, `local_service::delete_local_path(path: &str) -> Result<(), String>`; commands `create_local_dir(path)`, `delete_local_path(path)`; TS `api.createLocalDir(path)`, `api.deleteLocalPath(path)`; `LocalFileBrowser.onUploadSelection` reuses `onUpload(cb)`.

- [ ] **Step 1: Backend with tests**

```rust
/// Create a new directory. Parent must exist; the path must be absolute and new.
pub fn create_local_dir(path: &str) -> Result<(), String> {
    reject_null_bytes(path)?;
    if !path.starts_with('/') {
        return Err("Only absolute paths are accepted".to_string());
    }
    let p = Path::new(path);
    if p.exists() {
        return Err(format!("'{}' already exists", path));
    }
    std::fs::create_dir(p).map_err(|e| format!("Create folder failed: {}", e))
}

/// Delete a file, a symlink (never its target) or a directory recursively.
pub fn delete_local_path(path: &str) -> Result<(), String> {
    reject_null_bytes(path)?;
    if !path.starts_with('/') {
        return Err("Only absolute paths are accepted".to_string());
    }
    let p = Path::new(path);
    if p.parent().is_none() || p == Path::new(&get_home_dir()) {
        return Err("Refusing to delete this path".to_string());
    }
    let meta = std::fs::symlink_metadata(p).map_err(|e| format!("'{}': {}", path, e))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    }
    .map_err(|e| format!("Delete failed: {}", e))
}

#[cfg(test)]
mod local_ops_tests {
    use super::*;

    #[test]
    fn create_and_delete_local_dir_tree() {
        let root = std::env::temp_dir().join(format!("murmur-localops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dir = root.join("new");
        let dir_s = dir.to_string_lossy().to_string();
        create_local_dir(&dir_s).unwrap();
        assert!(create_local_dir(&dir_s).is_err(), "second create must fail");
        std::fs::write(dir.join("f.txt"), b"x").unwrap();
        delete_local_path(&dir_s).unwrap();
        assert!(!dir.exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn delete_symlink_keeps_target() {
        let root = std::env::temp_dir().join(format!("murmur-localops-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::os::unix::fs::symlink(root.join("target"), root.join("link")).unwrap();
        delete_local_path(&root.join("link").to_string_lossy()).unwrap();
        assert!(root.join("target").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_relative_and_root() {
        assert!(delete_local_path("relative").is_err());
        assert!(delete_local_path("/").is_err());
        assert!(create_local_dir("rel").is_err());
    }
}
```

Commands in `commands/local.rs`:

```rust
#[tauri::command]
pub fn create_local_dir(path: String) -> Result<(), String> {
    local_service::create_local_dir(&path)
}

#[tauri::command]
pub fn delete_local_path(path: String) -> Result<(), String> {
    local_service::delete_local_path(&path)
}
```

Register both in `lib.rs` after `commands::local::open_local_file`. Run `cd src-tauri && cargo test --lib local_ops_tests` → 3 passed.

- [ ] **Step 2: TS api**

```ts
export async function createLocalDir(path: string): Promise<void> {
  return invoke("create_local_dir", { path });
}

export async function deleteLocalPath(path: string): Promise<void> {
  return invoke("delete_local_path", { path });
}
```

- [ ] **Step 3: Local keyboard handler**

In `LocalFileBrowser` constructor call `this.setupKeyboardShortcuts();` and add:

```ts
  private typeAheadBuffer = "";
  private typeAheadTimer: number | null = null;

  private setupKeyboardShortcuts(): void {
    document.addEventListener("keydown", (e) => {
      if (document.querySelector(".modal-overlay")) return;
      if (getActivePanel() !== "local" || !this.profileId) return;
      const tag = (document.activeElement as HTMLElement)?.tagName?.toLowerCase();
      if (tag === "input" || tag === "textarea" || tag === "select") return;

      const id = matchShortcut(e, ["panels", "local"]);
      if (!id) {
        if (!e.ctrlKey && !e.altKey && !e.metaKey && e.key.length === 1 && /\S/.test(e.key)
            && !this.busy && !matchShortcut(e, "global")) {
          e.preventDefault();
          this.typeAhead(e.key);
        }
        return;
      }
      if (this.busy) return;

      const names = this.entries.map((x) => x.name);
      const st = (): SelectionState => ({ selected: this.selectedNames, anchor: this.anchorName, cursor: this.cursorName });
      const page = Math.max(1, Math.floor((this.container.querySelector<HTMLElement>(".local-browser__scroll")?.clientHeight ?? 300) / 24) - 1);
      const selected = this.entries.filter((x) => this.selectedNames.has(x.name));

      const run: Record<string, () => void> = {
        cursorUp: () => this.applySelection(moveCursor(names, st(), -1, false)),
        cursorDown: () => this.applySelection(moveCursor(names, st(), 1, false)),
        extendUp: () => this.applySelection(moveCursor(names, st(), -1, true)),
        extendDown: () => this.applySelection(moveCursor(names, st(), 1, true)),
        first: () => this.applySelection(moveCursor(names, st(), "start", false)),
        last: () => this.applySelection(moveCursor(names, st(), "end", false)),
        pageUp: () => this.applySelection(moveCursor(names, st(), -page, false)),
        pageDown: () => this.applySelection(moveCursor(names, st(), page, false)),
        open: () => {
          if (selected.length !== 1) return;
          const p = joinPath(this.currentPath, selected[0].name);
          if (selected[0].is_dir) void this.navigateTo(p);
          else void this._ctxOpen(p, this.editorCommand, true);
        },
        parent: () => { if (this.currentPath !== "/") void this.navigateTo(parentPath(this.currentPath)); },
        focusPath: () => {
          const input = this.container.querySelector<HTMLInputElement>("#lb-path-input");
          input?.focus();
          input?.select();
        },
        refresh: () => void this.refresh(),
        rename: () => { if (selected.length === 1) void this._ctxRename(selected[0].name); },
        newFolder: () => void this.createFolder(),
        delete: () => { if (selected.length > 0) void this.deleteSelected(); },
        selectAll: () => this.applySelection({ selected: new Set(names), anchor: names[0] ?? null, cursor: this.cursorName }),
        clearSelection: () => {
          if (this._contextMenu) { this._hideContextMenu(); return; }
          this.applySelection({ selected: new Set(), anchor: null, cursor: null });
        },
        upload: () => {
          const paths = this.getSelectedPaths();
          if (paths.length > 0 && this.onUploadCallback) void this.onUploadCallback(paths, paths.length === 1 ? selected[0].name : `${paths.length}`);
        },
      };
      const action = run[id];
      if (!action) return;
      e.preventDefault();
      action();
    });
  }

  private typeAhead(ch: string): void {
    this.typeAheadBuffer += ch.toLowerCase();
    if (this.typeAheadTimer !== null) window.clearTimeout(this.typeAheadTimer);
    this.typeAheadTimer = window.setTimeout(() => { this.typeAheadBuffer = ""; this.typeAheadTimer = null; }, 800);
    const match = this.entries.find((x) => x.name.toLowerCase().startsWith(this.typeAheadBuffer));
    if (match) this.applySelection({ selected: new Set([match.name]), anchor: match.name, cursor: match.name });
  }

  private async createFolder(): Promise<void> {
    const name = await showPrompt(t("localBrowser.newFolderTitle"), t("localBrowser.newFolderPlaceholder"));
    if (!name) return;
    if (name.includes("/")) { this.inlineError = t("localBrowser.nameContainsSlash"); this.render(); return; }
    try {
      await api.createLocalDir(joinPath(this.currentPath, name));
      await this.refresh();
    } catch (err) {
      this.inlineError = t("localBrowser.createFolderFailed", { error: String(err) });
      this.render();
    }
  }

  private async deleteSelected(): Promise<void> {
    const paths = this.getSelectedPaths();
    const label = paths.length === 1 ? paths[0] : t("localBrowser.itemsLabel", { count: paths.length });
    const ok = await showConfirm(t("localBrowser.deleteConfirmMsg", { label }), t("localBrowser.deleteConfirmTitle"));
    if (!ok) return;
    const failed: string[] = [];
    for (const p of paths) {
      try { await api.deleteLocalPath(p); } catch (err) { failed.push(`${p}: ${String(err)}`); }
    }
    await this.refresh();
    if (failed.length) { this.inlineError = t("localBrowser.deleteFailed", { error: failed.slice(0, 2).join("; ") }); this.render(); }
  }
```

Imports: `showPrompt`, `showConfirm` from `./dialog`; `matchShortcut` from `../shortcuts`; `getActivePanel` from `../panel-focus`; `moveCursor, clickSelect, type SelectionState` from `./list-selection`.
Replace `window.prompt` in `_ctxRename` with `await showPrompt(t("localBrowser.renameTitle"), "", oldName)` (native prompts block the webview and ignore the modal key handler).
Add to the local context menu: `<button data-action="newFolder">${t("localBrowser.ctxNewFolder")}</button>` and `<button data-action="delete">${t("localBrowser.ctxDelete")}</button>`, dispatching to `createFolder()` / select the row + `deleteSelected()`.

- [ ] **Step 4: Ctrl+U confirmation wiring**

The existing `localBrowser.onUpload` in `main.ts` shows a confirm with `{name}`; keep it (Enter confirms via modal keys) and pass the label built above.

- [ ] **Step 5: i18n (localBrowser.*)**

| Key | EN | DE |
|---|---|---|
| `newFolderTitle` | New folder | Neuer Ordner |
| `newFolderPlaceholder` | Folder name | Ordnername |
| `nameContainsSlash` | Name cannot contain "/" | Name darf kein "/" enthalten |
| `createFolderFailed` | Create folder failed: {error} | Ordner anlegen fehlgeschlagen: {error} |
| `itemsLabel` | {count} items | {count} Elemente |
| `deleteConfirmTitle` | Delete locally | Lokal löschen |
| `deleteConfirmMsg` | Permanently delete {label} from this computer? Folders are deleted with all contents. | {label} endgültig von diesem Rechner löschen? Ordner werden mit gesamtem Inhalt gelöscht. |
| `deleteFailed` | Delete failed: {error} | Löschen fehlgeschlagen: {error} |
| `ctxNewFolder` | New folder | Neuer Ordner |
| `ctxDelete` | Delete | Löschen |

- [ ] **Step 6: Build + manual**

`cd src-tauri && cargo build && cargo clippy && cargo test --lib`; `npx tsc && npx vite build`.
Manual (Tab to local panel): arrows/Shift/Home/End/PgUp/PgDn; Enter opens folder / file in editor; Backspace up; Ctrl+L path; F5; F2 rename via in-app prompt; F7 folder; Delete → confirm → removed; Ctrl+A; Esc; type-ahead; Ctrl+U → confirm → queued upload into remote dir; no remote action fires while local is active. Open help → every key used above is listed.

- [ ] **Step 7: Commit**

```bash
git add -A src-tauri/src src/api src/components src/main.ts src/main.js src/i18n
git commit -m "feat: keyboard navigation, new folder and delete in local panel"
```

---

### Task 14: Docs, full validation

**Files:**
- Modify: `CLAUDE.md` (Phases Complete entry), `README.md` (features: Copy to, terminal selection, keyboard), `CHANGELOG.md` (Unreleased section)

- [ ] **Step 1: Shortcut audit** — `grep -n "e.key ===\|case \"" src/components/*.ts src/main.ts` (exclude `.js`): every key handled outside `modal-keys.ts`, path inputs (Enter/Escape inside `#path-input`/`#lb-path-input`) and `shortcuts.ts` must appear in `SHORTCUTS`. Add missing ones (with i18n) before continuing. Add `pathEnter` ("Enter in path field: go to path") and `pathEscape` ("Esc in path field: reset") to `SHORTCUTS` under `panels` with `displayOnly: true`, with i18n in all locales.
- [ ] **Step 2: Full validation** — `cd src-tauri && cargo build && cargo clippy -- -D warnings && cargo test --lib` and `npx tsc && npx vite build`; `grep -rn crispy-tools . --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git` → no hits.
- [ ] **Step 3: Docs** — CLAUDE.md phase entry summarizing tasks 1–13 (files, new commands `list_terminals`/`create_local_dir`/`delete_local_path`, job kind `remoteCopy`, settings `terminal`/`terminal_custom_command`); README feature bullets; CHANGELOG Unreleased.
- [ ] **Step 4: Commit** — `git commit -m "doc: copy-to, terminal selection, keyboard shortcuts"`
- [ ] **Step 5: Ask the user** whether to bump version and release (one release for everything vs. none yet).
