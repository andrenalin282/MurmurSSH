# Phase 1 — File List & Permissions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a modification-date column and a permissions column in the remote file browser, and let the user change remote file permissions (chmod) via a context-menu dialog that keeps an rwx checkbox grid in sync with an octal field.

**Architecture:** Backend gains a `perm: Option<u32>` field on `FileEntry` (populated from the SFTP `stat.perm` already read for symlink detection) and a `set_permissions` command that calls `sftp.setstat` with a perm-only `FileStat`. Frontend renders two new table columns and a new `showPermissionsDialog` modal. FTP profiles get a clear "not supported" error for chmod (FTP has no portable chmod here).

**Tech Stack:** Rust (Tauri 2, `ssh2`), TypeScript frontend (vanilla, DOM modals).

---

## CRITICAL build convention (applies to EVERY frontend task)

`tsconfig.json` has no `outDir`/`noEmit`, so `tsc` emits a `.js` file **in-place** next to each `.ts`, and these `.js` files are **tracked in git** (only `dist/` and `src-tauri/target/` are ignored). Vite loads `/src/main.ts` directly — `.ts` is the source of truth; the `.js` are committed build artifacts.

Therefore, for every frontend (`src/**`) change:
1. Edit the `.ts` file(s).
2. Regenerate the sibling `.js`: run `npx tsc` (from repo root).
3. `git add` **only** the specific `.ts` files you changed **and their matching `.js` siblings** (e.g. `git add src/components/file-browser.ts src/components/file-browser.js`). Do NOT `git add` unrelated regenerated `.js` files — if `npx tsc` rewrites other `.js` files that you did not intend to change, leave them unstaged (or `git checkout --` them) so the commit stays scoped.
4. Validate the whole build with `npm run build` (runs `tsc && vite build`).

---

## File Structure

**Backend (Rust):**
- `src-tauri/src/models/sftp.rs` — add `perm: Option<u32>` to `FileEntry`.
- `src-tauri/src/services/sftp_service.rs` — populate `perm` in `list_directory`; add `set_permissions(profile, path, mode)`.
- `src-tauri/src/services/ftp_service.rs` — add `set_permissions(profile, path, mode)` returning a clear "unsupported" error.
- `src-tauri/src/commands/sftp.rs` — add `set_permissions` command (dispatch SFTP/FTP).
- `src-tauri/src/lib.rs` — register `commands::sftp::set_permissions`.

**Frontend (TS, + regenerated JS):**
- `src/types.ts` — add `perm: number | null` to `FileEntry`.
- `src/api/index.ts` — add `setPermissions` wrapper.
- `src/components/dialog.ts` — add `showPermissionsDialog`.
- `src/components/file-browser.ts` — date/perm format helpers, two new columns (fix colspans), permissions context-menu entries + handler, ICONS entry.
- `src/i18n/{en,de,fr,nl,pl,ru}.ts` — new strings.

**Scope note:** The new columns and chmod apply to the **remote** browser (`file-browser.ts`) only. The local browser (`local-file-browser.ts`) is intentionally out of scope for Phase 1 (local files are better managed by the OS file manager; chmod here is remote-focused). This is a deliberate YAGNI boundary.

---

## Task 1: Add `perm` to FileEntry (backend model + populate + TS type)

**Files:**
- Modify: `src-tauri/src/models/sftp.rs`
- Modify: `src-tauri/src/services/sftp_service.rs` (`list_directory`)
- Modify: `src/types.ts`

- [ ] **Step 1: Add the field to the Rust model**

In `src-tauri/src/models/sftp.rs`, change the struct to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    /// Unix timestamp in seconds.
    pub modified: Option<u64>,
    /// Unix permission/mode bits (e.g. 0o644). None when unavailable (e.g. FTP).
    pub perm: Option<u32>,
}
```

- [ ] **Step 2: Populate it in `list_directory`**

In `src-tauri/src/services/sftp_service.rs`, in the `FileEntry { ... }` literal inside `list_directory`, add the `perm` field so it reads:

```rust
                FileEntry {
                    name: name.to_string_lossy().to_string(),
                    is_dir,
                    size: stat.size,
                    modified: stat.mtime,
                    perm: stat.perm,
                }
