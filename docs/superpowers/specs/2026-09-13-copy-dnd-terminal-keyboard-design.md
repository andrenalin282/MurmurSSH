# Copy-to, Folder DnD, Terminal Selection, Safe Upload, Keyboard — Design

Date: 2026-09-13
Status: approved 2026-09-13

Five independent sub-projects, delivered in the order below, each its own commit
(frontend `.ts` + regenerated `.js` siblings, i18n in all 6 locales).

Decisions taken with the user (2026-09-13):
- Copy: server-side `cp` via SSH exec, fallback download+upload through the queue.
- Terminal: auto-detection + Settings choice (detected terminal or custom command).
- Keyboard: hybrid scheme — existing keys stay, new navigation/transfer keys added.
- Upload: safe overwrite via temp file + rename; per-file errors skip instead of abort.

---

## A. Safe folder/file upload (backend)

### Findings (code review)
1. Recursive upload creates all subfolders and uploads all files; existing files are
   overwritten (`sftp.create` truncates). Existing folder → one prompt for the whole folder.
2. **Data loss:** on error/cancel the partial-file cleanup (`sftp.unlink` in
   `sftp_service.rs` L313 single file, L733 directory; `ftp.rm` in `ftp_service.rs`
   L207, L361) deletes the target path — when that path was an existing file being
   overwritten, the previous remote version is gone.
3. One failing file aborts the rest of the folder (`?` propagation).
4. Symlink to an ancestor directory → infinite recursion (`is_dir()` follows links).

### Design
- New helper per protocol: upload into `<dir>/.<name>.murmur-part`, on success rename to
  the final name. SFTP: try `rename` with `OVERWRITE|ATOMIC|NATIVE` flags; if the server
  refuses because the target exists, `unlink(target)` then `rename` (the original stays
  intact until the new content is fully uploaded). FTP: `rm` target (ignore not-found),
  then `rename`.
- Cleanup on error/cancel only ever removes the `.murmur-part` file.
- Used by single-file upload and both directory uploads.
- Directory upload: per-file errors are collected (`Vec<String>`), remaining entries
  continue; at the end the job fails with a summary `"N of M files failed: a: …; b: …"`
  if any failed. Cancel still stops immediately.
- Loop guard: track canonicalized local directory paths on the current recursion stack
  (`HashSet<PathBuf>`); a directory already on the stack is skipped.
- Prompt behaviour unchanged (one prompt per top-level item).

### Tests
Unit tests for the part-name helper and the loop guard (local temp dir with a symlink
cycle, recursion walker extracted so it can run against a fake sink).
Manual: upload over existing folder, cancel mid-file → original remote file still present.

---

## B. Dialogs close with Enter / Esc (frontend)

Today only `showPrompt` handles keys. Affected: `dialog.ts` (confirm, overwrite,
permissions), `credential-dialog.ts`, `profile-form.ts`, `settings-dialog.ts`,
`update-dialog.ts`, help dialog in `main.ts`, file-browser modals.

### Design
- New `src/components/modal-keys.ts`: `installModalKeyHandler()` registers ONE
  `document` keydown listener (capture phase), called once from `main.ts`.
- It targets the topmost `.modal-overlay` (last in DOM order):
  - `Escape` → click `[data-modal-cancel]` in that overlay (if present).
  - `Enter` → click `[data-modal-primary]`, unless focus is in a `<textarea>`, a
    `<select>`, or on a `<button>` (a focused button keeps native Enter = click that button).
  - Handled events: `preventDefault()` + `stopPropagation()` so file-browser shortcuts
    never see them.
- Every modal marks its buttons with `data-modal-cancel` / `data-modal-primary`
  (overwrite dialog: Yes = primary, Cancel = cancel; host-key dialog: "accept once" is
  NOT primary — Enter there does nothing to avoid accidental trust; Esc = cancel).
- `showPrompt`'s own input handler is removed in favour of the central one.
- Focus: each dialog focuses its first input, or the primary button if none.

---

## C. Terminal selection (backend + Settings)

### Cause
`ssh_service::launch_ssh` hard-codes `Command::new("x-terminal-emulator")`, which only
exists on Debian/Ubuntu.

### Passphrase / password
Terminal-independent: the terminal runs `bash -c TERMINAL_SCRIPT -- ssh …`; `ssh -i key`
prompts for the passphrase inside the terminal, the password ControlMaster options are
plain ssh args. Only the exec flag differs per terminal.

### Design
- New `services/terminal_service.rs`:
  - `KNOWN_TERMINALS: &[(id, binary, exec_prefix)]` —
    `x-terminal-emulator -e`, `gnome-terminal --`, `ptyxis --`, `kgx --`, `konsole -e`,
    `xfce4-terminal -x`, `kitty` (no flag), `alacritty -e`, `foot` (no flag),
    `wezterm start --`, `xterm -e`.
  - `detect() -> Vec<String>` — ids whose binary is found on `$PATH`.
  - `resolve(settings) -> Result<(String, Vec<String>), String>`:
    - `terminal = "custom"` → whitespace-split `terminal_custom_command`; first token is
      the program, rest are prefix args (lc-debt: no shell-quote parsing).
    - `terminal = <known id>` → that entry.
    - `"auto"`/unset → `$TERMINAL` (known basename → its flag, else `-e`), then the
      known list in order, first found on `$PATH`.
    - nothing found → error `NO_TERMINAL` (frontend shows a hint to Settings).
