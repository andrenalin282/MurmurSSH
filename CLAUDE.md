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
    dialog.ts               # showConfirm() — promise-based in-app modal
    file-browser.ts         # SFTP file browser
    profile-form.ts         # Profile create/edit modal form
    profile-selector.ts     # Profile dropdown + management buttons (New/Edit/Delete)
    status-bar.ts           # Connection status indicator
  types.ts                  # Shared TypeScript types (mirrors Rust models)
  main.ts                   # App entry point — wires all components together
src-tauri/src/
  models/                   # Rust data types: Profile, Settings, FileEntry
  services/                 # Business logic — one file per concern
  commands/                 # Tauri IPC layer — thin wrappers over services
  lib.rs                    # Registers all commands with Tauri builder
  main.rs                   # Binary entry point
```

Profiles are stored as individual JSON files in `~/.config/murmurssh/profiles/`.
Settings live in `~/.config/murmurssh/settings.json`.

SSH sessions are launched via `x-terminal-emulator -e ssh ...` — no embedded terminal.

SFTP (`sftp_service`) uses the `ssh2` crate. Each operation opens a fresh connection — simple, no shared session state.

The workspace/edit flow (`workspace_service`) downloads remote files to `~/.config/murmurssh/workspace/<profile_id>/`, opens them with `xdg-open` or a configured editor command, and watches for saves with `notify` (inotify on Linux). On save: auto-upload or emit `upload-ready` event for confirm mode.

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

## Guidance Files

Before implementing anything, read all of the following:

- `PRD.md` — full product requirements and acceptance criteria
- `.claude/skills/product-scope.md` — scope rules and decision bias
- `.claude/skills/architecture-rules.md` — structure, module, and persistence rules
- `.claude/skills/linux-integration.md` — platform, terminal, and filesystem conventions
- `.claude/skills/open-source-guidelines.md` — code and community expectations
