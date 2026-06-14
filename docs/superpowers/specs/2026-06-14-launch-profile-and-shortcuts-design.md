# Design — Launch-a-profile, parallel windows, desktop shortcuts, and group accordion

Date: 2026-06-14
Status: Approved (pending written-spec review)

## Summary

Two related changes to MurmurSSH, both touching the profile-selector area:

1. **Launch / parallel connections.** Let the app be started pre-pointed at a profile
   (`murmurssh --profile <id>`) so it connects immediately. From a running, *connected*
   app the user can open a *second, independent* window for a different profile (a new OS
   process), and can generate a desktop launcher (`.desktop`) for one-click connect from
   the OS app menu.
2. **Group accordion.** Profile groups become an accordion: at most one group is expanded
   at a time, and the last-opened group is remembered across restarts (instead of every
   group being expanded by default, which wastes vertical space).

These are independent features shipped together; either could be reverted without the other.

## Background / current state

- Tauri 2 app, single window labelled `main`. **No single-instance plugin is installed**, so
  re-launching the binary opens a genuinely independent process + window. This is the desired
  behaviour for parallel connections — we are not adding single-instance.
- Connection state lives in `src/main.ts`: `connectedProfileId` / `connectingProfileId`, plus
  `verifyConnection()` and the `profileSelector.onConnect(...)` handler. `ProfileSelector`
  exposes `setConnected(isConnected)`.
- Profiles have an immutable slug `id` (lowercase, non-alphanumeric → `-`). Backfilled
  `created_at`. `get_profile` is the single source of truth.
- Profile groups: `profile-selector.ts` renders a collapsible grouped tree. Collapse state is
  `private collapsedGroups = new Set<string>()` — per-session, default **all expanded**.
- A reusable context-menu CSS class `.lb-context-menu` already exists (used by
  `local-file-browser.ts`). Profile rows carry `data-id` and have `click` / `dblclick` handlers.
- Settings persistence pattern: read-merge-write through `api.getSettings()` / `api.saveSettings()`
  (e.g. `profile_sort`, `local_browser_position`).

## Feature 1 — Launch a profile / parallel windows / desktop shortcut

### Mechanism (chosen approach)

Plain CLI argument `--profile <id-or-name>`. Rationale: minimal, no new plugins, directly usable
in a `.desktop` `Exec=` line and from a terminal, and it works *with* the no-single-instance
design (every launch = independent window). Rejected: custom URI scheme + deep-link/single-instance
plugins (fights the separate-window goal, more moving parts); env var (awkward for `.desktop`/CLI).

### Argument matching

Resolution order: **exact `id` match first, then case-insensitive `name` match.** Unknown → treated
as "no launch profile". The in-app button and generated shortcuts always use the stable `id`;
ID-or-name matching exists so a hand-typed `--profile MyServer` also works.

### Backend

New `src-tauri/src/services/launch_service.rs` and `src-tauri/src/commands/launch.rs`
(one concern per file, per architecture rules). Three commands, registered in `lib.rs`:

- `get_launch_profile() -> Option<String>`
  Returns the profile `id` parsed from startup args, *resolved against existing profiles*
  (id-or-name → canonical id). Returns `None` if absent or unresolved. The raw arg is captured
  once at startup into a `OnceLock<Option<String>>`; resolution against the profile list happens
  in the command so it reflects current profiles.

- `open_profile_in_new_window(profile_id: String) -> Result<(), String>`
  Validates the profile exists, then spawns a detached child:
  `Command::new(std::env::current_exe()?).arg("--profile").arg(id).spawn()`.
  Child outlives the parent (no `wait`). stdin/stdout/stderr left default. Reject `profile_id`
  containing null bytes.

- `create_desktop_shortcut(profile_id: String) -> Result<String, String>`
  Validates the profile exists. Writes `~/.config`-adjacent XDG path
  `~/.local/share/applications/murmurssh-<id>.desktop` (create dir if missing, `0644`) with:
  ```
  [Desktop Entry]
  Type=Application
  Name=MurmurSSH — <profile name>
  Comment=Connect to <username>@<host>
  Exec=<current_exe> --profile <id>
  Icon=murmurssh
  Terminal=false
  Categories=Network;RemoteAccess;
  ```
  `Exec` is the absolute `current_exe()` path with the id quoted/escaped. Returns the written
  path (frontend surfaces it in a toast/confirm). Reject ids with null bytes or path separators
  before building the filename.

### Startup arg parsing

`main.rs` / `lib.rs`: read `std::env::args()`, find `--profile <value>` (and `--profile=<value>`),
store the raw value in the `OnceLock`. No other CLI surface is added. Documented in `--help`? Out
of scope — no arg framework added; unknown args are ignored as today.

