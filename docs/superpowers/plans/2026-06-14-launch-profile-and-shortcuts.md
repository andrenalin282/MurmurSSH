# Launch-a-profile, Parallel Windows, Desktop Shortcuts & Group Accordion — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let MurmurSSH be launched pre-pointed at a profile (`murmurssh --profile <id>`) that auto-connects, open a selected profile in a second independent window while connected, generate a `.desktop` launcher per profile, and turn profile groups into a single-open accordion whose last-open group is remembered.

**Architecture:** A plain CLI arg captured at startup into a `OnceLock`, resolved against existing profiles by id-or-name, exposed via three new Tauri commands in a `launch_service` / `commands::launch` pair. The frontend reads the launch profile on init and reuses the existing connect path. The profile-selector gains a context menu + an "Open in new window" primary button, and its group collapse model changes from a many-collapsed `Set` to a single `expandedGroup` persisted in `settings.json`.

**Tech Stack:** Rust (Tauri 2, `std::process::Command`, `std::fs`), vanilla TypeScript frontend (tsc-emitted `.js` siblings), `settings.json` persistence.

**Spec:** `docs/superpowers/specs/2026-06-14-launch-profile-and-shortcuts-design.md`

---

## File Structure

**Backend (create):**
- `src-tauri/src/services/launch_service.rs` — arg parse/capture/resolve, spawn new window, build + write `.desktop` file. Pure helpers (`parse_profile_arg`, `resolve_profile_id`, `desktop_entry`) are unit-tested.
- `src-tauri/src/commands/launch.rs` — three thin `#[tauri::command]` wrappers.

**Backend (modify):**
- `src-tauri/src/services/mod.rs` — add `pub mod launch_service;`
- `src-tauri/src/commands/mod.rs` — add `pub mod launch;`
- `src-tauri/src/lib.rs` — capture arg at startup; register 3 commands.
- `src-tauri/src/models/settings.rs` — add `expanded_profile_group: Option<String>`.

**Frontend (modify):**
- `src/types.ts` — add `expanded_profile_group` to `Settings`.
- `src/api/index.ts` — 3 new wrappers.
- `src/components/profile-selector.ts` — accordion model, open-in-new-window button, context menu, 2 new callbacks, `selectProfile()`, `setConnected(isConnected, connectedId)`.
- `src/main.ts` — refactor `onConnect` body into a named `connectToProfile()`, wire new callbacks, auto-connect on launch.
- `src/i18n/{en,de,fr,nl,pl,ru}.ts` (+ regenerated `.js`) — 3 new `profiles.*` keys.

---

## Task 1: Add `expanded_profile_group` to Settings (Rust + TS type)

**Files:**
- Modify: `src-tauri/src/models/settings.rs`
- Modify: `src/types.ts:50` (Settings interface)

- [ ] **Step 1: Add the Rust field**

In `src-tauri/src/models/settings.rs`, add this field to the `Settings` struct, after `profile_sort`:

```rust
    /// Which profile group is currently expanded in the selector accordion.
    /// At most one group is open at a time. `Some("")` is the ungrouped bucket;
    /// `None` means nothing is expanded. Restored on next launch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded_profile_group: Option<String>,
```

- [ ] **Step 2: Add the TS field**

In `src/types.ts`, inside `export interface Settings { ... }` (after the `profile_sort` line), add:

```ts
  /** Which profile group is expanded in the selector accordion (""=ungrouped, absent=none). */
  expanded_profile_group?: string | null;
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo build 2>&1 | tail -5`
Expected: builds (warnings about unused field are fine until used).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/models/settings.rs src/types.ts
git commit -m "feat(settings): add expanded_profile_group field"
```

---

## Task 2: `launch_service` pure helpers (TDD)

**Files:**
- Create: `src-tauri/src/services/launch_service.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Register the module**

In `src-tauri/src/services/mod.rs`, add (keep alphabetical-ish, near other services):

