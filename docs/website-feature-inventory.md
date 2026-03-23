# MurmurSSH — Website Feature Inventory

**Version audited:** 0.1.3-beta.2
**Audit date:** 2026-03-23
**Source:** Direct codebase inspection (src/, src-tauri/src/, README.md, PRD.md, CLAUDE.md)

This document records only features that are verifiably present in the codebase. It is intended as the single source of truth for marketing copy and documentation. Do not add claims not backed by this inventory.

---

## 1. Confirmed Features

### 1.1 Core App Purpose

- Minimal Linux desktop application for SSH and SFTP
- Built with Tauri 2 (Rust backend + WebView frontend using Vanilla TypeScript)
- No cloud account, telemetry, or external services
- All data stored locally in `~/.config/murmurssh/`
- Open-source (MIT license)
- Distributed as a `.deb` package for Ubuntu/Debian

---

### 1.2 Connection Features

**Authentication methods (all three implemented and tested in code):**
- **SSH Key**: private key file path required; passphrase-protected keys supported (passphrase prompted at connection time)
- **SSH Agent**: delegates to running `ssh-agent`; tries each available identity automatically
- **Password**: entered at connection time; optional save modes available

**Host key verification:**
- First-connection host key dialog: shows SHA-256 fingerprint before trusting
- Three choices: Accept once (session-only, no disk write), Accept and save (persists to `~/.config/murmurssh/known_hosts`), Cancel
- Trusted keys stored in `~/.config/murmurssh/known_hosts` (format: `hostname:port SHA256:hex`)
- Host key mismatch (MITM detection): connection rejected if stored fingerprint differs from server

**Connection flow:**
- TCP connection with 15-second timeout
- SSH handshake → host key check → authentication (in that order)
- Structured errors returned to frontend: unknown host, need password, need passphrase, auth failure

**SSH key compatibility (runtime key copy):**
- Detects if SSH private key has group/other permission bits set (common with mounted filesystems)
- User prompted before terminal launch; can choose to skip
- If accepted: 0600 copy created in `~/.config/murmurssh/runtime-keys/<profile_id>`
- Copy deleted on disconnect and on app startup (cleanup of crashed sessions)
- Original key file never modified

---

### 1.3 Profile Management

**Stored per-profile fields:**
- Display name
- Host (hostname or IP)
- Port (default 22)
- Username
- Auth type: key, agent, or password
- Key path (for key auth)
- Default remote path (optional; SFTP initial directory)
- Local path (optional; default directory for upload/download file pickers)
- Editor command (optional; blank = `xdg-open`)
- Upload mode: auto or confirm
- Credential storage mode: never, local_machine, or portable_profile
- Portable password field (only populated in portable_profile mode)

**Storage:**
- One JSON file per profile in `~/.config/murmurssh/profiles/` (or custom path)
- Plain text JSON; can be manually edited or copied between machines
- Backup created before each overwrite (`<id>.json.bkp`)
- Profile ID generated from display name (immutable after creation)
- Last-used profile restored automatically on startup

**Management actions (all implemented):**
- Create, edit, delete
- Edit and Delete disabled while a profile is connected (re-enabled on disconnect)
- Profile deletion removes associated secret files and clears session credential

---

### 1.4 Credential Storage

**Three tiers (explicitly selectable at password prompt):**

| Option | Storage location | Portable? | Plaintext? |
|---|---|---|---|
| Don't save (default) | Memory only; cleared on exit | — | — |
| Save on this PC only | `~/.config/murmurssh/secrets/<id>` with 0600 permissions | No | Yes |
| Save in profile file | Profile JSON field `stored_secret_portable` | Yes | Yes |

**Passphrase rule (enforced at all layers):**
- SSH key passphrases are never saved — not to disk, not in session beyond connection use
- Passphrase prompt has no save-mode options
- Backend `save_credential` is a no-op for non-password auth profiles
- Auto-load from storage only triggers for password auth profiles

**Credential cleanup:**
- "Clear Saved Credential" in Edit dialog: removes local secret file, clears portable field, resets mode, clears session store
- Auth type change away from password: all stored credentials cleared automatically
- Profile delete: local secret file removed, session store cleared

---

### 1.5 SSH Terminal

- SSH terminal launched in system terminal (`x-terminal-emulator`)
- Launched with `x-terminal-emulator -e bash -c "<ssh command>"`
- For key auth: `ssh -p <port> -i <keyfile> -o PasswordAuthentication=no user@host`
- For password auth (connected): ControlMaster socket reused for SSO; terminal inherits auth without re-prompting
- If SSH exits non-zero: terminal displays "SSH exited with code N. Press Enter to close this window." and waits
- If SSH exits zero: terminal closes automatically when session ends

**Requires `x-terminal-emulator`** installed (standard on most Ubuntu systems).

---

### 1.6 SFTP File Browser

**Navigation:**
- Directory listing with directories listed first, then files (alphabetical)
- Single-click to select; double-click to navigate into directory
- Up button (navigate to parent)
- Home button (navigate to configured default remote path, or SFTP home)
- Breadcrumb navigation (clickable path segments)
- Editable path input (Enter to navigate, Escape to reset)
- Refresh button (reload current listing)

**File operations:**
- Open for edit (text files only, ≤ 1 MB, non-binary)
- Upload (file picker; starts in local_path if configured)
- Download (to local_path if configured, otherwise save dialog)
- Delete (with confirmation)
- Rename (backend supports; no dedicated UI button — accessible only programmatically)
- Create empty file (＋ File button, prompts for filename)

