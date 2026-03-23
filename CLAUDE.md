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
