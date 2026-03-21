# MurmurSSH

A minimal, open-source Linux desktop client for SSH and SFTP.

Built with [Tauri](https://tauri.app) and Rust. Designed for Ubuntu. Distributed as a `.deb` package.

## Features

- Local profile management — no cloud, no account, no telemetry
- SSH session launch via the system terminal (`x-terminal-emulator`)
- SFTP file browser — browse, upload, download, delete files
- Remote file editing — open a remote text file locally, save changes, upload back automatically or after confirmation
- Last-used profile restored on startup

## Requirements

- Ubuntu 22.04 or later (or compatible Debian-based Linux)
- [Rust](https://rustup.rs) (stable toolchain via rustup)
- Node.js 18+
- `x-terminal-emulator` (installed by default on most Ubuntu systems)
- `xdg-utils` (for opening files with system default editor; usually pre-installed)
- `libssh2-1` (runtime) and `libssh2-dev` (build time) for SFTP support
- Tauri system dependencies:

```bash
sudo apt install \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssh2-1-dev
```

## Development

```bash
# Install Node dependencies
npm install

# Start development build with hot reload
npm run tauri dev
```

## Building a .deb Package

Before building, generate icons from a 1024×1024 PNG source image:

```bash
npm run tauri icon path/to/icon.png
```

Then build the release package:

```bash
npm run tauri build
```

The `.deb` package is written to `src-tauri/target/release/bundle/deb/`.

## Managing Profiles

Profiles are managed through the sidebar UI. No JSON editing required.

**Create a profile:**
1. Click **New** in the sidebar
2. Fill in the required fields (name, host, port, username)
3. Choose an authentication method:
   - **SSH Key** — select a private key file using the Browse button
   - **SSH Agent** — connects via your running `ssh-agent`
4. Optionally set a default remote path, editor command, and upload mode
5. Click **Save**

**Edit a profile:** select it in the dropdown, click **Edit**.

**Delete a profile:** select it, click **Delete**, then confirm. Only the profile entry is removed — workspace files are not affected.

Profiles are stored as JSON files in `~/.config/murmurssh/profiles/` and can also be edited manually if needed.

### Authentication

Only two methods are supported:

- **SSH Key** (`auth_type: "key"`) — the path to an unencrypted private key is stored in the profile. The key file must exist at connect time.
- **SSH Agent** (`auth_type: "agent"`) — delegates to a running `ssh-agent`. No key path is stored.

Password authentication is not supported. Key generation is not included — generate keys with `ssh-keygen` beforehand.

### Limitations

- Encrypted private keys (passphrase-protected) are not supported
- Only one profile can be active at a time

## Configuration

All data is stored locally in `~/.config/murmurssh/`.

```
~/.config/murmurssh/
  profiles/      # One JSON file per saved profile
  settings.json  # App settings (last used profile, etc.)
  workspace/     # Temporary cache for files opened for editing
  logs/          # Application logs
```

### Profile Format

Profiles are plain JSON files in `~/.config/murmurssh/profiles/`.

Example (`~/.config/murmurssh/profiles/my-server.json`):

```json
{
  "id": "my-server",
  "name": "My Server",
  "host": "192.168.1.100",
  "port": 22,
  "username": "kai",
  "auth_type": "key",
  "key_path": "/home/kai/.ssh/id_ed25519",
  "default_remote_path": "/home/kai",
  "editor_command": null,
  "upload_mode": "confirm"
}
```

- `auth_type`: `"key"` (SSH private key) or `"agent"` (SSH agent)
- `key_path`: path to private key; required when `auth_type` is `"key"`, otherwise `null`
- `upload_mode`: `"auto"` (upload immediately on save) or `"confirm"` (prompt before upload)
- `editor_command`: optional command to open files, e.g. `"code"`, `"gedit"`, `"vim"`. If `null`, uses `xdg-open` (system default)

## SFTP File Browser

After connecting to a profile:

- **Browse**: Click on directory rows to navigate. Double-click a directory to enter it. Click `..` to go up.
- **Upload**: Click **Upload** to pick a local file. It is uploaded to the current remote directory.
- **Download**: Select a file, click **Download**. File is saved to `~/Downloads/<filename>`.
- **Edit**: Select a text file, click **Edit**. The file opens in your configured editor. When you save, it uploads back to the server automatically (auto mode) or after confirmation (confirm mode).
- **Delete**: Select a file, click **Delete**. Confirmation is required.

### Limitations

- Upload uses the browser's file picker; for large files, consider direct `scp` instead
- Downloads always go to `~/Downloads/`. A naming conflict will overwrite the existing file.
- Directory delete is not implemented (remove contents first, then delete via SSH)
- Binary files and files > 1 MB cannot be opened for editing; use Download instead
- Each SFTP operation opens a fresh connection. This is simple and correct but not optimised for rapid sequential operations.
- The file watcher (for edit flow) runs one background thread per opened file and lives until the app exits or the file is deleted

## Project Structure

```
src/                        # TypeScript frontend (vanilla, no framework)
  api/index.ts              # Typed wrappers around Tauri IPC invoke calls
  components/               # UI components (plain DOM manipulation)
  types.ts                  # Shared TypeScript types
  main.ts                   # App entry point and event listeners
src-tauri/
  src/
    models/                 # Rust data types (Profile, Settings, FileEntry)
    services/               # Business logic (profile I/O, SSH launch, SFTP, workspace)
    commands/               # Tauri IPC command handlers
    lib.rs                  # Command registration
    main.rs                 # Binary entry point
```

## Contributing

Read `PRD.md` before adding anything. The project is intentionally small.

- Scope rules: `.claude/skills/product-scope.md`
- Architecture rules: `.claude/skills/architecture-rules.md`

Keep it simple. Stay within MVP scope. Prefer less code over more code.
