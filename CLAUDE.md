# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

MurmurSSH is a minimal open-source Linux desktop client for SSH and SFTP, built with Tauri 2 and Rust, targeting Ubuntu and distributed as a `.deb` package.

## Commands

```bash
npm install             # install Node dependencies (first time)
npm run tauri dev       # start development build with hot reload
npm run tauri build     # build release binary and .deb package
```

The `.deb` output lands in `src-tauri/target/release/bundle/deb/`.
Icons must be generated before building: `npm run tauri icon path/to/icon.png`

## Architecture

```
src/                        # Vanilla TypeScript frontend (no framework)
  api/index.ts              # Typed invoke() wrappers for all Tauri commands
  components/               # DOM-based UI components
    credential-dialog.ts    # Password/passphrase/host-key dialogs + save mode choice
    dialog.ts               # showConfirm() — promise-based in-app modal
    file-browser.ts         # SFTP file browser (toolbar: Disconnect/Terminal/Home/Up/Refresh)
    profile-form.ts         # Profile create/edit modal form
    profile-selector.ts     # Profile dropdown + management buttons (New/Edit/Delete)
    settings-dialog.ts      # Settings modal: profile storage path configuration
    status-bar.ts           # Connection status indicator
  types.ts                  # Shared TypeScript types (mirrors Rust models)
  main.ts                   # App entry point — wires all components together
src-tauri/src/
  models/                   # Rust data types: Profile, Settings, FileEntry
  services/                 # Business logic — one file per concern
    credentials_store.rs    # Session-only in-memory credential cache (never on disk)
    known_hosts_service.rs  # Host key verification (~/.config/murmurssh/known_hosts)
    secrets_service.rs      # Local-machine persistent secret files (~/.config/murmurssh/secrets/)
    sftp_service.rs         # SFTP operations + connect() with host-key/auth logic
    ssh_session_service.rs  # SSH SSO: ControlMaster (password auth) / ssh-agent (key+passphrase)
    workspace_service.rs    # Remote editing, watcher dedup via active_watchers()
  commands/                 # Tauri IPC layer — thin wrappers over services
    connection.rs           # connect_sftp, accept_host_key, save_credential, clear_credential
  lib.rs                    # Registers all commands with Tauri builder
  main.rs                   # Binary entry point
```

Profiles are stored as individual JSON files in `~/.config/murmurssh/profiles/`.
Settings live in `~/.config/murmurssh/settings.json`.
Known hosts stored in `~/.config/murmurssh/known_hosts` (format: `hostname:port SHA256:hex`).

SSH sessions are launched via `x-terminal-emulator -e ssh ...` — no embedded terminal. The terminal handles its own auth interactively; the MurmurSSH known_hosts and credentials only apply to SFTP connections.

SFTP (`sftp_service`) uses the `ssh2` crate. Each operation opens a fresh connection — simple, no shared session state. `connect()` enforces host key verification and authentication in order: TCP → handshake → known_hosts check → auth.

**Auth types**: `key` (pubkey file, passphrase optional), `agent` (ssh-agent), `password` (runtime prompt).

**Connection flow**: frontend calls `api.connectSftp(profileId)` which returns structured errors handled by `verifyConnection()` in `main.ts`:
- `"UNKNOWN_HOST:<fp>"` → show host key dialog → `api.acceptHostKey()` → retry
- `"NEED_PASSWORD"` → show password prompt with save-mode choice → retry with credential
- `"NEED_PASSPHRASE"` → show passphrase prompt with save-mode choice → retry with credential

On successful auth after a prompt, if the user chose a save mode != "never", `main.ts` calls `api.saveCredential(profileId, secret, mode)`.

**Credential storage tiers** (security, highest → lowest):
- `never` — prompted every connection; nothing written to disk (default)
- `local_machine` — plaintext file at `~/.config/murmurssh/secrets/<profile_id>`, 0600 perms; does not travel with profile JSON
- `portable_profile` — plaintext inside the profile JSON field `stored_secret_portable`; portable but clearly labeled as less secure

**Passphrase rule (enforced at all layers)**: SSH key passphrases are NEVER persisted. `showPassphrasePrompt` returns `string | null` with no save option. `save_credential` in the backend no-ops for any non-password auth profile. Auto-load in `connect_sftp` only triggers for `AuthType::Password`. Passphrases are always prompted at connection time and discarded after use.

