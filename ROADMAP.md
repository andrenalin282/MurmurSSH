# Roadmap

MurmurSSH is intentionally minimal. The goal is a reliable, local-first SSH/SFTP client for Linux — not a feature-complete enterprise tool.

This roadmap reflects realistic near-term priorities and longer-term ideas. It will be updated as the project evolves.

---

## Near Term (v1.0 → v1.1)

These are the next concrete improvements:

- **ARM builds** — additional `.deb` and `.AppImage` for arm64 / Raspberry Pi
- **Rename via UI** — rename files and directories directly in the file browser (backend already supports it)
- **Improved error messages** — more actionable SSH and SFTP error descriptions
- **Keyboard navigation** — arrow keys and Enter in the file browser table
- **Download progress** — show progress indicator for larger file downloads

---

## Planned (v1.1 → v1.x)

These features are planned but not yet started:

- **Multiple simultaneous profiles** — connect to more than one server at a time in separate tabs or panels
- **File permissions display** — show Unix permission bits in the file browser
- **Drag-and-drop upload** — drag files from the desktop into the browser
- **Profile import/export** — backup and restore profiles as a single archive
- **Flatpak packaging** — alternative to `.deb` for broader Linux distribution support
- **Configurable terminal** — let users choose which terminal emulator to use
- **Session reconnect** — detect a dropped connection and offer to reconnect

---

## Ideas / Wishlist

These are not committed to — just tracked for discussion:

- Port forwarding / tunnels
- Bookmarks for frequently visited directories
- Diff view for edited remote files before re-upload
- Dark/light theme per-profile
- Plugin or extension system
- Windows or macOS support (out of scope for current maintainers, welcome as community effort)

---

Issues and feature requests are tracked on [GitHub Issues](https://github.com/andrenalin282/MurmurSSH/issues).
Label `wishlist` is used for ideas not yet planned.