```rust
pub mod launch_service;
```

- [ ] **Step 2: Write the file with pure helpers + failing tests**

Create `src-tauri/src/services/launch_service.rs`:

```rust
use crate::models::profile::Profile;
use crate::services::profile_service;
use std::sync::OnceLock;

/// Raw `--profile <value>` captured once from process args at startup.
static LAUNCH_ARG: OnceLock<Option<String>> = OnceLock::new();

/// Capture the launch arg from the current process args. Call once at startup.
pub fn capture_launch_arg() {
    let _ = LAUNCH_ARG.set(parse_profile_arg(std::env::args().skip(1).collect()));
}

/// Pure: extract the value of `--profile <v>` or `--profile=<v>` from args.
/// Returns the first non-empty value found, else None.
pub fn parse_profile_arg(args: Vec<String>) -> Option<String> {
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if let Some(v) = a.strip_prefix("--profile=") {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        } else if a == "--profile" {
            if let Some(v) = it.next() {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// Pure: resolve a raw id-or-name token to a canonical profile id.
/// Exact `id` match wins; otherwise case-insensitive `name` match; else None.
pub fn resolve_profile_id(raw: &str, profiles: &[Profile]) -> Option<String> {
    if let Some(p) = profiles.iter().find(|p| p.id == raw) {
        return Some(p.id.clone());
    }
    let lower = raw.to_lowercase();
    profiles
        .iter()
        .find(|p| p.name.to_lowercase() == lower)
        .map(|p| p.id.clone())
}

/// Pure: build the `.desktop` file content for a profile.
/// The exec path is double-quoted so paths with spaces (e.g. AppImage) stay valid.
pub fn desktop_entry(exec_path: &str, profile: &Profile) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=MurmurSSH — {name}\n\
         Comment=Connect to {user}@{host}\n\
         Exec=\"{exec}\" --profile {id}\n\
         Icon=murmurssh\n\
         Terminal=false\n\
         Categories=Network;RemoteAccess;\n",
        name = profile.name,
        user = profile.username,
        host = profile.host,
        exec = exec_path,
        id = profile.id,
    )
}

/// Resolve the captured launch arg against current profiles → canonical id.
pub fn launch_profile_id() -> Option<String> {
    let raw = LAUNCH_ARG.get().and_then(|o| o.clone())?;
    let profiles = profile_service::list_profiles().ok()?;
    resolve_profile_id(&raw, &profiles)
}

/// Spawn a new, detached MurmurSSH instance pointed at `profile_id`.
pub fn open_in_new_window(profile_id: &str) -> Result<(), String> {
    if profile_id.contains('\0') {
        return Err("Invalid profile id".to_string());
    }
    // Validate the profile exists before launching.
    profile_service::get_profile(profile_id)?;
    let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    std::process::Command::new(exe)
        .arg("--profile")
        .arg(profile_id)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch new window: {e}"))
}

/// Write a `.desktop` launcher for `profile_id` into the user's applications dir.
/// Returns the written path.
pub fn create_desktop_shortcut(profile_id: &str) -> Result<String, String> {
    if profile_id.contains('\0') || profile_id.contains('/') {
        return Err("Invalid profile id".to_string());
    }
    let profile = profile_service::get_profile(profile_id)?;
    let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let dir = std::path::Path::new(&home).join(".local/share/applications");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {e}"))?;
    let path = dir.join(format!("murmurssh-{profile_id}.desktop"));
    let content = desktop_entry(&exe.to_string_lossy(), &profile);
    std::fs::write(&path, content).map_err(|e| format!("Failed to write shortcut: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::profile::{AuthType, Profile, UploadMode};

    fn mk(id: &str, name: &str) -> Profile {
        Profile {
            id: id.to_string(),
            name: name.to_string(),
            host: "example.com".to_string(),
            port: 22,
            username: "root".to_string(),
            auth_type: AuthType::Agent,
            key_path: None,
            default_remote_path: None,
            editor_command: None,
            upload_mode: UploadMode::default(),
            protocol: None,
            local_path: None,
            credential_storage_mode: None,
            stored_secret_portable: None,
            local_paths_by_user: None,
            group: None,
            created_at: None,
        }
    }

    #[test]
    fn parse_space_form() {
        let a = vec!["--profile".to_string(), "web1".to_string()];
        assert_eq!(parse_profile_arg(a), Some("web1".to_string()));
    }

    #[test]
    fn parse_equals_form() {
        let a = vec!["--profile=web1".to_string()];
        assert_eq!(parse_profile_arg(a), Some("web1".to_string()));
    }

    #[test]
    fn parse_absent() {
        let a = vec!["--other".to_string(), "x".to_string()];
        assert_eq!(parse_profile_arg(a), None);
    }

    #[test]
    fn parse_empty_value_ignored() {
        let a = vec!["--profile".to_string(), "".to_string()];
        assert_eq!(parse_profile_arg(a), None);
    }

    #[test]
    fn resolve_by_id_exact() {
        let ps = vec![mk("web-1", "Web One"), mk("db-1", "DB One")];
        assert_eq!(resolve_profile_id("db-1", &ps), Some("db-1".to_string()));
    }

    #[test]
    fn resolve_by_name_case_insensitive() {
        let ps = vec![mk("web-1", "Web One")];
        assert_eq!(resolve_profile_id("web one", &ps), Some("web-1".to_string()));
    }

    #[test]
    fn resolve_unknown_is_none() {
        let ps = vec![mk("web-1", "Web One")];
        assert_eq!(resolve_profile_id("nope", &ps), None);
    }

    #[test]
    fn desktop_entry_quotes_exec_and_uses_id() {
        let p = mk("web-1", "Web One");
        let s = desktop_entry("/opt/My App/murmurssh", &p);
        assert!(s.contains("Exec=\"/opt/My App/murmurssh\" --profile web-1"));
        assert!(s.contains("Name=MurmurSSH — Web One"));
        assert!(s.contains("Comment=Connect to root@example.com"));
    }
}
```