- `launch_ssh` builds `Command::new(program).args(prefix).args(["bash","-c",SCRIPT,"--"]).args(ssh_args)`.
- Settings: `terminal: Option<String>`, `terminal_custom_command: Option<String>`
  (serde skip-if-none, backward compatible).
- Command `list_terminals` → detected ids for the Settings dropdown
  ("Automatic", detected terminals, "Custom…" + text field).
- Tests: resolve logic with injected PATH lookup closure.

---

## D. Local browser selection + folder drag & drop

### Findings
- Local browser has no selection; only single files are draggable (folders
  `draggable="false"`).
- Local→remote drop calls `uploadFileList` → always job kind `upload` → folders would fail.
- Every drop targets the current directory, never the folder row under the cursor.

### Design
- `LocalFileBrowser` gets `selectedNames`/`anchorName` with the same click semantics as
  `FileBrowser` (plain / Ctrl / Shift), selection info, `..` row not selectable.
- All rows except `..` draggable; drag payload = selection if the dragged row is selected,
  else just that row. `DndSource.local.paths` carries all paths.
- Remote side accepts local drops via `uploadPathList` (file/dir resolved per path).
- Drop onto a folder row (not part of the drag set) → target = that folder; elsewhere →
  current directory. Same for remote→local (`onDownloadCallback(names, destDir)`) and for
  OS drops (Tauri drop `position` → `document.elementFromPoint` after dividing by
  `devicePixelRatio`). Row highlight reuses `file-entry--drop-target` style.
- `uploadPathList` / `uploadFileList` take an explicit `targetDir` parameter.

---

## E. Copy to…

### Design
- Frontend: context menu + action "Copy to…" beside "Move to…", prompt pre-filled with
  current path, file name kept. Per item: skip if target == source path; conflict →
  existing `resolveOverwrite` dialog (apply-to-all).
- New queue job kind `remoteCopy` (`TransferKind::RemoteCopy`, src = remote source,
  dst = remote target), so copies run in the background with cancel + progress rows.
- Worker:
  1. SSH profile: `sftp_service::exec_available(profile)` — opens a session channel,
     runs `printf murmur-exec-ok`, true only if stdout matches; result cached per
     profile id for the process lifetime (in-memory `OnceLock<Mutex<HashMap>>`,
     cleared on disconnect and in `cleanup_on_exit`).
  2. Available → `cp -a -- '<src>' '<dst>'` with POSIX single-quote escaping; non-zero
     exit → job error with stderr.
  3. Not available or FTP → fallback: download (file or dir) into
     `~/.config/murmurssh/workspace/.copy-tmp/<job_id>/`, then upload (safe upload from
     A) to dst; temp dir removed in all outcomes; stale `.copy-tmp` wiped at startup.
- Folder copies into themselves (`dst` starts with `src + "/"`) are rejected in the
  frontend.

---

## F. Keyboard shortcuts

### Panel focus
- New `src/panel-focus.ts`: `activePanel: "local" | "remote"`, set on click inside a
  panel and by `Tab`; active panel gets a CSS outline class.
- FileBrowser's global handler only acts when remote is active; LocalFileBrowser gets
  its own handler (same guard rules: no modal open, not typing in an input).

### Keys (both panels unless noted)
| Key | Action |
|---|---|
| ↑ / ↓ | move cursor (selection) |
| Shift+↑ / Shift+↓ | extend selection |
| Home / End, PgUp / PgDn | jump |
| Enter | open folder / edit file (existing) |
| Backspace, Alt+↑ | parent folder |
| Tab | switch panel |
| Ctrl+L | focus path field |
| F2 | rename |
| F5 | refresh (unchanged) |
| F6 | Move to… (remote) |
| Ctrl+Shift+C | Copy to… (remote) |
| Ctrl+C / Ctrl+X, then Ctrl+V | internal clipboard copy/move within the remote panel (paste into current dir) |
| F7, Ctrl+Shift+N | new folder |
| Ctrl+N | new file (remote) |
| Delete | delete |
| Ctrl+A | select all |
| Ctrl+U (local) / Ctrl+D (remote) | transfer selection to the other panel's current dir |
| F11 | terminal (remote, SSH) |
| F1, ? | help dialog |
| Esc | close menu / clear selection |
| type letters | type-ahead (existing) |

- Help dialog shortcut table MUST list every shortcut of the app (both panels, dialogs
  Enter/Esc, type-ahead) — no key may exist that is not documented there (all 6 locales).
  The table is generated from one shared shortcut list (`src/shortcuts.ts`) that the
  handlers reference, so help and behaviour cannot drift.
- Local panel gains rename (F2), new folder (F7) and delete (with confirm) only where
  backend commands already exist; missing local commands (`create_local_dir`,
  `delete_local_path`) are added with the same null-byte + canonicalize checks as
  `list_local_directory`.

---

## Out of scope
Clipboard between local and remote panels (use Ctrl+U/Ctrl+D), remote-to-remote copy
across profiles, customizable key bindings.

## Validation
`cargo build`, `cargo clippy`, `cargo test` (lib), `npx tsc`, `npx vite build`; manual run
of each flow in `npm run tauri dev` against a test SSH host and an FTP host.