```

- [ ] **Step 3: Find any other `FileEntry { ... }` constructions and fix them**

Run: `grep -rn "FileEntry {" src-tauri/src`
For each construction (e.g. in `ftp_service.rs`), add `perm: None,` (FTP listings don't expose mode bits here). Expected: the FTP `list_directory` builds `FileEntry` — add `perm: None`. Verify none are missed (compile will catch it).

- [ ] **Step 4: Update the TS type**

In `src/types.ts`, change the `FileEntry` interface to add:

```typescript
export interface FileEntry {
  name: string;
  is_dir: boolean;
  size: number | null;
  /** Unix timestamp in seconds. */
  modified: number | null;
  /** Unix permission/mode bits (e.g. 0o644). Null when unavailable (e.g. FTP). */
  perm: number | null;
}
```

- [ ] **Step 5: Compile both sides**

Run: `cd src-tauri && cargo check 2>&1 | tail -15` → must be clean (fixes any missed `FileEntry` literal).
Run (repo root): `npx tsc 2>&1 | tail -15` → must be clean. Then regenerate JS is automatic (tsc emitted `src/types.js`).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models/sftp.rs src-tauri/src/services/sftp_service.rs src-tauri/src/services/ftp_service.rs src/types.ts src/types.js
git commit -m "feat(files): add perm (mode bits) to FileEntry across backend and TS"
```

---

## Task 2: `set_permissions` backend command + API wrapper

**Files:**
- Modify: `src-tauri/src/services/sftp_service.rs`
- Modify: `src-tauri/src/services/ftp_service.rs`
- Modify: `src-tauri/src/commands/sftp.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/api/index.ts`

- [ ] **Step 1: Add the SFTP service function**