Runtime (session-only) credentials are in `credentials_store` (`OnceLock<Mutex<HashMap>>`). On `connect_sftp`, if the session store is empty **and auth type is Password**, the backend auto-loads from persistent storage before calling `sftp_service::test_connection()`.

**Credential cleanup rules**:
- `clear_credential` removes local file + portable field + resets metadata + clears session store
- `delete_profile` removes local secret file + clears session store before deleting the profile JSON
- Switching a profile from password to key/agent auth clears all stored credentials (both frontend and backend enforce this)

The workspace/edit flow (`workspace_service`) downloads remote files to `~/.config/murmurssh/workspace/<profile_id>/`, opens them with `xdg-open` or a configured editor command, and watches for saves with `notify` (inotify on Linux). On save: auto-upload or emit `upload-ready` event for confirm mode. Duplicate watchers for the same file are prevented via `active_watchers()` registry.

Tauri events used by the workspace flow:
- `upload-ready` → payload: `{ profile_id, local_path, remote_path }` — frontend asks user to confirm
- `upload-complete` → payload: remote path string — auto-upload succeeded
- `upload-error` → payload: error string — auto-upload failed

File picker for SSH key paths uses `tauri-plugin-dialog` (Rust) + `@tauri-apps/plugin-dialog` (JS). Capability: `dialog:allow-open` in `src-tauri/capabilities/default.json`.

Profile IDs are generated from the display name: lowercase, non-alphanumeric sequences replaced with `-`, leading/trailing dashes stripped. IDs are immutable after creation.

## Phases Complete

