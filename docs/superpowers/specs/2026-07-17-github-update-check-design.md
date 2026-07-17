# Design — GitHub release update check

Date: 2026-07-17
Status: Approved (pending written-spec review)

## Summary

Add a lightweight update check that compares the running MurmurSSH version against the
latest GitHub Release. Users can:

1. Enable/disable **check on startup** in Settings (default: **on**).
2. Click **Check now** in Settings for an immediate check with always-visible feedback.
3. When an update exists, see a dialog with current vs latest version and a button that
   opens the GitHub Releases page (no auto-download / auto-install).

Ship as part of **v1.7.2**.

## Background / current state

- Tauri 2 Linux app. Version comes from package info via existing `get_app_version`
  (`src-tauri/src/commands/ssh.rs`) → `app.package_info().version`.
- Releases are published at `https://github.com/andrenalin282/MurmurSSH/releases`
  with semver tags (`v1.7.1`, …). Help dialog already links there via `open_url`.
- Settings persist through `settings_service` + `Settings` model
  (`src-tauri/src/models/settings.rs`, mirrored in `src/types.ts`). Pattern: optional
  fields with `#[serde(skip_serializing_if = "Option::is_none")]`; frontend treats
  absent as a documented default.
- Settings UI is `src/components/settings-dialog.ts` with i18n across six locales.
- No update-check code exists today. No Tauri updater plugin.

## Goals

- Detect newer stable GitHub releases without blocking UI.
- Opt-out startup check (default on); manual check always reports a result.
- Open Releases in the system browser; user installs manually.
- Fail silently on startup network errors; show a clear status on manual check.

## Non-goals

- Auto-download, auto-install, or signed updater flow.
- Pre-release / draft detection (GitHub `/releases/latest` already excludes them).
- Per-version snooze / “don’t remind me for this version”.
- Changelog rendering inside the app.
- Platform-specific asset selection (.deb vs AppImage).

## Chosen approach

**Backend GitHub API check** (not Tauri updater plugin, not frontend-only fetch).

Rationale: one Tauri command reusable for startup and Settings; version comparison and
HTTP errors stay in Rust; reuses existing `open_url`; `/releases/latest` matches “stable
only”; no signing/CI changes required for a notify-only UX.

Rejected:

- **tauri-plugin-updater** — overkill for notify + link; needs signatures and install pipeline.
- **Frontend `fetch`** — CSP/permission friction; duplicates version logic outside services.

## Architecture

```
App start (if setting on)  ─┐
Settings “Check now”       ─┼→ check_for_updates()
                            │     → update_service
                            │     → GET api.github.com/.../releases/latest
                            │     → semver compare vs package_info().version
                            ▼
              UpdateCheckResult { update_available, current_version,
                                  latest_version, release_url }
                            │
         ┌──────────────────┴──────────────────┐
         │ startup + update                    │ manual check
         │ → Update dialog                     │ → status line always
         │                                     │ → dialog if update
         └─────────────────────────────────────┘
```

### Components

| Piece | Role |
|-------|------|
| `update_service.rs` | HTTP GET, parse `tag_name` / `html_url`, compare versions |
| `commands` (new or under existing module) | `check_for_updates` Tauri command |
| `Settings.check_updates_on_startup` | `Option<bool>`; **absent = true** |
| `settings-dialog.ts` | Updates section: toggle + Check now + status line |
| `main.ts` | After init, if setting on → background check → dialog only if update |
| i18n (all 6 locales) | Labels, status strings, dialog copy |

## Data model

### Settings field

```rust
/// When true (default), check GitHub Releases once after app start.
/// Absent/None = true (opt-out).
pub check_updates_on_startup: Option<bool>,
```

TypeScript mirror: `check_updates_on_startup?: boolean | null` with the same default.

### Command result

```rust
pub struct UpdateCheckResult {
    pub update_available: bool,
    pub current_version: String,  // e.g. "1.7.2"
    pub latest_version: String,   // tag_name with optional leading 'v' stripped
    pub release_url: String,      // html_url
}
```

Errors return `Result::Err(String)` for the frontend to map into the status line.
Startup ignores `Err` (no dialog).

## Version comparison

1. Current: `app.package_info().version` (already without `v`).
2. Latest: strip one leading `v`/`V` from `tag_name`, then parse `major.minor.patch`.
3. Ignore any local pre-release / build suffix for comparison (take numeric triple only).
4. `update_available` iff `latest > current` (tuple compare).
5. Equal or local newer (dev build ahead of GitHub) → not an update (“up to date”).
6. Missing/unparseable `tag_name` → `Err`, not a crash.

Endpoint:  
`GET https://api.github.com/repos/andrenalin282/MurmurSSH/releases/latest`  
with a sensible `User-Agent` (e.g. `MurmurSSH/<version>`) and a short timeout.
No auth token. Rely on unauthenticated rate limits (fine for single-user desktop use).

HTTP stack: prefer a minimal dependency already acceptable in-tree, or std + a small
client if the project already has one; do **not** pull the full Tauri updater plugin.
If no HTTP client exists yet, add a focused crate (e.g. `ureq` or `reqwest` blocking)
only for this service.

## UI behaviour

### Settings — “Updates” section

- Checkbox: check on startup (bound to `check_updates_on_startup`, default checked).
- Button: Check now.
- Status line under the button:
  - idle / previous result cleared when opening dialog or starting a new check
  - “Checking…” while in flight
  - “You’re up to date (x.y.z)” when no update
  - “Version x.y.z is available” when update found
  - short error text on failure (offline / unreachable)

Saving the checkbox uses the existing Save path with other settings (no separate save
for Check now).

### Update dialog (update found)

- Title: Update available
- Body: MurmurSSH **{latest}** is published. You are on **{current}**.
- Primary: Open releases → `open_url(release_url)` then close
- Secondary: Later → close

Used for both startup discovery and manual check when `update_available`.

### Startup

- After normal init, if `check_updates_on_startup != false`, fire check in background.
- Update → show dialog once.
- Up to date or error → silent (no toast, no dialog).

### Manual Check now

- Always show feedback via status line.
- If update → also open the update dialog.
- Guard against parallel checks (ignore second click while in flight).

## Error handling

| Situation | Startup | Check now |
|-----------|---------|-----------|
| Offline / timeout / non-200 | Silent | Status error |
| Invalid JSON / tag | Silent | Status error |
| Parse / compare failure | Silent | Status error |

Never block window paint or connection flows on the check.

## i18n

Add keys for all six locales for:

- Section / checkbox / button labels
- Status: checking, up to date, update available, generic error
- Dialog title, body (with version placeholders), Open releases, Later

## Files (expected touch list)

**New**

- `src-tauri/src/services/update_service.rs`
- Command registration (new file under `commands/` or adjacent to version helper)

**Extend**

- `src-tauri/src/models/settings.rs`
- `src-tauri/src/services/settings_service.rs` (only if normalization needed)
- `src-tauri/src/lib.rs` (register command)
- `src/types.ts`, `src/api/index.ts`
- `src/components/settings-dialog.ts`
- `src/main.ts` (startup check + dialog helper)
- `src/i18n/*.ts` (all locales)
- `CHANGELOG.md`, version bump to **1.7.2** (Cargo.toml / package.json / related)

## Testing

- Unit tests in `update_service` for tag stripping and semver compare (newer / equal / older / bad tag).
- Manual: toggle off → no startup network expectation; Check now with network on/off;
  dialog opens Releases URL.

## Release

After implementation and verification: bump to **1.7.2**, commit, tag `v1.7.2`, push,
create GitHub Release (same process as v1.7.1).
