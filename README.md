# MurmurSSH

**A minimal, open-source SSH and SFTP client for Linux.**

MurmurSSH is a lightweight desktop application for managing SSH connections and browsing remote files over SFTP. It is designed for Linux users who want a simple, profile-based tool without cloud accounts, telemetry, or unnecessary complexity.

Built with [Tauri](https://tauri.app) and Rust. Free to use, free to modify, free to contribute to.

---

## What it does

- **Profile management** — save connection profiles locally with name, host, port, username, and auth settings. No account required.
- **SSH sessions** — launch an SSH connection directly in your system terminal with one click.
- **SFTP file browser** — browse remote directories, upload files, download files, delete files and folders (recursive), rename files, create directories and files.
- **Remote file editing** — open a remote text file in your local editor. When you save, MurmurSSH uploads the changes back automatically or asks for confirmation first.
- **Multiple auth methods** — SSH key, SSH agent, or password authentication.
- **Optional password saving** — choose whether to save a password locally (machine-only) or inside the profile file (portable), or not at all. SSH key passphrases are never saved.
- **Host key verification** — unknown host keys are shown with their fingerprint before you accept them. Trusted keys are stored locally.
- **Fully local** — all profiles, settings, and credentials stay on your machine. No cloud, no sync, no telemetry.

---

## Screenshots

*Coming soon.*

---

## Installation

### Requirements

- Ubuntu 22.04 or later, or any compatible Debian-based Linux
- `x-terminal-emulator` (pre-installed on most Ubuntu systems)
- `xdg-utils` (pre-installed on most Ubuntu systems)

### Install system dependencies (build only)

```bash
sudo apt install \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssh2-1-dev
```

### Install from .deb

Download the latest `.deb` release from the [Releases](../../releases) page and install it:

```bash
sudo dpkg -i murmurssh_*.deb
```

---

## Building from source

You need [Rust](https://rustup.rs) (stable toolchain) and Node.js 18 or later.

```bash
# Clone the repository
git clone https://github.com/your-org/murmurssh.git
cd murmurssh

# Install Node dependencies
npm install

# Start in development mode with hot reload
npm run tauri dev
```

To build a release `.deb` package:

```bash
# Generate icons from a 1024×1024 PNG
npm run tauri icon path/to/icon.png

# Build
npm run tauri build
```

The `.deb` package is written to `src-tauri/target/release/bundle/deb/`.

---

## Usage

### Profiles

Create a profile for each server you connect to:

1. Click **New** in the sidebar
2. Enter the display name, host, port, and username
3. Choose an authentication method:
   - **SSH Key** — pick your private key file with the Browse button
   - **SSH Agent** — delegates to your running `ssh-agent`
   - **Password** — entered at connection time; you can choose whether to save it
4. Optionally set:
   - **Default Remote Path** — the directory opened when you connect
   - **Local Path** — a local folder used as the default for uploads and downloads
   - **Editor Command** — the command used to open files for editing (blank = system default)
   - **Upload Mode** — confirm before upload, or auto-upload on file save
5. Click **Save**

The last used profile is restored automatically on startup.

### Connecting

Select a profile and click **Connect**. MurmurSSH will:

1. Verify the host key (or ask you to trust it on first connect)
2. Authenticate (prompt for password or passphrase if needed)
3. Open an SSH session in your system terminal
4. Load the SFTP file browser

### SFTP file browser

| Action | How |
|---|---|
| Navigate | Click a directory row |
| Go up | Click `..` |
| Navigate | Click a directory row or type a path in the path input and press Enter |
| Go up | Click `..` or the **Up** button |
| Upload | Click **Upload** → file picker opens, starts in your configured local path if set |
| Download | Select a file → **Download** → saves to your configured local path, or opens a save dialog |
| Edit | Select a text file → **Edit** → opens in your editor → saves back on file save |
| Delete file | Select a file → **Delete** → confirm |
| Delete folder | Select a folder → **Delete** → confirm recursive deletion |
| New file | Click **＋ File** → enter a name |
| New folder | Click **＋ Folder** → enter a name |

### Credential storage

When connecting with password authentication, you choose how to handle the password:

| Option | What happens |
|---|---|
| **Don't save** (default) | Prompted every time. Nothing written to disk. |
| **Save on this PC only** | Plaintext file at `~/.config/murmurssh/secrets/` with `0600` permissions. Does not travel with the profile. |
| **Save in profile file** | Plaintext inside the profile JSON. Portable to other PCs, but anyone with access to the file can read it. |

SSH key passphrases are **never saved**. They are prompted at connection time and discarded immediately after.

To clear a saved password: open the profile in **Edit** → **Clear Saved Credential**.

### SSH key compatibility

If your SSH private key is stored on a mounted or network filesystem, the system `ssh` client may reject it with "UNPROTECTED PRIVATE KEY FILE" because the filesystem does not honour UNIX file permissions as expected by OpenSSH.

When this happens, MurmurSSH will prompt you to create a local runtime copy of the key:

- The copy is stored in `~/.config/murmurssh/runtime-keys/` with `0600` permissions
- The terminal session uses the copy instead of the original
- The original key file is **never modified**
- The copy is **temporary** — it is deleted when you disconnect or when the app starts up

No passphrase is stored. If the key requires a passphrase, the terminal will still prompt you interactively.

---

## Configuration

All data is stored locally in `~/.config/murmurssh/`:

```
~/.config/murmurssh/
  profiles/        # One JSON file per saved profile
  settings.json    # App settings (last used profile, etc.)
  secrets/         # Machine-local saved passwords (0600, never synced)
  workspace/       # Local cache of files opened for editing
  known_hosts      # Accepted SSH host key fingerprints
  runtime-keys/    # Temporary key copies for terminal compatibility (0600, deleted on disconnect)
  logs/            # Application logs
```

Profiles are plain JSON and can be edited manually or copied between machines.

---

## Project structure

```
src/                    # Vanilla TypeScript frontend (no framework)
  api/index.ts          # Typed wrappers for all Tauri IPC commands
  components/           # DOM-based UI components
  types.ts              # Shared TypeScript types
  main.ts               # App entry point
src-tauri/src/
  models/               # Rust data types (Profile, Settings, FileEntry)
  services/             # Business logic (SSH, SFTP, profiles, secrets, workspace)
  commands/             # Tauri IPC handlers
  lib.rs                # Command registration
```

---

## Contributing

MurmurSSH is free and open source. Contributions are very welcome.

Whether it's a bug report, a small fix, a usability improvement, or documentation — all contributions are appreciated.

**Before opening a pull request:**

- Read `PRD.md` — it defines what this project is and is not
- Keep changes focused and minimal
- Prefer simple solutions over clever ones
- Stay within the existing architecture

**Good first areas to contribute:**

- Bug reports and reproducible test cases
- UI polish and accessibility improvements
- Documentation improvements
- Platform testing on Debian, Fedora, or other distributions
- Translations (if the project eventually supports them)

Please open an issue first for larger changes so we can discuss the approach before you invest time in it.

---

## License

MurmurSSH is released under the [MIT License](LICENSE).

---

## Known limitations

- Only one profile can be active at a time
- Folder deletion is recursive and permanent — there is no undo or trash recovery
- Binary files and files larger than 1 MB cannot be opened for editing
- Each SFTP operation opens a fresh connection — not optimised for rapid sequential use
- No Windows or macOS support — Linux only, by design
- This is a beta release — please report bugs via the issue tracker
