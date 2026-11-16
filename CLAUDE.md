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
    credential-dialog.ts    # Password/passphrase/host-key dialogs + save mode choice
    dialog.ts               # showConfirm() — promise-based in-app modal
    file-browser.ts         # SFTP file browser (toolbar: Disconnect/Terminal/Home/Up/Refresh)
    profile-form.ts         # Profile create/edit modal form
    profile-selector.ts     # Profile dropdown + management buttons (New/Edit/Delete)
    settings-dialog.ts      # Settings modal: storage path, theme, editors, concurrency, …
    status-bar.ts           # Connection status indicator
  types.ts                  # Shared TypeScript types (mirrors Rust models)
  main.ts                   # App entry point — wires all components together
src-tauri/src/
  models/                   # Rust data types: Profile, Settings, FileEntry
  services/                 # Business logic — one file per concern
    credentials_store.rs    # Session-only in-memory credential cache (never on disk)
    known_hosts_service.rs  # Host key verification (~/.config/murmurssh/known_hosts)
    secrets_service.rs      # Local-machine persistent secret files (~/.config/murmurssh/secrets/)
    sftp_service.rs         # SFTP operations + connect() with host-key/auth logic
    ssh_session_service.rs  # SSH SSO: ControlMaster (password auth) / ssh-agent (key+passphrase)
    workspace_service.rs    # Remote editing, watcher dedup via active_watchers()
  commands/                 # Tauri IPC layer — thin wrappers over services
    connection.rs           # connect_sftp, accept_host_key, save_credential, clear_credential
  lib.rs                    # Registers all commands with Tauri builder
  main.rs                   # Binary entry point
