# Changelog

All notable changes to MurmurSSH are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

No changes yet.

---

## [1.4.1] - 2026-04-05

### Changed
- Documentation updated to reflect v1.4.0 features (README, CHANGELOG, ROADMAP, CLAUDE.md)
- Website update instructions revised for v1.4.x

---

## [1.4.0] - 2026-04-05

### Added
- **Local file browser panel** — a second file browser pane shows your local filesystem alongside the remote browser. Toggle it on/off with the new split-pane icon in the toolbar (icon is accent-coloured when the panel is hidden, so the state is always obvious at a glance).
- **Split-pane layout** — when the local browser is open, the interface splits into local (left) and remote (right) side by side, mirroring the familiar FileZilla layout.
- **Configurable panel position** — a new setting in Settings → Local browser position lets you put the local panel on the left or right side of the remote browser.
- **Per-user local path persistence** — the last visited local directory is saved per profile. Shared (portable) profiles store a path per OS username so each user gets their own remembered location; local-machine profiles use the single `local_path` field.
- **Drag local → remote to upload** — drag a file row from the local browser onto the remote file list to upload it immediately.
- **Drop remote → local to download** — drag remote file or folder rows onto the local browser panel to download them to the current local directory.
- **Download drop zone** — a dashed "Drop here to download" zone below the remote browser's action bar accepts dragged remote rows and triggers download.
- **Keyboard shortcuts in the file browser:**

  | Key | Action |
  |---|---|
  | `F5` | Refresh directory listing |
  | `F2` | Rename selected entry |
  | `F11` | Open SSH terminal |
  | `Delete` | Delete selected entries |
  | `Enter` | Open folder / edit file |
  | `Ctrl+A` | Select all entries |
  | `Escape` | Clear selection / close context menu |

- **Keyboard shortcuts listed in Help** — the About dialog now includes the shortcuts table in both English and German.

### Changed
- **Disconnect button** is now icon-only (logout arrow) — the text label has been removed for a cleaner toolbar.
- Local filesystem commands use `canonicalize()` path resolution and null-byte rejection for security.

---

## [1.3.1] - 2026-04-02

### Fixed
- **UI no longer appears frozen during transfers** — progress events are now streamed from Rust to the frontend via Tauri Channels during every file transfer; the UI stays responsive and the progress bar updates continuously
- **Single-file downloads and folder transfers now show a progress bar** — previously these operations ran silently with only a spinner; they now show the same progress bar as multi-file uploads

### Added
- **Transfer speed display** — current speed (e.g. `2.1 MB/s`, `340 KB/s`) shown in the progress bar row, updated in real time during uploads and downloads
- **Byte-level progress fill** for SFTP transfers — the progress bar fill reflects actual bytes transferred within the current file rather than jumping from 0% to 100% per file
- **Per-file progress for folder operations** — uploading or downloading a folder now updates the current filename and speed continuously as each file is processed

---

## [1.3.0] - 2026-04-02

### Added
- **Activity log panel** — 5-line FileZilla-style log between the file list and action buttons showing live status: listing, uploading, downloading, connection events, errors
- **Cancel connection button** — spinner and cancel button appear in the toolbar while a connection is being established; disappear once connected or cancelled

### Improved
- **Transfer progress bar** — visual fill bar with percentage and current filename now shows during uploads and downloads, so long transfers give clear feedback instead of appearing frozen
- **Dark theme** redesigned with anthracite and neutral grey tones (replaces Catppuccin Mocha purples); light theme unchanged

---

## [1.2.0] - 2026-04-02

### Added
- **Multi-protocol support** — profiles now have a Protocol field: SSH (terminal + file browser), SFTP (file browser only), FTP (file browser only)
- **FTP file browser** — full file browser support over FTP: list, navigate, upload, download, rename, delete, new file/folder, all with the same UI as SFTP
- **FTP recursive operations** — recursive folder upload, download, and delete via FTP using a single persistent connection
- **FTP file editing** — remote file editing (open in editor + auto-upload on save) works over FTP in addition to SFTP
- **Protocol-aware auth** — FTP profiles always use password authentication; the auth selector is hidden for FTP; passphrase-never-saved rule is preserved
- **Protocol-aware port default** — selecting a protocol auto-fills the port field with 22 (SSH/SFTP) or 21 (FTP) when the field is empty or still at the previous default
- **Protocol + port in one row** — protocol dropdown and port input appear side by side in the profile form for a cleaner layout
- **Credential saving for FTP** — FTP passwords can be saved with the same local-machine / portable-profile / never tiers as SSH password auth