> NOTE: The exact field set in `mk()` must match the real `Profile` struct. If `cargo test` reports missing/extra fields, open `src-tauri/src/models/profile.rs` and adjust the literal to match (the struct around lines 16–60). Do NOT add `#[serde(default)]` hacks — just match the fields.

- [ ] **Step 3: Run the tests — expect compile/pass**

Run: `cd src-tauri && cargo test launch_service 2>&1 | tail -20`
Expected: all 8 `launch_service::tests::*` pass. If the `Profile` literal mismatches, fix it per the NOTE and re-run.

- [ ] **Step 4: Clippy clean**

Run: `cd src-tauri && cargo clippy 2>&1 | tail -15`
Expected: no new warnings in `launch_service.rs`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/launch_service.rs src-tauri/src/services/mod.rs
git commit -m "feat(launch): launch_service — arg parse, resolve, spawn, desktop entry"
```

---

## Task 3: `commands::launch` + register + capture arg at startup

**Files:**
- Create: `src-tauri/src/commands/launch.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create the command wrappers**

Create `src-tauri/src/commands/launch.rs`:

```rust
use crate::services::launch_service;

/// Canonical id of the profile this instance was launched to connect to, if any.
#[tauri::command]
pub fn get_launch_profile() -> Option<String> {
    launch_service::launch_profile_id()
}

/// Spawn a second, independent MurmurSSH window connecting to `profile_id`.
#[tauri::command]
pub fn open_profile_in_new_window(profile_id: String) -> Result<(), String> {
    launch_service::open_in_new_window(&profile_id)
}

/// Write a `.desktop` launcher for `profile_id`; returns the written path.
#[tauri::command]
pub fn create_desktop_shortcut(profile_id: String) -> Result<String, String> {
    launch_service::create_desktop_shortcut(&profile_id)
}
```