- Phase 1: Project foundation, profile persistence, SSH launch, UI shell
- Phase 2: SFTP file browser, workspace/remote edit flow
- Phase 3: Profile management GUI, file picker, in-app modals, connect validation
- Phase 4: Password auth, passphrase support, host key verification, workspace stability
- Phase 5: Credential storage modes (never/local_machine/portable_profile), save-mode UX in dialogs, profile portability
- Phase 5.3: Host-key dialog (Accept once/save/cancel), file browser toolbar (Disconnect/Terminal/Home/Up), breadcrumbs, permission-safe navigation, disconnect state cleanup
- Phase 5.4: SSH SSO (ControlMaster for password auth, ssh-agent for key+passphrase), Terminal button in toolbar, file browser scroll fix, configurable profile storage path, profile backup on write (.json.bkp), open-profile-folder command, settings dialog with path configuration
- Phase 5.5: Removed auto terminal launch (Terminal button only), centralized connection state (connectedProfileId + ProfileSelector.setConnected), CSS custom properties + light theme (Catppuccin Latte), theme selector in settings dialog, system theme listener via matchMedia, settings save bug fix (read-merge-write)
- Phase 5.6: Toolbar state hardening (refresh errors use inlineError+render() keeping buttons active), symlink-to-directory fix in SFTP listing (stat() follows symlink for public_html etc.), editable path input field (Enter navigates, Escape resets), download flow via save dialog (downloadFileTo command), New File and New Folder create actions (＋ File / ＋ Folder buttons)
- Phase 5.7: local_path per-profile field (stored in profile JSON, folder picker in form), download uses local_path directly if set (no dialog), upload picker starts in local_path via Tauri open() dialog (replaces hidden file input), connected profile locking (Edit/Delete disabled while connected, restored on disconnect)
- Phase 6.2/6.3: Recursive folder delete with explicit confirmation dialog (delete_directory command + sftp_service), delete flow audit (correct confirmation text per type, stale selection cleared, refresh after delete), README and CLAUDE.md release documentation update
- Phase 6.5: Recursive folder download (download_directory command + sftp_service + frontend), Help dialog version display (get_app_version command), Help dialog terminal-close note + Releases link, file-browser scroll-jump fix (save/restore scrollTop around render()), Download button supports both files and folders
- Phase 6.6: Terminal passphrase reliability fix (key+passphrase profiles now use direct ssh -i invocation; terminal handles passphrase prompt interactively; removed fragile ssh-agent SSO path for key auth), file-browser scroll stability fix (save/restore applied to .file-browser scrollable div instead of .file-browser__table which has no overflow)
- Phase 6.8: Runtime SSH key copy for terminal compatibility (check_key_needs_copy + copy_key_for_runtime + delete_runtime_key + cleanup_runtime_keys commands; runtime_key_service with 0600 perms; user prompt in file-browser before terminal launch; startup cleanup; disconnect cleanup; Help dialog and README updated)
- Phase 6.9: AppImage packaging (added "appimage" to bundle targets in tauri.conf.json), release metadata cleanup (identifier → de.kai-schultka.murmurssh; publisher, copyright, category, shortDescription, longDescription added), GitHub workflow extended to find and upload both .deb and .AppImage as release assets (AppImage upload is conditional — skipped with warning if not found), README updated with AppImage install instructions
- Phase 7.0: Recursive folder upload (upload_directory in sftp_service using single SFTP session + mkdir_ok_if_exists helper; upload_directory command; uploadDirectory API wrapper; Upload Folder button in file browser action bar; folder picker starts in local_path if set; symlinks followed via is_dir()/is_file(); broken symlinks and special files silently skipped)
- Phase 7.1: Release metadata polish (publisher/copyright → Kai André Schultka; shortDescription/longDescription updated to local-first framing), website link added to help dialog and README, README screenshots section wired to docs/screenshots/ (real screenshots present), SFTP table de-duped and Upload Folder row added, GitHub URL placeholder fixed, docs/screenshots/README.md added for naming conventions
- Phase 8: Stable readiness — LICENSE copyright → Kai André Schultka (2026), CHANGELOG rewritten with full user-facing history (0.1.0-beta through 0.1.4-beta.1 + Unreleased), CONTRIBUTING clone URL fixed, SECURITY.md "early development" language removed, ROADMAP updated (completed items removed, near-term reflects v1.0+), README beta caveat removed from known limitations (publisher/copyright → Kai André Schultka; shortDescription/longDescription updated to local-first framing), website link added to help dialog and README, README screenshots section wired to docs/screenshots/ (real screenshots present), SFTP table de-duped and Upload Folder row added, GitHub URL placeholder fixed, docs/screenshots/README.md added for naming conventions
- Phase 9.2: Multi-selection (Set-based, Ctrl/Shift/plain click, selection count info bar), Rename single entry (showPrompt pre-filled with current name), Move via drag-and-drop onto folder rows or ".." row, Move to… action (prompted target path), batch Delete/Download on multi-select; all actions on selectedNames Set; no new backend code (reuses existing rename_file SFTP command for move)
- Phase 9.3: Transfer progress bar (file counter, cancel button) for Upload/Download batch ops; Upload overwrite dialog (Yes/No/Cancel + Apply-to-all checkbox) using new remote_file_exists SFTP command; showOverwriteDialog added to dialog.ts; transfer state managed via startTransfer/updateTransfer/endTransfer without re-rendering mid-loop
- Phase 9.4: Keyboard shortcuts in file browser (F2 rename, F5 refresh, F11 terminal, Delete, Ctrl+A, Enter, Escape); download drop zone (dashed zone below action bar, drag remote rows onto it to download); drag-from-local-browser to remote = upload; setupKeyboardShortcuts() registered once in constructor; shortcuts listed in help dialog (EN+DE)
- Phase 9.5: Local file browser panel (LocalFileBrowser component, src/components/local-file-browser.ts); split-pane layout (#browsers-pane flex row); toggle button in remote toolbar (accent-colored when hidden); disconnect button icon-only (logout SVG); per-user local path persistence (portable profiles: local_paths_by_user HashMap in profile JSON, keyed by $USER; local profiles: existing local_path field); cross-browser DnD via shared dnd-state.ts module; new Rust commands: list_local_directory, get_home_dir, get_current_user, get_local_browser_path, save_local_browser_path (all with null-byte rejection + canonicalize() security); local browser position (left/right) configurable in Settings → stored in settings.json as local_browser_position
- Phase 10.0 (v1.4.7): Remote-edit flow fix — replaced mtime-based change detection in workspace_service.rs with a content-hash baseline registry (`file_content_hash` + `baselines()` OnceLock<Mutex<HashMap>>) shared between `open_for_edit` (sets baseline after each download) and the watcher (acts only on hash change); fixes spurious upload prompt after re-opening a file and double upload confirmation from temp+rename saves. App-exit cleanup — `cleanup_on_exit()` wired via Tauri `RunEvent::ExitRequested` (`.build().run(...)` in lib.rs) calls new `ssh_session_service::stop_all_sessions()` + `cleanup_all_runtime_keys()` + `credentials_store::clear_all()` so closing via the OS title bar tears down ControlMaster sessions, runtime keys, and session credentials. Roadmap spec/plans under docs/superpowers/; vault note vault/MurmurSSH Optimization Roadmap.md
- Phase 10.1 (v1.4.8): File list & permissions. Added `perm: Option<u32>` to `FileEntry` (models/sftp.rs), populated from SFTP `stat.perm`; `None` for FTP/local. New `set_permissions` command (commands/sftp.rs → sftp_service.rs) does a perm-preserving `setstat` (stats first, ORs existing `& 0o170000` type bits with `mode & 0o7777`); FTP returns unsupported. File browser gained Modified (locale date from mtime) and Permissions (symbolic + octal tooltip) columns — table is now 4 columns (colspans updated). New `showPermissionsDialog` (dialog.ts): rwx checkbox grid ↔ octal field, bidirectional sync, Apply returns grid-derived mode; wired into file/folder context menus via `handlePermissions`. i18n keys added to all 6 locales. NOTE: frontend has tracked `.js` siblings (tsc emits in-place; Vite loads .ts) — edit .ts, regenerate .js via `npx tsc`, commit both.
- Phase 11.0 (v1.5.0): Background transfer queue + multi-connection. New `transfer_queue` service (services/transfer_queue.rs): one dispatcher thread (`Mutex<QueueState>` + `Condvar`) promotes Queued→Active jobs up to `max_concurrent_transfers` (Settings, default 2, clamp 1–8 via `models/transfer.rs::clamp_concurrency`), spawning one worker thread per job — each worker opens its own SFTP/FTP session for true parallelism. Per-job cancellation via `Arc<AtomicBool>` replaced the deleted profile-keyed `transfer_cancel`; the low-level transfer fns in `sftp_service`/`ftp_service` now take `cancel: &dyn Fn() -> bool` (workspace edit-flow passes `&|| false`). New model `models/transfer.rs` (`TransferKind`/`TransferState`/`TransferJobView`, `CANCELLED_ERROR`, `clamp_concurrency`). New commands `commands/transfer.rs`: `enqueue_transfer`/`cancel_transfer(job_id)`/`cancel_all_transfers`/`list_transfers`/`clear_finished_transfers` + `local_path_is_dir`; the old `Channel`-based transfer commands (`upload_file`/`download_file`/`download_file_to`/`upload_directory`/`upload_path`/`download_directory`/`cancel_transfer(profile_id)`) were removed (`upload_file_bytes` kept for New File). Queue emits a global `transfer-update` event (one `TransferJobView`); `transfer_queue::init(&app)` wired in `lib.rs` `.setup`, and `cancel_all()` added to `cleanup_on_exit`. Frontend: new `TransferQueuePanel` (src/components/transfer-queue.ts) renders jobs from events with in-place row updates (full re-render only on structural change — add/remove/state change) plus per-job/cancel-all/clear-finished controls; `file-browser.ts` transfer methods resolve overwrites/destinations then `enqueue` (single-progress-bar machinery removed; `disconnect()` no longer cancels — the queue is independent); Settings dialog gains a concurrency input; i18n in all 6 locales (`transferQueue.*`, `fileBrowser.queuedCount`, `settings.maxConcurrentTransfers[Hint]`). Dispatcher correctness (Opus-reviewed): settings read + event emit happen OUTSIDE the queue lock, and the `cv.wait` is predicate-guarded under the lock (closes a lost-wakeup window), plus a worker `catch_unwind` panic guard. Reviews: per-task spec + quality + Opus final holistic (which caught and fixed the lost-wakeup regression) = SHIP. Build: cargo/clippy/tsc/vite green, 9 lib tests pass.

## Guidance Files

Before implementing anything, read all of the following:

- `PRD.md` — full product requirements and acceptance criteria
- `.claude/skills/product-scope.md` — scope rules and decision bias
- `.claude/skills/architecture-rules.md` — structure, module, and persistence rules
- `.claude/skills/linux-integration.md` — platform, terminal, and filesystem conventions
- `.claude/skills/open-source-guidelines.md` — code and community expectations
- `.claude/skills/execution-workflow.md` — execution order, sub-agent usage, issue logging, validation, and final reporting rules

## Execution Rules

When implementing anything in this repository:

- first read all guidance files listed above
- extract constraints before making changes
- identify affected files by layer before editing
- decompose non-trivial work into small implementation steps
- use sub-agents for complex, cross-layer, or unclear-root-cause tasks
- maintain a live issue/finding list during the work
- resolve errors and warnings explicitly instead of silently working around them
- validate requirements and affected flows before declaring completion
- report files changed, root causes, validation, and remaining issues in the final summary

Sub-agents are especially expected when work crosses:
- `src/` and `src-tauri/src/`
- UI + backend + persistence
- security/authentication/secret handling
- multiple core modules with shared state or compatibility impact

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **MurmurSSH** (1363 symbols, 3792 relationships, 114 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## When Debugging

1. `gitnexus_query({query: "<error or symptom>"})` — find execution flows related to the issue
2. `gitnexus_context({name: "<suspect function>"})` — see all callers, callees, and process participation
3. `READ gitnexus://repo/MurmurSSH/process/{processName}` — trace the full execution flow step by step
4. For regressions: `gitnexus_detect_changes({scope: "compare", base_ref: "main"})` — see what your branch changed

## When Refactoring

- **Renaming**: MUST use `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` first. Review the preview — graph edits are safe, text_search edits need manual review. Then run with `dry_run: false`.
- **Extracting/Splitting**: MUST run `gitnexus_context({name: "target"})` to see all incoming/outgoing refs, then `gitnexus_impact({target: "target", direction: "upstream"})` to find all external callers before moving code.
- After any refactor: run `gitnexus_detect_changes({scope: "all"})` to verify only expected files changed.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Tools Quick Reference

| Tool | When to use | Command |
|------|-------------|---------|
| `query` | Find code by concept | `gitnexus_query({query: "auth validation"})` |
| `context` | 360-degree view of one symbol | `gitnexus_context({name: "validateUser"})` |
| `impact` | Blast radius before editing | `gitnexus_impact({target: "X", direction: "upstream"})` |
| `detect_changes` | Pre-commit scope check | `gitnexus_detect_changes({scope: "staged"})` |
| `rename` | Safe multi-file rename | `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` |
| `cypher` | Custom graph queries | `gitnexus_cypher({query: "MATCH ..."})` |

## Impact Risk Levels

| Depth | Meaning | Action |
|-------|---------|--------|
| d=1 | WILL BREAK — direct callers/importers | MUST update these |
| d=2 | LIKELY AFFECTED — indirect deps | Should test |
| d=3 | MAY NEED TESTING — transitive | Test if critical path |

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/MurmurSSH/context` | Codebase overview, check index freshness |
| `gitnexus://repo/MurmurSSH/clusters` | All functional areas |
| `gitnexus://repo/MurmurSSH/processes` | All execution flows |
| `gitnexus://repo/MurmurSSH/process/{name}` | Step-by-step execution trace |

## Self-Check Before Finishing

Before completing any code modification task, verify:
1. `gitnexus_impact` was run for all modified symbols
2. No HIGH/CRITICAL risk warnings were ignored
3. `gitnexus_detect_changes()` confirms changes match expected scope
4. All d=1 (WILL BREAK) dependents were updated

## Keeping the Index Fresh

After committing code changes, the GitNexus index becomes stale. Re-run analyze to update it:

```bash
npx gitnexus analyze
```

If the index previously included embeddings, preserve them by adding `--embeddings`:

```bash
npx gitnexus analyze --embeddings
```

To check whether embeddings exist, inspect `.gitnexus/meta.json` — the `stats.embeddings` field shows the count (0 means no embeddings). **Running analyze without `--embeddings` will delete any previously generated embeddings.**

> Claude Code users: A PostToolUse hook handles this automatically after `git commit` and `git merge`.

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