### Frontend — API wrappers (`src/api/index.ts`)

`getLaunchProfile()`, `openProfileInNewWindow(id)`, `createDesktopShortcut(id)`.

### Frontend — auto-connect on launch (`src/main.ts`)

After the initial profile load + component wiring (end of init), call
`const launchId = await api.getLaunchProfile()`. If set and it matches a known profile:
select it in the selector and run the **same path** `onConnect` uses (reusing existing
`verifyConnection` flow — credential prompts happen normally, since a fresh process shares no
session credentials). If the id does not resolve, show an inline error and continue to the normal
UI. Guard so auto-connect runs once and respects the existing `connectingProfileId` re-entrancy
guard.

### Frontend — "Open in new window" button (`profile-selector.ts`)

Visible **only when connected AND the selected profile differs from the connected one**. It occupies
the place where the Connect button is otherwise disabled (Connect stays the primary action only for
the not-connected case). Implemented inside the existing `setConnected()` / button-state logic so it
appears/disappears as selection and connection state change. Click → `api.openProfileInNewWindow(selectedId)`
(errors → inline error).

### Frontend — context menu on profile rows (`profile-selector.ts`)

Right-click a profile row → `.lb-context-menu` (reuse existing CSS) with:
- **Open in new window** → `api.openProfileInNewWindow(rowId)`
- **Create desktop shortcut** → `api.createDesktopShortcut(rowId)`, then a confirm/toast showing the
  written path.
Menu closes on outside-click / Escape / action (mirror `local-file-browser` behaviour). Right-click
also selects the row first (so the action targets the clicked profile).

## Feature 2 — Group accordion + persisted last-open group

### Data model (`profile-selector.ts`)

Replace `private collapsedGroups = new Set<string>()` with
`private expandedGroup: string | null = null` (at most one open group). Render: a group is
expanded iff `key === this.expandedGroup`; all others render collapsed (caret ▸, rows hidden).

### Interaction

Header click handler: if `key === this.expandedGroup` → set `null` (collapse it); else set
`this.expandedGroup = key` (this single assignment collapses every other group). After updating,
persist and re-render.

### Persistence (`settings.json`)

New optional field `expanded_profile_group: Option<String>` on `Settings`
(`models/settings.rs`, serde `skip_serializing_if = "Option::is_none"`, backward compatible).
On header toggle, read-merge-write via `api.getSettings()` / `api.saveSettings()` (same pattern as
`profile_sort`). On selector init, restore `expandedGroup` from settings.

### Edge cases

- Stored group no longer exists (renamed/deleted) → resolves to nothing expanded (safe; minimal space).
- Fresh install / no stored value → nothing expanded (matches the "saves space" goal).
- Ungrouped bucket uses the empty-string key `""`, which is a valid `expandedGroup` value and round-trips
  through settings as an empty string (kept distinct from `None` = nothing expanded).

## i18n

New keys in all 6 locales (`src/i18n/`): `profiles.openInNewWindow`, `profiles.createShortcut`,
`profiles.shortcutCreated` (with `{path}` interpolation), and a launch-not-found error message.
No new keys needed for the accordion (no new visible strings).

## Security / safety

- Reject `profile_id` with null bytes; for the shortcut filename also reject path separators
  (defence in depth — ids are already slugified, but the command is a public IPC surface).
- `open_profile_in_new_window` only ever execs `current_exe()` with a fixed `--profile` flag and a
  validated id; no shell, no user-supplied executable.
- `.desktop` `Exec` uses the absolute `current_exe()` path and the validated id.

## Build / validation gates

`cargo build`, `cargo clippy`, `npx tsc`, `vite build` all green; existing lib tests pass. Per CLAUDE.md,
edit `.ts` and regenerate `.js` siblings via `npx tsc`, committing both. Manual checks:
(1) `murmurssh --profile <id>` connects; unknown id → graceful; (2) connected + other profile selected
shows the button and opens a second independent window; (3) right-click → create shortcut writes a working
`.desktop`; (4) accordion: opening one group collapses others, last-open group restored after restart.

## Known limitations (honest)

- `.desktop` `Icon=murmurssh` is best-effort: resolves for `.deb` installs (icon registered in the
  hicolor theme); for AppImage / `tauri dev` the icon may not appear. Exact installed icon name to be
  confirmed during implementation; fallback is an absolute icon path if one is reliably available.
- No de-duplication of windows: launching the same profile twice yields two independent windows
  (acceptable / arguably desirable).

## Out of scope (YAGNI)

- Custom URI scheme / clickable `murmurssh://` links.
- Single-instance coordination or window manager / session restore.
- Removing/Managing previously-created `.desktop` files from within the app.
- Cross-window credential sharing.
