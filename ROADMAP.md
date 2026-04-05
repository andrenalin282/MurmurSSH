# Roadmap

MurmurSSH is intentionally minimal. The goal is a reliable, local-first SSH/SFTP client for Linux — not a feature-complete enterprise tool.

This roadmap reflects realistic near-term priorities and longer-term ideas. It will be updated as the project evolves.

---

## Near Term (v1.4 → v1.5)

These are the next concrete improvements:

- **ARM builds** — additional `.deb` and `.AppImage` for arm64 / Raspberry Pi
- **Improved error messages** — more actionable SSH and SFTP error descriptions
- **Resizable split pane** — drag the divider between local and remote browsers to adjust proportions
- **Local browser folder support for drag-out** — currently only files can be dragged from the local browser; extend to folders

---

## Planned (v1.5+)

These features are planned but not yet started:

- **Multiple simultaneous profiles** — connect to more than one server at a time in separate tabs or panels
- **File permissions display** — show Unix permission bits in the file browser
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