```

Profiles are stored as individual JSON files in `~/.config/murmurssh/profiles/`.
Settings live in `~/.config/murmurssh/settings.json`.
Known hosts stored in `~/.config/murmurssh/known_hosts` (format: `hostname:port SHA256:hex`).

SSH sessions are launched via `x-terminal-emulator -e ssh ...` — no embedded terminal. The terminal handles its own auth interactively; the MurmurSSH known_hosts and credentials only apply to SFTP connections.

SFTP (`sftp_service`) uses the `ssh2` crate. Each operation opens a fresh connection — simple, no shared session state. `connect()` enforces host key verification and authentication in order: TCP → handshake → known_hosts check → auth.

**Auth types**: `key` (pubkey file, passphrase optional), `agent` (ssh-agent), `password` (runtime prompt).

**Connection flow**: frontend calls `api.connectSftp(profileId)` which returns structured errors handled by `verifyConnection()` in `main.ts`:
- `"UNKNOWN_HOST:<fp>"` → show host key dialog → `api.acceptHostKey()` → retry
- `"NEED_PASSWORD"` → show password prompt with save-mode choice → retry with credential
- `"NEED_PASSPHRASE"` → show passphrase prompt with save-mode choice → retry with credential

On successful auth after a prompt, if the user chose a save mode != "never", `main.ts` calls `api.saveCredential(profileId, secret, mode)`.

**Credential storage tiers** (security, highest → lowest):
- `never` — prompted every connection; nothing written to disk (default)
- `local_machine` — plaintext file at `~/.config/murmurssh/secrets/<profile_id>`, 0600 perms; does not travel with profile JSON
- `portable_profile` — plaintext inside the profile JSON field `stored_secret_portable`; portable but clearly labeled as less secure

**Passphrase rule (enforced at all layers)**: SSH key passphrases are NEVER persisted. `showPassphrasePrompt` returns `string | null` with no save option. `save_credential` in the backend no-ops for any non-password auth profile. Auto-load in `connect_sftp` only triggers for `AuthType::Password`. Passphrases are always prompted at connection time and discarded after use.

Runtime (session-only) credentials are in `credentials_store` (`OnceLock<Mutex<HashMap>>`). On `connect_sftp`, if the session store is empty **and auth type is Password**, the backend auto-loads from persistent storage before calling `sftp_service::test_connection()`.

**Credential cleanup rules**:
- `clear_credential` removes local file + portable field + resets metadata + clears session store
- `delete_profile` removes local secret file + clears session store before deleting the profile JSON
- Switching a profile from password to key/agent auth clears all stored credentials (both frontend and backend enforce this)

The workspace/edit flow (`workspace_service`) downloads remote files to `~/.config/murmurssh/workspace/<profile_id>/`, opens them via `editor_service` resolution (profile `editor_command` → Settings `editor_by_extension` → `default_editor` → `xdg-open`), and watches for saves with `notify` (inotify on Linux). On save: auto-upload or emit `upload-ready` event for confirm mode. Duplicate watchers for the same file are prevented via `active_watchers()` registry.

Tauri events used by the workspace flow:
- `upload-ready` → payload: `{ profile_id, local_path, remote_path }` — frontend asks user to confirm
- `upload-complete` → payload: remote path string — auto-upload succeeded
- `upload-error` → payload: error string — auto-upload failed

File picker for SSH key paths uses `tauri-plugin-dialog` (Rust) + `@tauri-apps/plugin-dialog` (JS). Capability: `dialog:allow-open` in `src-tauri/capabilities/default.json`.

Profile IDs are generated from the display name: lowercase, non-alphanumeric sequences replaced with `-`, leading/trailing dashes stripped. IDs are immutable after creation.

## Phases Complete

Full phase-by-phase development history (phases 1–16) lives in [`docs/PHASES.md`](docs/PHASES.md)
and in git. `CHANGELOG.md` holds the user-facing release notes. Kept out of this file to
stay lean.

## Guidance Files

Before implementing anything, read all of the following:

- `PRD.md` — full product requirements and acceptance criteria
- `.claude/skills/product-scope.md` — scope rules and decision bias
- `.claude/skills/architecture-rules.md` — structure, module, and persistence rules
- `.claude/skills/linux-integration.md` — platform, terminal, and filesystem conventions
- `.claude/skills/open-source-guidelines.md` — code and community expectations
- `.claude/skills/execution-workflow.md` — execution order, sub-agent usage, issue logging, validation, and final reporting rules

## Execution Rules

When implementing anything in this repository:

- first read all guidance files listed above
- extract constraints before making changes
- identify affected files by layer before editing
- decompose non-trivial work into small implementation steps
- use sub-agents for complex, cross-layer, or unclear-root-cause tasks
- maintain a live issue/finding list during the work
- resolve errors and warnings explicitly instead of silently working around them
- validate requirements and affected flows before declaring completion
- report files changed, root causes, validation, and remaining issues in the final summary

Sub-agents are especially expected when work crosses:
- `src/` and `src-tauri/src/`
- UI + backend + persistence
- security/authentication/secret handling
- multiple core modules with shared state or compatibility impact

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **MurmurSSH** (2587 symbols, 8062 relationships, 223 execution flows).

> Index stale? Run `node .gitnexus/run.cjs analyze --index-only` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? Bootstrap with `npx`, `bunx`, or `pnpm dlx` — e.g. `bunx gitnexus@latest analyze` (npm 11 npx crash; #1939).

## Always Do

- **MUST run impact before editing.** Use `impact({target: "symbolName", direction: "upstream"})` or `node .gitnexus/run.cjs impact "symbolName" --direction upstream --repo .`; report callers, processes, and risk. Never substitute grep for graph analysis.
- **MUST analyze graph changes before committing.** Use `detect_changes({scope: "all"})` (MCP) or `node .gitnexus/run.cjs detect-changes --scope all --repo .` (CLI fallback). `partial: true` or `truncated: true` is not a clean check — a zero means unseen, not unaffected; re-run it. For regression review: `detect_changes({scope: "compare", base_ref: "main"})` or `node .gitnexus/run.cjs detect-changes --scope compare --base-ref "main" --repo .`.
- MUST warn on HIGH/CRITICAL `risk` pre-edit; never use `riskSharedAxes` to waive a HIGH/CRITICAL `risk` warning. Compare File/symbol: MCP File omits axes; Graph-RAG expands File.
- **MUST treat `risk: UNKNOWN` as unresolved, not as low.** An empty caller set is not evidence the symbol is unused — it can also mean the callers are not resolvable by the index (plain-object property access, dynamic dispatch, cross-language calls). `impact` pairs `UNKNOWN` with a `riskNote` saying so. Confirm with a text search before treating the symbol as safe to change or delete; do not proceed on the strength of a zero.
- **MUST use `query({search_query: "concept"})` for concepts/flows, `context({name: "symbolName"})` for a named symbol, or `impact` for blast radius, on read-only callers, dependencies, imports, or execution flow.** Graph first; text search only for empty/`UNKNOWN`/literals.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method before MCP/CLI impact analysis.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis, and never read `UNKNOWN` as an all-clear — it means the walk could not answer, which is the one verdict that requires confirming by other means.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit before MCP/CLI graph change analysis.

## Resources

| Resource | Use for |
| --- | --- |
| `gitnexus://repo/MurmurSSH/context` | Codebase overview, check index freshness |
| `gitnexus://repo/MurmurSSH/clusters` | All functional areas |
| `gitnexus://repo/MurmurSSH/processes` | All execution flows |
| `gitnexus://repo/MurmurSSH/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
| --- | --- |
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