- [ ] **Step 2: Register the command module**

In `src-tauri/src/commands/mod.rs`, add:

```rust
pub mod launch;
```

- [ ] **Step 3: Capture the arg + register commands in lib.rs**

In `src-tauri/src/lib.rs`, inside `pub fn run()`, make the FIRST line of the body capture the arg:

```rust
pub fn run() {
    services::launch_service::capture_launch_arg();
    tauri::Builder::default()
```

Then in the `tauri::generate_handler![ ... ]` list, add these three (e.g. right after the `commands::ssh::*` block):

```rust
            commands::launch::get_launch_profile,
            commands::launch::open_profile_in_new_window,
            commands::launch::create_desktop_shortcut,
```

- [ ] **Step 4: Build**

Run: `cd src-tauri && cargo build 2>&1 | tail -8`
Expected: builds clean (no unused-function warnings now that everything is wired).

- [ ] **Step 5: Manual smoke test of the arg parse**

Run: `cd src-tauri && cargo test launch_service 2>&1 | tail -5`
Expected: tests still pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/launch.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(launch): register launch commands + capture --profile at startup"
```

---

## Task 4: Frontend API wrappers

**Files:**
- Modify: `src/api/index.ts`

- [ ] **Step 1: Add the three wrappers**

Append to `src/api/index.ts` (end of file is fine; follows existing `invoke` style with camelCase keys auto-mapped to snake_case Rust args):

```ts
/** Canonical id of the profile this instance was launched to connect to, if any. */
export async function getLaunchProfile(): Promise<string | null> {
  return invoke<string | null>("get_launch_profile");
}

/** Open a second, independent window connecting to the given profile. */
export async function openProfileInNewWindow(profileId: string): Promise<void> {
  return invoke("open_profile_in_new_window", { profileId });
}

/** Write a .desktop launcher for the profile; resolves to the written path. */
export async function createDesktopShortcut(profileId: string): Promise<string> {
  return invoke<string>("create_desktop_shortcut", { profileId });
}
```

- [ ] **Step 2: Typecheck**

Run: `npx tsc --noEmit 2>&1 | tail -10`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/api/index.ts
git commit -m "feat(api): wrappers for launch profile + new window + desktop shortcut"
```

---

## Task 5: Group accordion in profile-selector

**Files:**
- Modify: `src/components/profile-selector.ts`

- [ ] **Step 1: Replace the collapse model field**

In `src/components/profile-selector.ts`, replace this line (≈ line 22):

```ts
  private collapsedGroups = new Set<string>();
```

with:

```ts
  // Accordion: at most one group open at a time. "" = ungrouped bucket, null = none.
  private expandedGroup: string | null = null;
```

- [ ] **Step 2: Restore the expanded group in init()**

In `init()`, after the `this.sortMode = ...` line, add:

```ts
    this.expandedGroup = settings.expanded_profile_group ?? null;
```

(Note: `?? null` preserves an empty-string value — the ungrouped bucket — while mapping absent/null to "nothing expanded".)

- [ ] **Step 3: Update render() collapse logic**

In `render()`, replace:

```ts
          const collapsed = this.collapsedGroups.has(key);
```

with:

```ts
          const collapsed = key !== this.expandedGroup;
```

- [ ] **Step 4: Update the header click handler**

In `render()`, replace the header click listener body:

```ts
    this.container.querySelectorAll<HTMLElement>(".profile-group__header").forEach((h) => {
      h.addEventListener("click", () => {
        const key = h.dataset.group ?? "";
        if (this.collapsedGroups.has(key)) this.collapsedGroups.delete(key);
        else this.collapsedGroups.add(key);
        this.render();
      });
    });
```

with:

```ts
    this.container.querySelectorAll<HTMLElement>(".profile-group__header").forEach((h) => {
      h.addEventListener("click", () => {
        const key = h.dataset.group ?? "";
        // Accordion: clicking the open group closes it; any other becomes the sole open one.
        this.expandedGroup = this.expandedGroup === key ? null : key;
        void this.persistExpandedGroup();
        this.render();
      });
    });
```