In `src-tauri/src/services/sftp_service.rs`, add (mirroring `rename_file`'s connect/sftp pattern):

```rust
/// Change the Unix permission bits of a remote file or directory.
/// `mode` is the permission value (e.g. 0o644); only the perm field is set,
/// leaving size/uid/gid/atime/mtime untouched on the server.
pub fn set_permissions(profile: &Profile, path: &str, mode: u32) -> Result<(), String> {
    let session = connect(profile)?;
    let sftp = session
        .sftp()
        .map_err(|e| format!("Failed to open SFTP channel: {}", e))?;

    let stat = ssh2::FileStat {
        size: None,
        uid: None,
        gid: None,
        perm: Some(mode),
        atime: None,
        mtime: None,
    };
    sftp.setstat(Path::new(path), stat)
        .map_err(|e| format!("Failed to change permissions of '{}': {}", path, e))
}
```

> If `ssh2::FileStat` is already imported in the file, use the short form; otherwise the fully-qualified `ssh2::FileStat` above is fine. Confirm `ssh2` is referenced elsewhere in the file (it is — the session type comes from `ssh2`).

- [ ] **Step 2: Add the FTP stub (unsupported)**

In `src-tauri/src/services/ftp_service.rs`, add:

```rust
/// Changing permissions is not supported over FTP in MurmurSSH.
pub fn set_permissions(_profile: &Profile, _path: &str, _mode: u32) -> Result<(), String> {
    Err("Changing permissions is not supported over FTP.".to_string())
}
```

- [ ] **Step 3: Add the command**

In `src-tauri/src/commands/sftp.rs`, add (mirroring `rename_file`):

```rust
/// Change the Unix permission bits (mode) of a remote file or directory.
#[tauri::command]
pub fn set_permissions(
    profile_id: String,
    remote_path: String,
    mode: u32,
) -> Result<(), String> {
    let profile = profile_service::get_profile(&profile_id)?;
    if is_ftp(&profile) {
        ftp_service::set_permissions(&profile, &remote_path, mode)
    } else {
        sftp_service::set_permissions(&profile, &remote_path, mode)
    }
}
```

- [ ] **Step 4: Register the command**

In `src-tauri/src/lib.rs`, add to the `generate_handler!` list (next to `commands::sftp::rename_file`):

```rust
            commands::sftp::set_permissions,
```

- [ ] **Step 5: Add the API wrapper**

In `src/api/index.ts`, after `renameFile`, add:

```typescript
/**
 * Change the Unix permission bits (mode) of a remote file or directory.
 * `mode` is the integer permission value (e.g. 0o644 === 420).
 * Returns an error string for FTP profiles (unsupported).
 */
export async function setPermissions(
  profileId: string,
  remotePath: string,
  mode: number
): Promise<void> {
  return invoke("set_permissions", { profileId, remotePath, mode });
}
```

- [ ] **Step 6: Compile**

Run: `cd src-tauri && cargo check 2>&1 | tail -15` (clean).
Run (root): `npx tsc 2>&1 | tail -15` (clean; regenerates `src/api/index.js`).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/services/sftp_service.rs src-tauri/src/services/ftp_service.rs src-tauri/src/commands/sftp.rs src-tauri/src/lib.rs src/api/index.ts src/api/index.js
git commit -m "feat(files): add set_permissions command + API wrapper (SFTP chmod; FTP unsupported)"
```

---

## Task 3: Modified + Permissions columns in the file browser

**Files:**
- Modify: `src/components/file-browser.ts`

- [ ] **Step 1: Add format helpers**

In `src/components/file-browser.ts`, near the existing `formatBytes` function (around line 93), add:

```typescript
/** Format a Unix timestamp (seconds) as a locale date-time, or "—" if absent. */
function formatDate(sec: number | null): string {
  if (sec == null) return "—";
  const d = new Date(sec * 1000);
  if (isNaN(d.getTime())) return "—";
  return d.toLocaleString();
}

/** Format Unix mode bits as a symbolic permission string, e.g. "rwxr-xr-x". */
function formatPermSymbolic(mode: number | null, isDir: boolean): string {
  if (mode == null) return "—";
  const bits = mode & 0o777;
  const rwx = (n: number) =>
    `${n & 4 ? "r" : "-"}${n & 2 ? "w" : "-"}${n & 1 ? "x" : "-"}`;
  const type = isDir ? "d" : "-";
  return `${type}${rwx((bits >> 6) & 7)}${rwx((bits >> 3) & 7)}${rwx(bits & 7)}`;
}

/** Format Unix mode bits as a 3-digit octal string, e.g. "755", or "" if absent. */
function formatPermOctal(mode: number | null): string {
  if (mode == null) return "";
  return (mode & 0o777).toString(8).padStart(3, "0");
}
```

- [ ] **Step 2: Update the table header**

In `render()`, change the header row (currently):

```typescript
            <tr><th>${t("fileBrowser.columnName")}</th><th>${t("fileBrowser.columnSize")}</th></tr>
```

to:

```typescript
            <tr><th>${t("fileBrowser.columnName")}</th><th>${t("fileBrowser.columnSize")}</th><th>${t("fileBrowser.columnModified")}</th><th>${t("fileBrowser.columnPermissions")}</th></tr>
```

- [ ] **Step 3: Update the data row**

In `render()`, change the entry row template (currently the two `<td>` lines) to four columns:

```typescript
                return `<tr class="${cls}" data-name="${escHtml(entry.name)}" data-isdir="${entry.is_dir}" draggable="true">
                   <td>${entry.is_dir ? "&#128193; " : ""}${escHtml(entry.name)}</td>
                   <td>${entry.size != null && !entry.is_dir ? formatBytes(entry.size) : "—"}</td>
                   <td>${formatDate(entry.modified)}</td>
                   <td title="${formatPermOctal(entry.perm)}">${formatPermSymbolic(entry.perm, entry.is_dir)}</td>
                 </tr>`;
```

- [ ] **Step 4: Fix the colspans (2 → 4)**

In `render()`, update the up-row and empty-dir cells:
- Change `<td colspan="2">${t("fileBrowser.upRow")}</td>` to `colspan="4"`.
- Change `<tr><td colspan="2" class="empty-dir">${t("fileBrowser.emptyDir")}</td></tr>` to `colspan="4"`.

Run: `grep -n 'colspan="2"' src/components/file-browser.ts` — there should be **zero** matches afterward (confirm all were updated).

- [ ] **Step 5: Build + verify**

Run (root): `npx tsc 2>&1 | tail -15` (clean). Then `npm run build 2>&1 | tail -8` (clean bundle).

- [ ] **Step 6: Commit**

```bash
git add src/components/file-browser.ts src/components/file-browser.js
git commit -m "feat(files): show modification-date and permissions columns in file browser"
```

---

## Task 4: `showPermissionsDialog` (rwx grid ↔ octal)

**Files:**
- Modify: `src/components/dialog.ts`

- [ ] **Step 1: Add the dialog**

In `src/components/dialog.ts`, add a new exported function (mirroring the existing modal pattern). It shows a 3×3 rwx checkbox grid synced bidirectionally with an octal input, returns the chosen mode (0–0o777) or `null` if cancelled:

```typescript
/**
 * Show the change-permissions (chmod) dialog for a single entry.
 * Presents an rwx checkbox grid (owner/group/other) kept in sync with an
 * octal text field. Returns the selected mode (integer, 0..=0o777) or null
 * if cancelled.
 *
 * @param name      Display name of the target entry (for the title).
 * @param initial   Current mode bits (only the low 9 bits are used); defaults to 0o644.
 */
export function showPermissionsDialog(
  name: string,
  initial: number | null
): Promise<number | null> {
  return new Promise((resolve) => {
    const start = ((initial ?? 0o644) & 0o777);

    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    const classes: Array<["owner" | "group" | "other", number]> = [
      ["owner", 6],
      ["group", 3],
      ["other", 0],
    ];
    const perms: Array<["r" | "w" | "x", number]> = [
      ["r", 4],
      ["w", 2],
      ["x", 1],
    ];

    const rowsHtml = classes
      .map(
        ([cls]) => `
        <tr>
          <td class="perm-grid__label">${t(`dialogs.perm_${cls}`)}</td>
          ${perms
            .map(
              ([p]) =>
                `<td><input type="checkbox" class="perm-cb" data-cls="${cls}" data-perm="${p}"></td>`
            )
            .join("")}
        </tr>`
      )
      .join("");

    overlay.innerHTML = `
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal__title">${t("dialogs.permTitle")}: ${escHtml(name)}</div>
        <table class="perm-grid">
          <thead>
            <tr>
              <th></th>
              <th>${t("dialogs.perm_read")}</th>
              <th>${t("dialogs.perm_write")}</th>
              <th>${t("dialogs.perm_execute")}</th>
            </tr>
          </thead>
          <tbody>${rowsHtml}</tbody>
        </table>
        <div class="form-field perm-grid__octal">
          <label>${t("dialogs.permOctal")}
            <input id="perm-octal" type="text" inputmode="numeric" maxlength="4" autocomplete="off" value="${start
              .toString(8)
              .padStart(3, "0")}">
          </label>
        </div>
        <div class="modal__actions">
          <button class="btn-secondary" id="perm-cancel">${t("dialogs.promptCancel")}</button>
          <button id="perm-apply">${t("dialogs.permApply")}</button>
        </div>
      </div>
    `;

    document.body.appendChild(overlay);

    const octalInput = overlay.querySelector<HTMLInputElement>("#perm-octal")!;
    const cbs = Array.from(overlay.querySelectorAll<HTMLInputElement>(".perm-cb"));

    const clsShift: Record<string, number> = { owner: 6, group: 3, other: 0 };
    const permBit: Record<string, number> = { r: 4, w: 2, x: 1 };

    // Reflect a mode value into the checkbox grid.
    const modeToGrid = (mode: number) => {
      for (const cb of cbs) {
        const shift = clsShift[cb.dataset.cls!];
        const bit = permBit[cb.dataset.perm!];
        cb.checked = (((mode >> shift) & 7) & bit) !== 0;
      }
    };

    // Read the current grid state back into a mode value.
    const gridToMode = (): number => {
      let mode = 0;
      for (const cb of cbs) {
        if (cb.checked) {
          mode |= permBit[cb.dataset.perm!] << clsShift[cb.dataset.cls!];
        }
      }
      return mode;
    };

    // Initialise grid from the starting octal value.
    modeToGrid(start);

    // Grid change → update octal field.
    for (const cb of cbs) {
      cb.addEventListener("change", () => {
        octalInput.value = gridToMode().toString(8).padStart(3, "0");
      });
    }

    // Octal field change → update grid (only when it parses to a valid 0..=0o777).
    octalInput.addEventListener("input", () => {
      const raw = octalInput.value.trim();
      if (!/^[0-7]{1,4}$/.test(raw)) return;
      const parsed = parseInt(raw, 8) & 0o777;
      modeToGrid(parsed);
    });

    const cleanup = (result: number | null) => {
      overlay.remove();
      resolve(result);
    };

    overlay.querySelector("#perm-cancel")?.addEventListener("click", () => cleanup(null));
    overlay.querySelector("#perm-apply")?.addEventListener("click", () => {
      // Apply uses the authoritative grid state (covers a half-typed octal field).
      cleanup(gridToMode());
    });

    setTimeout(() => overlay.querySelector<HTMLButtonElement>("#perm-apply")?.focus(), 10);
  });
}
```

> Note: the template literal uses `t(\`dialogs.perm_${cls}\`)` with backticks inside the function — keep that exact form. The CSS classes `perm-grid`, `perm-grid__label`, `perm-grid__octal` reuse existing modal styling; no new CSS is strictly required for function (a later polish task may style the grid), but if the grid looks unstyled that is acceptable for this phase.

- [ ] **Step 2: Build**

Run (root): `npx tsc 2>&1 | tail -15` (clean — note `dialog.ts` already imports `t` and defines `escHtml`). Then `npm run build 2>&1 | tail -8`.

- [ ] **Step 3: Commit**

```bash
git add src/components/dialog.ts src/components/dialog.js
git commit -m "feat(files): add chmod dialog (rwx grid synced with octal field)"
```

---

## Task 5: Wire chmod into the context menu + handler

**Files:**
- Modify: `src/components/file-browser.ts`

- [ ] **Step 1: Import the dialog**

In the import on line 6, add `showPermissionsDialog`:

```typescript
import { showConfirm, showPrompt, showOverwriteDialog, showPermissionsDialog } from "./dialog";
```

- [ ] **Step 2: Add a permissions icon to ICONS**

In the `ICONS` object (around line 70), add a `permissions` entry (a lock/sliders glyph):

```typescript
  permissions:  `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>`,
```

- [ ] **Step 3: Add the handler method**

Add a new method to the `FileBrowser` class (near `handleRename`, around line 1688). This mirrors `handleRename`'s exact error-handling pattern (`setBusy` + `this.status(msg, isError)` + `this.normalizeRemoteError`), and uses the **module-level** `joinPath(dir, name)` function (NOT `this.joinPath`) and the `selectedEntry` getter — both verified to exist:

```typescript
  private async handlePermissions(): Promise<void> {
    const entry = this.selectedEntry;
    if (!this.profileId || !entry) return;

    const newMode = await showPermissionsDialog(entry.name, entry.perm);
    if (newMode === null) return; // cancelled

    const remotePath = joinPath(this.currentPath, entry.name);
    try {
      this.setBusy(true);
      await api.setPermissions(this.profileId, remotePath, newMode);
      this.status(t("fileBrowser.permissionsChanged", { name: entry.name }), false);
      await this.refresh();
    } catch (err) {
      this.status(t("fileBrowser.permissionsFailed", { error: this.normalizeRemoteError(err) }), true);
    } finally {
      this.setBusy(false);
    }
  }
```

> Verified symbols: `joinPath` (module fn, line ~100), `selectedEntry` (getter, line ~383), `this.status(msg, isError)`, `this.setBusy(bool)`, `this.normalizeRemoteError(err)`, `this.refresh()` — all used identically in `handleRename`. No adaptation needed; if any line fails to typecheck, copy the exact call form from `handleRename`.

- [ ] **Step 4: Add the menu entries**

In `buildFileContextItems` and `buildFolderContextItems`, add a permissions entry in the second group (after Move to…, before the delete separator). For `buildFileContextItems`:

```typescript
      { icon: ICONS.moveTo,   label: t("fileBrowser.moveTo"),   action: () => this.handleMoveTo() },
      { icon: ICONS.permissions, label: t("fileBrowser.permissions"), action: () => this.handlePermissions() },
      { separator: true },
      { icon: ICONS.delete,   label: t("fileBrowser.delete"),   action: () => this.handleDelete(), danger: true },
```

Apply the identical insertion (the `permissions` line) in `buildFolderContextItems` after its `moveTo` line.

- [ ] **Step 5: Build + verify**

Run (root): `npx tsc 2>&1 | tail -15` (clean — `noUnusedLocals`/`noUnusedParameters` are on, so make sure every added symbol is used). Then `npm run build 2>&1 | tail -8`.

- [ ] **Step 6: Commit**

```bash
git add src/components/file-browser.ts src/components/file-browser.js
git commit -m "feat(files): chmod context-menu action wired to set_permissions"
```

---

## Task 6: i18n strings (all 6 locales)

**Files:**
- Modify: `src/i18n/en.ts`, `de.ts`, `fr.ts`, `nl.ts`, `pl.ts`, `ru.ts`

Add the new keys into the existing nested sections of EACH locale file:
- Into the `fileBrowser: { ... }` object: `columnModified`, `columnPermissions`, `permissions`, `permissionsChanged`, `permissionsFailed`.
- Into the `dialogs: { ... }` object: `permTitle`, `permApply`, `permOctal`, `perm_owner`, `perm_group`, `perm_other`, `perm_read`, `perm_write`, `perm_execute`.

Use these exact values per locale (place each key in the correct nested object; match the file's existing indentation/quoting style). `permissionsChanged` uses the `{name}` interpolation placeholder consistent with how other interpolated strings in the file are written (e.g. `itemsSelected`).

**en.ts**
```
// in fileBrowser:
columnModified: "Modified",
columnPermissions: "Permissions",
permissions: "Permissions…",
permissionsChanged: "Permissions changed: {name}",
permissionsFailed: "Failed to change permissions: {error}",
// in dialogs:
permTitle: "Change permissions",
permApply: "Apply",
permOctal: "Octal",
perm_owner: "Owner",
perm_group: "Group",
perm_other: "Other",
perm_read: "Read",
perm_write: "Write",
perm_execute: "Execute",
```

**de.ts**
```
columnModified: "Geändert",
columnPermissions: "Rechte",
permissions: "Rechte ändern…",
permissionsChanged: "Rechte geändert: {name}",
permissionsFailed: "Rechte konnten nicht geändert werden: {error}",
permTitle: "Rechte ändern",
permApply: "Übernehmen",
permOctal: "Oktal",
perm_owner: "Eigentümer",
perm_group: "Gruppe",
perm_other: "Andere",
perm_read: "Lesen",
perm_write: "Schreiben",
perm_execute: "Ausführen",
```

**fr.ts**
```
columnModified: "Modifié",
columnPermissions: "Permissions",
permissions: "Permissions…",
permissionsChanged: "Permissions modifiées : {name}",
permissionsFailed: "Échec de la modification des permissions : {error}",
permTitle: "Modifier les permissions",
permApply: "Appliquer",
permOctal: "Octal",
perm_owner: "Propriétaire",
perm_group: "Groupe",
perm_other: "Autres",
perm_read: "Lecture",
perm_write: "Écriture",
perm_execute: "Exécution",
```

**nl.ts**
```
columnModified: "Gewijzigd",
columnPermissions: "Rechten",
permissions: "Rechten…",
permissionsChanged: "Rechten gewijzigd: {name}",
permissionsFailed: "Wijzigen van rechten mislukt: {error}",
permTitle: "Rechten wijzigen",
permApply: "Toepassen",
permOctal: "Octaal",
perm_owner: "Eigenaar",
perm_group: "Groep",
perm_other: "Overige",
perm_read: "Lezen",
perm_write: "Schrijven",
perm_execute: "Uitvoeren",
```

**pl.ts**
```
columnModified: "Zmodyfikowano",
columnPermissions: "Uprawnienia",
permissions: "Uprawnienia…",
permissionsChanged: "Zmieniono uprawnienia: {name}",
permissionsFailed: "Nie udało się zmienić uprawnień: {error}",
permTitle: "Zmień uprawnienia",
permApply: "Zastosuj",
permOctal: "Ósemkowo",
perm_owner: "Właściciel",
perm_group: "Grupa",
perm_other: "Inni",
perm_read: "Odczyt",
perm_write: "Zapis",
perm_execute: "Wykonanie",
```

**ru.ts**
```
columnModified: "Изменён",
columnPermissions: "Права",
permissions: "Права…",
permissionsChanged: "Права изменены: {name}",
permissionsFailed: "Не удалось изменить права: {error}",
permTitle: "Изменить права",
permApply: "Применить",
permOctal: "Восьмеричный",
perm_owner: "Владелец",
perm_group: "Группа",
perm_other: "Прочие",
perm_read: "Чтение",
perm_write: "Запись",
perm_execute: "Выполнение",
```

- [ ] **Step 1:** Add all keys to all 6 `.ts` locale files in the correct nested sections.
- [ ] **Step 2:** Run (root): `npx tsc 2>&1 | tail -15` (clean — regenerates the 6 `.js` locale files).
- [ ] **Step 3:** Run `npm run build 2>&1 | tail -8` (clean).
- [ ] **Step 4: Commit**

```bash
git add src/i18n/en.ts src/i18n/en.js src/i18n/de.ts src/i18n/de.js src/i18n/fr.ts src/i18n/fr.js src/i18n/nl.ts src/i18n/nl.js src/i18n/pl.ts src/i18n/pl.js src/i18n/ru.ts src/i18n/ru.js
git commit -m "i18n: permissions and file-list column strings (en/de/fr/nl/pl/ru)"
```

> If `npx tsc` reformats the locale `.js` files beyond your key additions (a known pre-existing inconsistency), that is fine — committing the regenerated `.js` brings them back in sync with their `.ts` source.

---

## Task 7: Build verification, manual acceptance, docs + release

- [ ] **Step 1: Full build**

Run: `cd src-tauri && cargo build 2>&1 | tail -8` (Finished, no errors).
Run (root): `npm run build 2>&1 | tail -8` (clean).

- [ ] **Step 2: Manual acceptance (requires a real SFTP server — performed/confirmed by the user)**

1. File list shows a Modified column (locale date) and a Permissions column (e.g. `-rw-r--r--`, octal in tooltip).
2. Right-click a file → "Permissions…" → toggle checkboxes ⇄ octal field stays in sync; edit octal ⇄ grid updates.
3. Apply → permission column reflects the new value after refresh.
4. On an FTP profile, "Permissions…" surfaces the "not supported over FTP" error without crashing.

- [ ] **Step 3: CHANGELOG**

Move `[Unreleased]` content into a new `## [1.4.8] - 2026-06-10` section (mirror the existing format) with:

```markdown
### Added
- File browser now shows a modification-date column and a permissions column (symbolic, with octal in the tooltip).
- Change remote file/folder permissions via the right-click "Permissions…" action: an rwx checkbox grid kept in sync with an octal field (SFTP only; FTP reports it as unsupported).
```

Leave `## [Unreleased]` with "No changes yet.".

- [ ] **Step 4: README** — if there is a features list, add a one-line mention of the permissions column + chmod. Otherwise skip (report which).

- [ ] **Step 5: CLAUDE.md** — add a `- Phase 10.1 (v1.4.8): ...` line under "Phases Complete" summarizing the perm field, set_permissions command, columns, and chmod dialog.

- [ ] **Step 6: vault note** — update `vault/MurmurSSH Optimization Roadmap.md`: set Phase 1 status to ✅ done — v1.4.8 and add a short outcome paragraph.

- [ ] **Step 7: Version bump + tag + push**

Bump 1.4.7 → 1.4.8 in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`; run `cd src-tauri && cargo check` to refresh `Cargo.lock`. Then:

```bash
git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json CHANGELOG.md README.md CLAUDE.md "vault/MurmurSSH Optimization Roadmap.md"
git commit -m "release: v1.4.8 — file-list columns and remote permissions (chmod)"
git tag -a v1.4.8 -m "v1.4.8 — Phase 1: modification-date + permissions columns, chmod dialog"
git push origin main --follow-tags
```

- [ ] **Step 8:** Run `npx gitnexus analyze` to refresh the index.

---

## Notes for the executor

- Respect the `.ts`→`.js` regeneration rule on EVERY frontend commit (edit `.ts`, `npx tsc`, stage the matching `.js`).
- `noUnusedLocals`/`noUnusedParameters` are enabled — unused additions fail `tsc`.
- Keep chmod SFTP-only; the FTP path returns a clear error by design.
- If `set_permissions`'s `ssh2::FileStat` field set differs in the installed `ssh2` version, adjust to the available fields (perm is the only one that must be `Some`).
