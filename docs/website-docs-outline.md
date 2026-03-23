# MurmurSSH — Documentation Outline

**Version:** 0.1.3-beta.2
**Date:** 2026-03-23

This outline defines what documentation sections the app can actually support. Each section maps to a confirmed feature from `website-feature-inventory.md`. Sections marked with a source reference indicate where the underlying behavior is verifiable.

Only include a section when the corresponding feature is fully implemented and stable enough for user-facing documentation.

---

## Structure

```
docs/
  getting-started.md
  installation.md
  profiles.md
  connecting.md
  sftp-browser.md
  remote-editing.md
  credential-storage.md
  ssh-keys.md
  settings.md
  known-limitations.md
  contributing.md
```

---

## Section Summaries

---

### getting-started.md

**Purpose:** Entry point for new users.

**Covers:**
- What MurmurSSH is (SSH/SFTP desktop client for Linux; Tauri 2; no cloud; MIT)
- What it does not do (no Windows/macOS, no built-in terminal, no cloud sync)
- Minimum requirements (Ubuntu 22.04+, `x-terminal-emulator`, `xdg-utils`)
- Two-step quickstart: install .deb → create a profile → connect

**Do not include:**
- Feature comparisons with other tools
- Performance or security superlatives

---

### installation.md

**Purpose:** How to install the application.

**Covers:**
- Install from `.deb` package: `sudo dpkg -i murmurssh_*.deb`
- Required runtime packages (`x-terminal-emulator`, `xdg-utils`)
- Build from source:
  - Prerequisites: Rust (stable), Node.js 18+
  - System build dependencies (list from README)
  - `npm install` → `npm run tauri dev` (development)
  - Icon generation: `npm run tauri icon path/to/icon.png`
  - `npm run tauri build` → `.deb` in `src-tauri/target/release/bundle/deb/`

---

### profiles.md

**Purpose:** Creating and managing connection profiles.

**Covers:**
- What a profile stores (name, host, port, username, auth type, key path, default remote path, local path, editor command, upload mode, credential storage mode)
- Creating a profile (New → fill form → Save)
- Editing a profile (Edit button; disabled while connected)
- Deleting a profile (Delete button; disabled while connected; removes associated secrets)
- Profile ID: generated from display name, immutable after creation
- Profile JSON location: `~/.config/murmurssh/profiles/<id>.json`
- Profile portability: plain JSON, can be copied manually
- Backup files: `.json.bkp` created before each overwrite

**Do not include:**
- Import/export UI (does not exist yet)
- Profile sync (out of scope)

---

### connecting.md

**Purpose:** How to connect to a server.

**Covers:**
1. Select a profile → click Connect
2. Host key verification on first connect:
   - SHA-256 fingerprint displayed
   - Three options: Accept once / Accept and save / Cancel
   - Mismatch (existing key differs from server): connection blocked
3. Authentication:
   - Key: path to private key; passphrase prompted if encrypted (never saved)
   - Agent: delegates to `ssh-agent`; no prompt needed if agent has identity
   - Password: prompted; save-mode options presented (see credential-storage.md)
4. On success: SFTP file browser loads; SSH terminal session available via Terminal button
5. Error states: unknown host, auth failure, timeout, host key mismatch — all handled with specific dialogs or status bar messages

**Terminal session subsection:**
- Terminal button launches SSH in `x-terminal-emulator`
- Password auth: ControlMaster socket reused (no re-prompt if connected)
- Key auth: direct `ssh -i <keyfile>` (passphrase prompted by terminal interactively)
- SSH key compatibility warning: if key permissions are too open, runtime copy prompt shown

---

### sftp-browser.md

**Purpose:** Using the SFTP file browser.

**Covers:**
- Navigation: directory rows, Up button, Home button, breadcrumbs, editable path input
- Toolbar: Disconnect, Terminal, Home, Up, Refresh
- Selecting files (single-click) and navigating into directories (double-click)
- File operations table (matches README usage table):
  - Upload, Download, Edit, Delete, Create file, Create folder
  - Download folder (recursive)
  - Delete folder (recursive; explicit confirmation required)