- [ ] **Step 5: Add the persistence helper**

Add this method right after `persistSortMode()`:

```ts
  private async persistExpandedGroup(): Promise<void> {
    try {
      const settings = await api.getSettings();
      await api.saveSettings({ ...settings, expanded_profile_group: this.expandedGroup });
    } catch (e) {
      console.warn("[ProfileSelector] Failed to persist expanded group:", e);
    }
  }
```

- [ ] **Step 6: Typecheck**

Run: `npx tsc --noEmit 2>&1 | tail -10`
Expected: no errors (no remaining references to `collapsedGroups`).

- [ ] **Step 7: Commit**

```bash
git add src/components/profile-selector.ts
git commit -m "feat(profiles): single-open group accordion persisted as expanded_profile_group"
```

---

## Task 6: "Open in new window" button + context menu in profile-selector

**Files:**
- Modify: `src/components/profile-selector.ts`

- [ ] **Step 1: Add fields for connected id, callbacks, and context-menu element**

Add these fields to the class (near the other `private on*Callback` fields):

```ts
  private connectedId: string | null = null;
  private onOpenInNewWindowCallback: ((profileId: string) => void) | null = null;
  private onCreateShortcutCallback: ((profileId: string) => void) | null = null;
  private contextMenu: HTMLElement | null = null;
```

- [ ] **Step 2: Add the two callback setters**

Add after the existing `onDelete(...)` method:

```ts
  onOpenInNewWindow(callback: (profileId: string) => void): void {
    this.onOpenInNewWindowCallback = callback;
  }

  onCreateShortcut(callback: (profileId: string) => void): void {
    this.onCreateShortcutCallback = callback;
  }
```

- [ ] **Step 3: Track the connected id in setConnected()**

Change the `setConnected` signature and first lines. Replace:

```ts
  setConnected(isConnected: boolean): void {
    this.isConnected = isConnected;
    if (!isConnected) this.isConnecting = false;
```

with:

```ts
  setConnected(isConnected: boolean, connectedId: string | null = null): void {
    this.isConnected = isConnected;
    this.connectedId = isConnected ? connectedId : null;
    if (!isConnected) this.isConnecting = false;
```

Then, at the END of `setConnected()` (after the existing edit/delete button lines), add:

```ts
    this.updatePrimaryButtons();
```

- [ ] **Step 4: Add the primary-button toggle helper**

Add this method (e.g. right after `updateButtonStates()`):

```ts
  /**
   * Show "Open in new window" instead of "Connect" only when a session is active
   * AND the selected profile differs from the connected one. Otherwise show Connect.
   */
  private updatePrimaryButtons(): void {
    const connectBtn = document.getElementById("connect-btn") as HTMLButtonElement | null;
    const openBtn = document.getElementById("open-window-btn") as HTMLButtonElement | null;
    const showOpen =
      this.isConnected && this.selectedId !== null && this.selectedId !== this.connectedId;
    if (connectBtn) connectBtn.style.display = showOpen ? "none" : "";
    if (openBtn) openBtn.style.display = showOpen ? "" : "none";
  }
```

- [ ] **Step 5: Call the toggle from the other state updaters**

At the END of `updateButtonStates()`, add:

```ts
    this.updatePrimaryButtons();
```

At the END of `setConnecting()`, add:

```ts
    this.updatePrimaryButtons();
```

- [ ] **Step 6: Render the open-window button**

In `render()`, replace the connect button line:

```ts
        <button id="connect-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.connect")}</button>
```

with:

```ts
        <button id="connect-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.connect")}</button>
        <button id="open-window-btn" style="display:none">${t("profiles.openInNewWindow")}</button>
```

- [ ] **Step 7: Wire the open-window button + call toggle after render**

In `render()`, just before the closing of the method (after the `connect-btn` click listener), add:

```ts
    document.getElementById("open-window-btn")?.addEventListener("click", () => {
      if (this.selectedId) this.onOpenInNewWindowCallback?.(this.selectedId);
    });
    this.updatePrimaryButtons();
```

- [ ] **Step 8: Add the context-menu methods**

Add these two methods to the class (e.g. after `updateRowSelection()`):

```ts
  private hideContextMenu(): void {
    this.contextMenu?.remove();
    this.contextMenu = null;
  }

  private showRowContextMenu(x: number, y: number, profileId: string): void {
    this.hideContextMenu();
    const menu = document.createElement("div");
    menu.className = "lb-context-menu";
    menu.innerHTML = `
      <button data-action="new-window">${t("profiles.openInNewWindow")}</button>
      <button data-action="shortcut">${t("profiles.createShortcut")}</button>
    `;
    document.body.appendChild(menu);
    const rect = menu.getBoundingClientRect();
    menu.style.left = `${Math.min(x, window.innerWidth - rect.width - 4)}px`;
    menu.style.top = `${Math.min(y, window.innerHeight - rect.height - 4)}px`;
    this.contextMenu = menu;

    menu.addEventListener("click", (e) => {
      const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-action]");
      if (!btn) return;
      const action = btn.dataset.action;
      this.hideContextMenu();
      if (action === "new-window") this.onOpenInNewWindowCallback?.(profileId);
      else if (action === "shortcut") this.onCreateShortcutCallback?.(profileId);
    });

    setTimeout(() => {
      document.addEventListener("mousedown", () => this.hideContextMenu(), { once: true });
    }, 0);
  }
```

- [ ] **Step 9: Attach contextmenu to profile rows**

In `render()`, inside the `.profile-row` `forEach((row) => { ... })`, after the existing `dblclick` listener, add:

```ts
      row.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        this.selectedId = row.dataset.id ?? null;
        this.updateRowSelection();
        this.updateButtonStates();
        if (this.selectedId) this.showRowContextMenu(e.clientX, e.clientY, this.selectedId);
      });
```

- [ ] **Step 10: Add the public selectProfile() method (used by auto-connect)**

Add this public method to the class:

```ts
  /** Programmatically select a profile by id and re-render (used by launch auto-connect). */
  selectProfile(id: string): void {
    this.selectedId = id;
    this.render();
  }
```

- [ ] **Step 11: Typecheck**

Run: `npx tsc --noEmit 2>&1 | tail -10`
Expected: no errors. (The new `profiles.openInNewWindow` / `profiles.createShortcut` keys are added in Task 8; `t()` returns a string regardless, so typecheck passes now.)

- [ ] **Step 12: Commit**

```bash
git add src/components/profile-selector.ts
git commit -m "feat(profiles): open-in-new-window button + row context menu (new window / shortcut)"
```

---

## Task 7: main.ts wiring — refactor connect, wire callbacks, auto-connect on launch

**Files:**
- Modify: `src/main.ts`

- [ ] **Step 1: Refactor onConnect into a named function**

In `src/main.ts`, find:

```ts
profileSelector.onConnect(async (profileId: string) => {
```

Replace that line with a named async function declaration:

```ts
async function connectToProfile(profileId: string) {
```

Then find the matching closing of that callback. It currently ends with `});` after the `localBrowser.onDownload(...)` block near the end of the file (the very large handler). Replace that final `});` (the one that closes `profileSelector.onConnect(async ... )`) with:

```ts
}
profileSelector.onConnect(connectToProfile);
```

> Verify: the function body between these two edits is unchanged. After this, `connectToProfile` is a hoisted declaration callable from the init block at the bottom.

- [ ] **Step 2: Pass the connected id to setConnected**

Inside `connectToProfile`, find:

```ts
  profileSelector.setConnected(true);
```

Replace with:

```ts
  profileSelector.setConnected(true, profileId);
```

- [ ] **Step 3: Wire the two new selector callbacks**

