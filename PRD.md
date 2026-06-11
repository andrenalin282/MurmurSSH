# MurmurSSH – Product Requirements Document

## 1. Overview

MurmurSSH is a minimal, open-source Linux desktop client for SSH and SFTP.
The application is designed primarily for Ubuntu and will be distributed as a `.deb` package.

The product goal is to provide a lightweight and simple alternative to larger SSH/SFTP clients such as Bitvise or Termius, while remaining intentionally focused on a small MVP feature set.

MurmurSSH is local-first:
- all profiles are stored locally
- no cloud sync
- no server backend
- no telemetry by default

## 2. Product Goals

- Provide a very small graphical interface for managing SSH/SFTP connection profiles
- Allow users to load a saved profile and connect quickly
- Use the Linux system terminal for SSH sessions instead of implementing a custom terminal
- Provide integrated SFTP browsing and basic file transfer
- Allow remote text files to be opened locally, edited, and uploaded back automatically or after confirmation
- Keep the architecture simple enough for open-source community contributions

## 3. Non-Goals

The following are explicitly out of scope for the MVP:
- custom built-in terminal emulator
- cloud sync
- account system
- remote sync engine
- port forwarding / tunnels
- SSH snippet library
- secret vault
- team sharing
- plugin system
- versioned backups of edited files
- support for protocols other than SSH/SFTP
- Windows and macOS support
- AppImage distribution
- embedded database unless truly necessary

## 4. Target Platform

Primary target:
- Ubuntu Linux desktop

Initial release format:
- `.deb`

Possible later community extensions:
- other Debian-based distributions
- AppImage
- broader Linux desktop support

## 5. Target Users

- Linux users who regularly connect to remote servers
- developers
- administrators
- power users who want a simple profile-based SSH/SFTP desktop tool
- users who prefer local configuration files and open-source software

## 6. Core Product Principles

- minimal UI
- profile-first workflow
- local-first storage
- simple over clever
- stable over feature-rich
- community-friendly code structure
- no unnecessary abstraction
- no forced cloud or vendor lock-in

## 7. MVP Scope

### 7.1 Profiles
The application must support local connection profiles.

Each profile should include at minimum:
- display name
- host
- port
- username
- authentication type
- SSH private key path
- optional default remote path
- optional editor command
- upload mode for edited files (`auto` or `confirm`)

Rules:
- the user always works through a saved profile
- no quick-connect flow in MVP
- on startup, the last used profile should be loaded automatically

### 7.2 SSH Session Launch
The application must be able to start an SSH session by opening the Linux system terminal and executing the appropriate SSH command.

Rules:
- do not implement a custom terminal renderer
- prefer SSH key authentication
- optional SSH agent support is allowed if simple to integrate
- password-based login is not a priority for the first implementation

### 7.3 SFTP File Browser
The application must provide a graphical file browser for remote files over SFTP.

MVP operations:
- browse directories
- refresh directory listing
- upload one file
- download one file
- delete file
- rename file
- create directory

Nice-to-have only if trivial:
- multi-select upload/download

### 7.4 Local/Remote Editing Flow
The application must support opening a remote text file for editing.

Required flow:
1. user selects a remote file
2. file is downloaded into a local workspace/cache area
3. file is opened in a local editor
4. file changes are detected
5. on save, the file is uploaded back automatically or after confirmation depending on profile setting

Rules:
- no backup/versioning in MVP
- no binary file editing in MVP
- large file warnings may be added if easy to implement

### 7.5 Local Storage
All application data should remain local.

Expected storage area:
- `~/.config/murmurssh/`

Suggested structure:
- `profiles/`
- `settings.json`
- `workspace/`
- `logs/`

## 8. UX Requirements

The interface should remain intentionally small.

Suggested layout:
- profile list or profile selector
- connect button
- connection status
- SFTP file browser area
- actions for upload/download/edit

UX expectations:
- startup should restore the last used profile
- connecting should require minimal clicks
- status should be visible and understandable
- errors should be shown clearly without technical overload where possible

## 9. Security Requirements

- SSH host verification should be respected
- SSH keys should be supported by path
- avoid storing secrets insecurely
- no cloud storage of credentials
- no telemetry in MVP

## 10. Open Source Requirements

MurmurSSH is intended as an open-source project.

Requirements:
- code should be easy to understand
- architecture should be modular but not over-engineered
- dependencies should be kept minimal
- documentation should explain setup and contribution clearly
- community contributions should not require understanding unnecessary complexity

## 11. Technical Direction

Preferred stack:
- Tauri
- Rust backend
- lightweight frontend GUI (TypeScript-based UI is acceptable)

Rationale:
- Linux desktop suitability
- access to native file/system operations
- future `.deb` packaging
- lower overhead than Electron

## 12. Acceptance Criteria for MVP

The MVP is complete when:

1. A user can create and save a profile locally
2. The application loads the last used profile on startup
3. A user can launch an SSH connection from the profile in the Linux system terminal
4. A user can browse remote directories over SFTP
5. A user can upload and download files
6. A user can open a remote text file locally
7. A changed file can be uploaded back automatically or after confirmation
8. The app can be packaged as a `.deb`
9. The project remains documented and understandable for open-source contributors

## 13. Deferred Features

Not part of the MVP, but acceptable for later discussion:
- multi-file transfers
- profile import/export
- AppImage
- stronger credential integration
- advanced SSH options
- port forwarding
- diff viewer
- integrated text editor

## 14. Delivery and Implementation Rules

Implementation should follow a disciplined phase-based process.

### Delivery principles

- work in small phases with clear goals
- do not silently expand scope beyond the current phase
- preserve the minimal Linux-first product direction
- keep the implementation understandable for open-source contributors
- prefer incremental improvements over broad rewrites