### Changed
- **Terminal button** hidden for SFTP and FTP profiles (terminal only makes sense for SSH)

---

## [1.1.0] - 2026-03-25

### Fixed
- **Wayland / AppImage compatibility** — added `WEBKIT_DISABLE_DMABUF_RENDERER=1` to the Linux startup path; resolves WebKit crashes on affected Wayland compositors when running via AppImage. X11 was already working correctly and is unaffected.

### Improved
- Continued UI and language improvements from v1.0.3–v1.0.4 (icon-only action bar, i18n cleanup)
- General stability and usability polish

### Notes
- The Wayland fix is applied unconditionally on Linux (including `.deb` installs); it only disables a GPU memory-sharing optimisation and has no visible effect on X11 or unaffected Wayland systems

---

## [1.0.4] - 2026-03-25

### Fixed
- Icon-only button rendering issues resolved — toolbar and action bar buttons now consistently render icon-only across all states (connected, disconnected, empty)
- Removed redundant visible labels from Upload, Upload Folder, New File, New Folder, and Terminal action buttons after icon migration
- Language file inconsistencies corrected — removed orphaned keys (`downloadFolder`, `downloadMultiple`, `deleteMultiple`) from EN and DE locales
- TypeScript locale type widened (`DeepStringify`) so the German locale no longer causes a type mismatch against the English base type

### Improved
- Better space usage in the action bar — all action buttons are now icon-only, reducing clutter and improving usability at narrow widths
- More consistent icon-based interface across toolbar and file browser actions

---

## [1.0.3] - 2026-03-25

### Added
- **Multilanguage support** — UI is now fully internationalised; English and German available; language is switched in Settings and persisted across sessions
- **SVG icon system** — all toolbar and file-browser action buttons now carry consistent Lucide-style inline SVG icons (no external dependency, theme-aware via `currentColor`)
- **File-browser context menus** — right-click on a file, folder, or empty area opens a context menu with the relevant actions (Download, Edit, Rename, Move to…, Delete; Open for folders; Upload, New File, New Folder, Refresh for empty space)
- **Tooltips on icon buttons** — every toolbar and action button exposes a `title` attribute for keyboard/screen-reader and hover discoverability

### Changed
- Action bar now wraps across multiple rows at narrow window widths, eliminating horizontal overflow
- Action buttons are more compact (smaller font, reduced padding, flex layout with icon + label)
- `＋ File` / `＋ Folder` labels replaced with `New File` / `New Folder` (DE: `Neue Datei` / `Neuer Ordner`) now that dedicated icons carry the visual signal
- Home, Up, and Refresh toolbar buttons are icon-only (Disconnect and Terminal retain icon + label for prominence)

### Known Issues
- Terminal may prompt for the SSH key passphrase again even when the session was already authenticated; this is a known limitation of the ControlMaster/key-agent flow and is being investigated
- AppImage compatibility has not been fully verified across all Linux configurations (Wayland compositors, certain GTK versions); `.deb` is the recommended install format for now
- Additional testing under both X11 and Wayland is required before these environments can be declared fully supported

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

[Unreleased]: https://github.com/andrenalin282/MurmurSSH/compare/v1.4.1...HEAD
[1.4.1]: https://github.com/andrenalin282/MurmurSSH/compare/v1.4.0...v1.4.1
[1.4.0]: https://github.com/andrenalin282/MurmurSSH/compare/v1.3.1...v1.4.0
[1.3.1]: https://github.com/andrenalin282/MurmurSSH/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/andrenalin282/MurmurSSH/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/andrenalin282/MurmurSSH/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/andrenalin282/MurmurSSH/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/andrenalin282/MurmurSSH/compare/v0.1.4-beta.1...v1.0.0
[0.1.4-beta.1]: https://github.com/andrenalin282/MurmurSSH/compare/v0.1.3-beta.2...v0.1.4-beta.1
[0.1.3-beta]: https://github.com/andrenalin282/MurmurSSH/releases/tag/v0.1.3-beta.2
[0.1.2-beta]: https://github.com/andrenalin282/MurmurSSH/releases/tag/v0.1.2-beta.1
[0.1.1-beta]: https://github.com/andrenalin282/MurmurSSH/releases/tag/v0.1.1-beta.1
[0.1.0-beta]: https://github.com/andrenalin282/MurmurSSH/releases/tag/v0.1.0