Immediately AFTER the `profileSelector.onConnect(connectToProfile);` line you added in Step 1, add:

```ts
profileSelector.onOpenInNewWindow(async (profileId: string) => {
  try {
    await api.openProfileInNewWindow(profileId);
  } catch (e) {
    statusBar.set("error", String(e));
  }
});

profileSelector.onCreateShortcut(async (profileId: string) => {
  try {
    const path = await api.createDesktopShortcut(profileId);
    await showConfirm(t("profiles.shortcutCreated", { path }), t("profiles.createShortcut"));
  } catch (e) {
    statusBar.set("error", String(e));
  }
});
```

> `showConfirm` and `statusBar` and `t` are already imported/available in main.ts (used elsewhere). The confirm dialog is used purely to display the written path (the boolean result is ignored).

- [ ] **Step 4: Auto-connect on launch**

In the `profileSelector.init().then(async (lastUsedId) => { ... })` block at the bottom, add this as the LAST statement inside the `.then(...)` callback (after the existing `if (lastUsedId) { ... }` block):

```ts
  // If launched with `--profile <id>`, auto-connect to it (resolved server-side).
  try {
    const launchId = await api.getLaunchProfile();
    if (launchId) {
      profileSelector.selectProfile(launchId);
      if (profileSelector.getSelectedProfile()) {
        await connectToProfile(launchId);
      }
    }
  } catch {
    // Non-fatal — fall back to the normal idle UI.
  }
```

- [ ] **Step 5: Typecheck**

Run: `npx tsc --noEmit 2>&1 | tail -15`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add src/main.ts
git commit -m "feat(app): auto-connect on --profile launch + wire new-window/shortcut callbacks"
```

---

## Task 8: i18n keys in all 6 locales

**Files:**
- Modify: `src/i18n/en.ts`, `de.ts`, `fr.ts`, `nl.ts`, `pl.ts`, `ru.ts`

- [ ] **Step 1: Add three keys to each locale's `profiles:` block**

In each file, inside the `profiles: { ... }` object (after `sortCreatedShort`), add the three keys with the per-language values below.

`src/i18n/en.ts`:
```ts
    openInNewWindow: "Open in new window",
    createShortcut: "Create desktop shortcut",
    shortcutCreated: "Desktop shortcut created:\n{path}",
```

`src/i18n/de.ts`:
```ts
    openInNewWindow: "In neuem Fenster öffnen",
    createShortcut: "Desktop-Verknüpfung erstellen",
    shortcutCreated: "Desktop-Verknüpfung erstellt:\n{path}",
```

`src/i18n/fr.ts`:
```ts
    openInNewWindow: "Ouvrir dans une nouvelle fenêtre",
    createShortcut: "Créer un raccourci bureau",
    shortcutCreated: "Raccourci bureau créé :\n{path}",
```

`src/i18n/nl.ts`:
```ts
    openInNewWindow: "In nieuw venster openen",
    createShortcut: "Snelkoppeling maken",
    shortcutCreated: "Snelkoppeling aangemaakt:\n{path}",
```

`src/i18n/pl.ts`:
```ts
    openInNewWindow: "Otwórz w nowym oknie",
    createShortcut: "Utwórz skrót na pulpicie",
    shortcutCreated: "Utworzono skrót:\n{path}",
```

`src/i18n/ru.ts`:
```ts
    openInNewWindow: "Открыть в новом окне",
    createShortcut: "Создать ярлык на рабочем столе",
    shortcutCreated: "Ярлык создан:\n{path}",
