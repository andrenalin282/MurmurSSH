# Changelog

All notable changes to MurmurSSH are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

No changes yet.

---

## [1.0.2] - 2026-03-24

### Added
- **Transfer progress with cancel** for uploads and downloads (file counter based)
- **Upload overwrite dialog** (`Yes` / `No` / `Cancel` + `Apply to all`) for conflict-safe batch uploads
- **SSH config import** in the New Profile dialog from `~/.ssh/config`

### Changed
- Removed duplicate breadcrumb row in the file browser; path is now shown only in the editable path input
- Hardened upload flows so overwrite protection is applied consistently across picker, drag-and-drop, and recursive uploads
- Hardened connect state to block parallel connection attempts during "connecting"
- SSH import now skips incomplete entries safely and reports created/skipped/failed results clearly
- Transfer, move, and rename feedback text is now more consistent and robust
- Small UI/CSS stability fixes for transfer status rendering

---

## [1.0.0] — 2026-03-23

_First stable release._

This release promotes MurmurSSH from beta to stable. No new features — all functionality is as shipped in 0.1.4-beta.1. This version establishes the baseline for stable versioning going forward.

---

## [0.1.4-beta.1] — 2026-03-23

### Added
- **AppImage packaging** — portable Linux option alongside the existing `.deb`; no installation required
- **Recursive folder upload** — upload an entire local directory to the current remote path, preserving structure; folder picker respects the per-profile local path setting
- **Upload Folder button** in the SFTP file browser action bar
- Website link (`murmurssh.kai-schultka.de`) in the Help/About dialog and README

### Changed
- App identifier updated to `de.kai-schultka.murmurssh`
- Package metadata: publisher and copyright updated to Kai André Schultka; short and long descriptions revised to lead with local-first framing
- GitHub release workflow now uploads both `.deb` and `.AppImage` artifacts; AppImage upload is conditional and non-blocking if the build does not produce one
- README: screenshots section wired to real screenshots in `docs/screenshots/`; SFTP browser table cleaned up and updated; clone URL fixed; AppImage instructions updated to correct filename pattern
- App icon set regenerated from source logo (adds 256×512px and other standard sizes)

---

## [0.1.3-beta] — 2025

### Added
- **SSH key runtime copy** — when a private key has overly permissive permissions (e.g. on mounted filesystems), MurmurSSH prompts to create a `0600` local copy for the terminal session; original key is never modified; copy is deleted on disconnect
- **Recursive folder download** — download an entire remote directory tree locally
- **Recursive folder delete** — delete a remote directory and all its contents with an explicit confirmation dialog
- **Help / About dialog** — shows app version, links to GitHub, issues, and releases
- **Download Folder button** in the SFTP file browser

### Changed
- Terminal passphrase reliability: key+passphrase profiles now use direct `ssh -i` invocation; the terminal handles the passphrase prompt interactively
- File browser scroll stability: scroll position preserved across re-renders

---

## [0.1.2-beta] — 2025

### Added
- **Per-profile local path** — set a default local folder for uploads and downloads; file pickers start in that folder
- **Editable path input** in the file browser — type a remote path and press Enter to navigate; Escape resets
- **New File and New Folder** actions in the file browser
- **Download save dialog** when no local path is configured
- **Symlink-to-directory support** — symlinks pointing to directories are shown and navigable as folders (e.g. `public_html`)

### Changed
- Edit and Delete profile buttons are locked while a session is active, restored on disconnect
- Toolbar errors show inline while keeping toolbar buttons active

---

## [0.1.1-beta] — 2025

### Added
- **Theme selector** — dark, light, and system themes in Settings; Catppuccin Mocha (dark) and Catppuccin Latte (light) palettes
- **SSH SSO** — ControlMaster session reuse for password auth (no re-prompt when opening the terminal); ssh-agent forwarding for key auth
- **Configurable profile storage path** in Settings — point profiles at any directory
- **Host key dialog** — Accept once (session only) / Accept and save / Cancel
- **Credential storage tiers** — never / save on this machine (0600 local file) / save in profile (portable JSON); SSH key passphrases are never saved under any setting
- **Profile backup on save** — `.json.bkp` created before each overwrite
- **Breadcrumb navigation** in the file browser
- **Disconnect button** in the toolbar
- **Terminal button** in the toolbar (replaces auto-launch on connect)

### Changed
- Connection state centralised; connect/disconnect cycle is consistent
- Settings save uses read-merge-write to avoid clobbering unrelated fields

---

## [0.1.0-beta] — 2025

_First working build._

### Added
- Profile management — create, edit, delete, persist as local JSON files
- SSH session launch via system terminal (`x-terminal-emulator`)
- SFTP file browser — list, navigate, upload, download, delete, rename
- Remote file editing — download to local workspace, open in editor, re-upload on save (auto or confirm mode)
- SSH key, SSH agent, and password authentication
- Host key fingerprint verification and local `known_hosts` storage
- Connection status bar
- Settings dialog (profile storage path)
- MIT License

---

[Unreleased]: https://github.com/andrenalin282/MurmurSSH/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/andrenalin282/MurmurSSH/compare/v0.1.4-beta.1...v1.0.0
[0.1.4-beta.1]: https://github.com/andrenalin282/MurmurSSH/compare/v0.1.3-beta.2...v0.1.4-beta.1
[0.1.3-beta]: https://github.com/andrenalin282/MurmurSSH/releases/tag/v0.1.3-beta.2
[0.1.2-beta]: https://github.com/andrenalin282/MurmurSSH/releases/tag/v0.1.2-beta.1
[0.1.1-beta]: https://github.com/andrenalin282/MurmurSSH/releases/tag/v0.1.1-beta.1
[0.1.0-beta]: https://github.com/andrenalin282/MurmurSSH/releases/tag/v0.1.0