- Symlink-to-directory behavior (navigable like a regular directory)
- Error handling: inline errors for listing failures; status bar for operation failures

**Do not include:**
- Rename UI (no button exists; backend only)
- Multi-file selection (not implemented)

---

### remote-editing.md

**Purpose:** Editing remote files with a local editor.

**Covers:**
- Select a file → click Edit
- Restrictions: text files only; maximum 1 MB; binary files rejected
- File downloaded to `~/.config/murmurssh/workspace/<profile_id>/`
- Opened in `editor_command` (per-profile) or system default (`xdg-open`)
- File watcher monitors for saves (inotify; 300ms debounce)
- Upload mode: auto (immediate upload on save) vs confirm (confirmation dialog before upload)
- Status events: upload-complete (success), upload-error (failure)
- Workspace cache: not automatically cleaned up between sessions

**Do not include:**
- Conflict resolution (not implemented)
- Versioned backups (not implemented)

---

### credential-storage.md

**Purpose:** How passwords are stored (or not stored).

**Covers:**
- Three save modes, what each does, where it stores the password
- "Don't save" (default): session-only, nothing on disk
- "Save on this PC only": plaintext file at `~/.config/murmurssh/secrets/<id>`, 0600, not portable
- "Save in profile file": plaintext in profile JSON (`stored_secret_portable`), portable but visible
- Passphrase rule: SSH key passphrases are never saved; no save-mode option is presented; always prompted at connection time
- Clearing a saved password: Edit → Clear Saved Credential
- Auto-clear on auth type change: switching from password to key/agent clears all stored credentials

**Tone note:** State the facts accurately — do not describe plaintext storage as "secure" or "encrypted". Describe it as plaintext with restricted permissions.

---

### ssh-keys.md

**Purpose:** Using SSH key authentication.

**Covers:**
- Selecting a private key file (Browse button in profile form)
- Passphrase-protected keys: prompt at connection time, never saved
- SSH agent (`ssh-agent`): how to configure it, what MurmurSSH does with it
- SSH key compatibility: the runtime key copy feature
  - When it triggers (group/other permission bits set on key file)
  - User prompt before terminal launch
  - Copy location and permissions (`~/.config/murmurssh/runtime-keys/`, 0600)
  - Automatic deletion on disconnect and startup
  - Original key never modified

---

### settings.md

**Purpose:** Configuring MurmurSSH.

**Covers:**
- App settings (stored in `~/.config/murmurssh/settings.json`):
  - Profiles storage path (custom directory via folder picker)
  - Theme (system / dark / light)
- Theme: dark = Catppuccin Mocha, light = Catppuccin Latte; system follows OS preference in real time
- Per-profile settings: editor command, default remote path, local path, upload mode (documented under profiles.md)

---

### known-limitations.md

**Purpose:** Honest documentation of current limitations.

**Covers (directly from README):**
- Only one profile can be active at a time
- No undo for deleted files or folders
- Binary files and files > 1 MB cannot be opened for editing
- Each SFTP operation opens a fresh connection (not optimized for rapid sequential use)
- Linux only; no Windows or macOS support
- Beta status: expect bugs; report via issue tracker
- File rename has no UI button (backend support only)
- Workspace cache is not cleaned up automatically

---

### contributing.md

**Purpose:** Guide for contributors.

**Covers:**
- Where to report bugs (GitHub issues)
- How to build from source (mirrors installation.md build section)
- Development workflow: `npm run tauri dev`
- Contribution expectations: read PRD.md first; minimal focused changes; stay within existing architecture
- Good first areas: bug reports, UI polish, documentation, platform testing on other Debian-based distros
- PR process: open issue first for larger changes

**Do not include:**
- Contribution statistics, commit graphs, or claims about community size that may be inaccurate at publish time