### Required implementation approach

For any non-trivial change, the work should follow this order:

1. read project guidance and current phase instructions
2. identify constraints, dependencies, and affected files
3. break the work into smaller implementation steps
4. use sub-agents for complex or cross-layer tasks; also use inline fix sub-agents when compile errors, regressions, or state bugs appear mid-implementation with unclear root cause; parallel sub-agents are appropriate for independent work streams (e.g. Rust backend and TypeScript frontend changes that do not share state)
5. implement in small, verifiable batches
6. explicitly review and resolve issues, warnings, and regressions
7. validate the affected user flows before considering the work complete

### Issue handling

Problems found during implementation should not be hidden.

They should be:
- listed clearly
- resolved in priority order
- documented if deferred

This includes:
- logic bugs
- validation gaps
- state inconsistencies
- warnings that indicate real maintenance risk
- incomplete edge-case handling

### Validation before completion

A phase should not be considered complete until:

- the defined requirements for that phase are implemented
- the affected flows are checked end-to-end as far as practical
- architectural constraints remain preserved
- any remaining limitations are explicitly documented

### Definition of done

A phase is considered done when:

- the work satisfies the phase goal
- scope has been respected
- architecture remains aligned with project rules
- user-visible behavior is coherent and minimal
- important warnings or issues have been resolved or documented
- the final output clearly lists changes, validation, and remaining known limitations

---

## 15. Future Roadmap (post-v1.6.0)

This section captures the planned next steps **after v1.6.0**. It is forward-looking:
the items below are not yet implemented. The MVP (sections 1–14) is complete and the
product has since grown beyond it (see status snapshot). Detailed, locked design lives in
`docs/superpowers/specs/2026-06-10-optimization-roadmap-design.md`; per-phase plans are
authored just-in-time under `docs/superpowers/plans/`; live progress is tracked in
`vault/MurmurSSH Optimization Roadmap.md`.

### 15.1 Status snapshot (shipped beyond the original MVP)

For context — these went beyond the original Non-Goals and are now shipped:
- FTP support (file browser only), AppImage distribution, multi-protocol port defaults.
- Multi-file/folder transfers, drag-and-drop, recursive upload/download.
- Credential storage tiers (never / local-machine / portable), host-key verification.
- Split local+remote file browsers, keyboard shortcuts, 6-language i18n.
- **Phase 1 (v1.4.8):** file-list Modified + Permissions columns, remote chmod dialog.
- **Phase 0 (v1.4.7):** remote-edit content-hash baseline fix, app-exit cleanup.
- **Phase 2 (v1.5.0):** background transfer queue with configurable concurrency (1–8),
  per-job cancel, queue panel — keeps the UI responsive during large transfers.
- **Phase 3 (v1.6.0):** profile groups (collapsible grouped tree), persisted sort toggle
  (A–Z | Newest), `created_at` stamping, group field in the profile form.

### 15.2 Phase 4 — FileZilla import (next up)

**Goal:** Let users migrate from FileZilla by importing its Site Manager.

Requirements:
- Parse `~/.config/filezilla/sitemanager.xml` (allow a custom file picker).
- Map FileZilla `<Folder>` nesting → the profile `group` field (flattened path, e.g.
  `Parent/Child`); `<Server>` → a MurmurSSH profile (host, port, user, name, protocol).
- **Security (locked decision): saved passwords are NOT imported.** Imported profiles use
  `credential_storage_mode = never`; credentials are prompted at connect time.
- Conflict handling: skip or suffix duplicate IDs; show an import summary (created /
  skipped / failed), reusing the existing SSH-config import summary UX where practical.
- Graceful handling of malformed/partial XML — never crash, report clearly.
- Depends on the Phase 3 group model (now available).

Acceptance: importing a real `sitemanager.xml` creates profiles grouped by their FileZilla
folders; no credentials are persisted; a summary is shown; malformed XML is handled cleanly.

### 15.3 Phase 5 — Editor configuration

**Goal:** Control which editor opens remote/local files for editing, globally and per type.

Requirements:
- Add `default_editor: Option<String>` and `editor_by_extension: HashMap<String,String>`
  to Settings.
- Resolution order in the open-in-editor flow: per-profile `editor_command` →
  per-extension map (by lowercased extension) → global default → `xdg-open`.
- Settings UI: a global default-editor field plus an editable extension→editor list
  (add/remove rows). Used by both remote-edit and the local browser's edit action.

Acceptance: a `.conf` file opens in the configured per-extension editor; a profile override
still wins; unmapped types fall back to the global default, then `xdg-open`.

### 15.4 Execution process (applies to every future phase)

- Read guidance files; author a just-in-time plan under `docs/superpowers/plans/`.
- Execute subagent-driven: fresh implementer per task + two-stage review (spec, then
  quality) + an Opus final holistic review before release. Per-task model assignment
  (Haiku = mechanical/i18n/docs, Sonnet = implementation, Opus = architecture/review).
- Commit each logical step separately; **always update the README** plus CHANGELOG,
  CLAUDE.md, and the vault roadmap note.
- At each phase boundary: version bump across `package.json` / `Cargo.toml` /
  `tauri.conf.json`, annotated git tag, push to `main`, then refresh the gitnexus index.
- Frontend build rule: edit `.ts` → `npx tsc` → commit both the `.ts` and its tracked
  `.js` sibling; revert unrelated regenerated `.js` (recurring `src/i18n/index.js` noise).

### 15.5 Still out of scope (unchanged)

- Separate sidecar transfer process; true file birth-time (unavailable over SFTP v3);
  chown/ownership editing; importing FileZilla saved passwords; nested multi-level group
  hierarchies beyond a single flattened `group` string; custom embedded terminal; cloud
  sync / accounts / telemetry; Windows/macOS support.
