# Changelog

All notable changes to MurmurSSH are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added
- Per-profile local path for downloads and uploads
- Download save dialog when no local path is configured
- Upload file picker starts in configured local path
- Edit/Delete profile buttons locked while a session is active
- New File and New Folder actions in the file browser
- Editable path input in the file browser (Enter to navigate, Escape to cancel)
- Symlinks-to-directories shown as navigable folders in the file browser
- Dark/light/system theme selector in settings
- SSH SSO via ControlMaster (password auth) and ssh-agent forwarding (key auth)
- Configurable profile storage path in settings
- Host key verification dialog (Accept once / Accept and save / Cancel)
- Credential storage modes: never / local machine only / portable profile
- Profile backup on save (.json.bkp)

---

## [0.1.0] — TBD

_First public release._

### Added
- Profile management (create, edit, delete, persist as local JSON)
- SSH session launch via system terminal (`x-terminal-emulator`)
- SFTP file browser (list, navigate, upload, download, delete, rename)
- Remote file editing with automatic or confirm-mode re-upload
- SSH key, SSH agent, and password authentication
- Host key fingerprint verification and known_hosts storage
- Breadcrumb navigation and manual path input
- Connection status bar
- Settings dialog (theme, profile storage path)
- MIT License

---

[Unreleased]: https://github.com/your-org/murmurssh/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/your-org/murmurssh/releases/tag/v0.1.0