**Folder operations:**
- Navigate into folders
- Download folder recursively (Download button on selected folder; creates local directory tree)
- Delete folder recursively (confirmation dialog explicitly mentions recursive deletion)
- Create new folder (＋ Folder button, prompts for name)

**Symlink handling:**
- Symlinks to directories detected via `stat()` (follows the link)
- Displayed and navigable as directories (e.g., `public_html` → actual target)

**Toolbar buttons:**
- Disconnect, Terminal, Home, Up, Refresh
- All disabled when no profile is connected

**Error handling in browser:**
- Directory listing errors shown inline (toolbar buttons remain active)
- File operation errors shown in status bar

---

### 1.7 Remote File Editing (Workspace)

- Select a remote text file → click Edit
- File downloaded to `~/.config/murmurssh/workspace/<profile_id>/<filename>`
- Restrictions: text files only (binary files rejected via null-byte detection in first 512 bytes); maximum 1 MB
- Opened with `editor_command` from profile, or `xdg-open` if not set
- File watcher (inotify on Linux) with 300ms debounce monitors for saves

**Upload mode: auto**
- File change detected → re-uploaded immediately
- `upload-complete` event sent on success; `upload-error` event on failure

**Upload mode: confirm**
- File change detected → `upload-ready` event sent
- Frontend shows confirmation dialog
- User confirms → file uploaded

**Watcher deduplication:**
- A second Edit click on the same file re-opens the editor (no duplicate watcher spawned)

---

### 1.8 Settings

**App-level settings (stored in `~/.config/murmurssh/settings.json`):**
- Last-used profile ID (auto-restored on startup)
- Custom profiles directory path (optional; defaults to `~/.config/murmurssh/profiles/`)
- Theme: system (default), dark, or light

**Theme system:**
- Dark mode: Catppuccin Mocha color palette (CSS custom properties)
- Light mode: Catppuccin Latte color palette
- System mode: follows OS `prefers-color-scheme` media query, updates in real time

**Per-profile settings (not global):**
- Default remote path
- Local path
- Editor command
- Upload mode (auto/confirm)

**Settings dialog:**
- Configures profiles storage path (folder picker)
- Configures theme

---

### 1.9 Error Handling and Validation

**Connection errors (structured, user-facing):**
- Unknown host (shows fingerprint dialog)
- Password required (shows password prompt with save-mode choice)
- Passphrase required (shows passphrase prompt without save option)
- Auth failure (shown in status bar)
- Host key mismatch (connection blocked, error shown)
- TCP timeout (15-second timeout, error shown)

**File browser errors:**
- Directory listing failures shown inline (non-fatal; toolbar stays active)
- File operation errors (upload, download, delete, create) shown in status bar
- Binary file or oversized file: rejected before download with specific error message
- Permission errors on navigation: inline error, breadcrumbs/toolbar remain usable

**Profile validation:**
- Required fields enforced before save (display name, host, username)
- Duplicate profile ID detection at creation time

---

### 1.10 Platform and Packaging

| Attribute | Value |
|---|---|
| Target OS | Ubuntu 22.04+, Debian-based Linux |
| Distribution | `.deb` package |
| Window size | 960×640 px (resizable) |
| Build tool | Tauri 2 (`npm run tauri build`) |
| Runtime deps | `x-terminal-emulator`, `xdg-utils` |
| Build deps | `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libssh2-1-dev`, and others |

---

### 1.11 Release / Distribution Status

- **Current release:** v0.1.3-beta.2
- **Status:** Beta — not production-ready; bugs should be reported via issue tracker
- **License:** MIT
- **Source:** Public GitHub repository
- **Install method:** Download `.deb` from GitHub Releases → `sudo dpkg -i murmurssh_*.deb`

---

## 2. Likely Features — Needs Confirmation

These behaviors are referenced in code or README but were not fully verified at the command level during this audit. They are almost certainly implemented but should be confirmed before use in marketing copy.

| Feature | Evidence | Confidence |
|---|---|---|
| Scroll position preserved during file browser refresh | Code pattern: `scrollTop` save/restore around `render()` | High |
| `.json.bkp` backup actually created before overwrite | Referenced in CLAUDE.md phases; service code pattern expected | High |
| `logs/` directory populated at runtime | Directory mentioned in README config map | Medium |
| Folder picker in settings dialog (for profiles path) | Referenced in CLAUDE.md phase 5.4 | High |
| Donate link in Help dialog | Referenced in agent report; link target unknown | Medium |
| GitHub Releases link in Help dialog | Referenced in agent report | High |

---

## 3. Not Yet Implemented / Explicitly Out of Scope

Per PRD.md and codebase inspection, the following are explicitly out of scope for this project:

**Never planned:**
- Cloud sync or remote settings storage
- Account or login system
- Telemetry or usage analytics
- Windows or macOS support
- AppImage distribution
- Port forwarding / SSH tunnels
- SSH snippet library
- Plugin system
- Team sharing
- Integrated text editor (always uses system editor)

**Not yet implemented (no UI or backend stub found):**
- Screenshots in README (marked "Coming soon")
- Rename file UI (backend command present; no button)
- Multi-file transfers (single-file operations only)
- Profile import/export UI (profiles are portable JSON manually; no UI for it)
- Versioned backups of workspace files
- Trash/undo for deleted files
- Advanced SSH options (ProxyJump, custom ssh_config entries, etc.)
- macOS or Windows builds