```

- [ ] **Step 2: Typecheck (all locales must share the same key shape)**

Run: `npx tsc --noEmit 2>&1 | tail -15`
Expected: no errors. If the i18n types enforce identical keys across locales, a missing key in any file surfaces here — fix the offending file.

- [ ] **Step 3: Commit**

```bash
git add src/i18n/en.ts src/i18n/de.ts src/i18n/fr.ts src/i18n/nl.ts src/i18n/pl.ts src/i18n/ru.ts
git commit -m "i18n: add openInNewWindow / createShortcut / shortcutCreated in all 6 locales"
```

---

## Task 9: Regenerate `.js` siblings, full build, validation

**Files:**
- Modify (generated): `src/**/*.js` siblings of every edited `.ts`

- [ ] **Step 1: Regenerate the tsc-emitted .js siblings**

Per CLAUDE.md, the frontend has tracked `.js` siblings (Vite loads `.ts`, but the `.js` are committed). Regenerate them:

Run: `npx tsc 2>&1 | tail -10`
Expected: emits `.js` next to the edited `.ts` files; no type errors.

- [ ] **Step 2: Frontend production build**

Run: `npm run build 2>&1 | tail -15`
Expected: Vite build succeeds.

- [ ] **Step 3: Backend build + clippy + tests**

Run: `cd src-tauri && cargo build 2>&1 | tail -5 && cargo clippy 2>&1 | tail -8 && cargo test 2>&1 | tail -15`
Expected: build clean, clippy clean, all lib tests pass (the 8 new `launch_service` tests + the existing 12).

- [ ] **Step 4: Commit the regenerated artifacts**

```bash
git add -A src/
git commit -m "build: regenerate .js siblings for launch feature + accordion"
```

---

## Task 10: Manual verification (dev run)

**Files:** none (runtime verification).

- [ ] **Step 1: Accordion behavior**

Run: `npm run tauri dev`
Verify, with at least two groups present:
- On launch, only the previously-opened group is expanded (or none on first run).
- Clicking a collapsed group's header expands it AND collapses the previously open one.
- Clicking the open group's header collapses it (none open).
- Quit and relaunch → the last-open group is expanded again.

- [ ] **Step 2: Open-in-new-window button**

While connected to profile A, click a DIFFERENT profile B in the tree.
Verify: the primary button switches from "Connect" to "Open in new window". Clicking it opens a SECOND independent app window that connects to B (prompting for credentials as normal). Selecting profile A (the connected one) again hides the button.

- [ ] **Step 3: Context menu**

Right-click any profile row.
Verify: a menu with "Open in new window" and "Create desktop shortcut" appears; it closes on outside click. "Create desktop shortcut" shows a dialog with the written path.

- [ ] **Step 4: Generated shortcut + CLI arg**

Run the produced `.desktop` from your app menu (or `gtk-launch murmurssh-<id>` / open the file). Also try from a terminal against the built binary:
Verify: `murmurssh --profile <id>` (or `--profile "<Name>"`) launches and auto-connects. An unknown `--profile zzz` launches normally to the idle UI (no crash).

- [ ] **Step 5: Refresh the GitNexus index (code changed)**

Run: `npx gitnexus analyze`
Expected: index updated. (The PostToolUse hook may also do this after commits.)

---

## Self-Review Notes (already reconciled)

- **Spec coverage:** CLI arg + auto-connect (T2/T3/T7) · new-window button only-when-connected-and-different (T6 `updatePrimaryButtons`) · context menu new-window + shortcut (T6) · `.desktop` generator (T2/T3) · id-or-name resolution (T2 `resolve_profile_id`) · accordion single-open + persisted last group (T1/T5) · i18n 6 locales (T8) · null-byte/path-sep safety (T2). All present.
- **Type consistency:** `setConnected(isConnected, connectedId)` is updated at its one call site (T7 Step 2); `selectProfile`, `onOpenInNewWindow`, `onCreateShortcut`, `getLaunchProfile`/`openProfileInNewWindow`/`createDesktopShortcut` names match across selector/main/api.
- **Known limitation (carried from spec):** `.desktop` `Icon=murmurssh` is best-effort — clean for `.deb`, may not render for AppImage/dev. Confirm the installed icon name during T10 Step 4; if it differs, update the `Icon=` line in `desktop_entry` and its test.
