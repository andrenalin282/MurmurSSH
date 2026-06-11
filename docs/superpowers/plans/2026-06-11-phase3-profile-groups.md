# Phase 3 — Profile Groups & Sorting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users organize connection profiles into named groups shown as a collapsible grouped tree in the profile selector, with a per-session collapse state and a persisted sort toggle (alphabetical | creation date).

**Architecture:** Add two backward-compatible fields to `Profile` — `group: Option<String>` and `created_at: Option<u64>` (epoch seconds). `created_at` is stamped at creation in `save_profile` (preserved on edit); for legacy profiles that lack it, `list_profiles` fills it from the JSON file's mtime so creation-date sorting still works. The selector's `<select>` is replaced by a grouped tree (group headers with collapse carets + profile rows) while keeping the component's public API unchanged so `main.ts` needs no rewiring. A sort toggle (A–Z | Newest) is persisted in Settings (`profile_sort`). The profile form gains a free-text group field with a datalist of existing groups.

**Tech Stack:** Rust (`serde`, `std::time`, `std::fs`), vanilla TypeScript frontend, existing `getSettings`/`saveSettings` IPC.

---

## Key decisions (read first)

1. **Sorting + grouping happen in the frontend selector.** The backend only supplies `group` + `created_at`; `list_profiles` keeps its existing alphabetical sort (a stable base order). The selector regroups and re-sorts per the toggle.
2. **Collapse state is per-session (in-memory in the component), NOT persisted.** Only the sort mode is persisted (Settings `profile_sort`).
3. **Ungrouped profiles** (no `group`, or empty/whitespace) go into a default group rendered with an i18n label (`profiles.ungrouped`) and sorted LAST among groups.
4. **`created_at` is never overwritten on edit** — only set once at first save. Legacy profiles get a mtime-derived value at list time (not written back to disk unless re-saved).
5. **Public selector API is unchanged.** `init() -> string|null`, `reload(selectId?)`, `onConnect/onNew/onEdit/onDelete`, `getSelectedProfile()`, `setConnected`, `setConnecting` all keep their signatures and semantics. Selection moves from a `<select>` change event to a row click; `selectedId` stays the source of truth.

---

## File structure

**Backend — modified:**
- `src-tauri/src/models/profile.rs` — add `group` + `created_at` to `Profile`.
- `src-tauri/src/models/settings.rs` — add `profile_sort: Option<String>`.
- `src-tauri/src/services/profile_service.rs` — stamp `created_at` in `save_profile`; backfill from mtime in `list_profiles`.

**Frontend — modified:**
- `src/types.ts` — add `group`/`created_at` to `Profile`; `profile_sort` to `Settings`.
- `src/components/profile-selector.ts` — grouped tree render + sort toggle (the bulk of this phase).
- `src/components/profile-form.ts` — group input + datalist; carry `created_at` forward; set `group` on save.
- `src/styles.css` — tree / group-header / row / sort-toggle styles.
- `src/i18n/{en,de,fr,nl,pl,ru}.{ts,js}` — new strings.

**Frontend build rule (MANDATORY, from CLAUDE.md):** edit `.ts` → run `npx tsc` → `git add` BOTH `.ts` and its `.js`. Before committing, `git checkout --` any unrelated regenerated `.js` (esp. `src/i18n/index.js`).

---

## Task 1: Profile model fields (backward-compatible)

**Files:**
- Modify: `src-tauri/src/models/profile.rs`

- [ ] **Step 1: Add a failing backward-compat test**

Add to `src-tauri/src/models/profile.rs` (create a `#[cfg(test)]` module if none exists):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_legacy_profile_without_group_or_created_at() {
        // A profile JSON written before Phase 3 has neither field.
        let json = r#"{
            "id": "old",
            "name": "Old",
            "host": "h",
            "port": 22,
            "username": "u",
            "auth_type": "agent",
            "key_path": null,
            "default_remote_path": null,
            "editor_command": null,
            "upload_mode": "confirm"
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.group, None);
        assert_eq!(p.created_at, None);
    }

    #[test]
    fn group_and_created_at_round_trip() {
        let json = r#"{
            "id": "x","name":"X","host":"h","port":22,"username":"u",
            "auth_type":"agent","key_path":null,"default_remote_path":null,
            "editor_command":null,"upload_mode":"confirm",
            "group":"Work","created_at":1700000000
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.group.as_deref(), Some("Work"));
        assert_eq!(p.created_at, Some(1_700_000_000));
    }
}
```

- [ ] **Step 2: Run the test to confirm it fails**

Run: `cd src-tauri && cargo test --lib models::profile`
Expected: FAIL to compile (`group`/`created_at` are not fields of `Profile`).

- [ ] **Step 3: Add the fields**

In `Profile` (after `local_paths_by_user`), add:

```rust
    /// Optional group/folder this profile belongs to (free-text). Empty/None = ungrouped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    /// Profile creation time, epoch seconds. Set once at first save; preserved on edit.
    /// Legacy profiles without it are backfilled from the JSON file mtime at list time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
```

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test --lib models::profile`
Expected: PASS (2 tests). `cargo check` clean.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/models/profile.rs
git commit -m "feat(profiles): add backward-compatible group and created_at fields to Profile

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Stamp created_at on save; backfill from mtime on list; Settings.profile_sort

**Files:**
- Modify: `src-tauri/src/services/profile_service.rs`
- Modify: `src-tauri/src/models/settings.rs`

- [ ] **Step 1: Add the Settings field**

In `src-tauri/src/models/settings.rs`, add after `max_concurrent_transfers`:

```rust
    /// Profile list sort mode: "name" (alphabetical) or "created" (creation date).
    /// When None, defaults to "name" on the frontend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_sort: Option<String>,
```

- [ ] **Step 2: Stamp `created_at` in `save_profile`**

In `src-tauri/src/services/profile_service.rs`, add a helper and use it in `save_profile`. At the top, add the import:

```rust
use std::time::{SystemTime, UNIX_EPOCH};
```

Add this helper near `config_base`:

```rust
/// Current time as epoch seconds (0 if the clock is before the epoch — never panics).
fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
```

Change `save_profile` so it stamps `created_at` when absent without mutating the caller's `&Profile` unexpectedly. Replace the body of `save_profile` with:

```rust
pub fn save_profile(profile: &Profile) -> Result<(), String> {
    ensure_dirs()?;
    let path = profiles_dir().join(format!("{}.json", profile.id));

    // Stamp created_at exactly once: keep the existing value if the on-disk
    // profile already has one, otherwise use the incoming value or now().
    let mut to_write = profile.clone();
    if to_write.created_at.is_none() {
        let existing = get_profile(&profile.id).ok().and_then(|p| p.created_at);
        to_write.created_at = Some(existing.unwrap_or_else(now_epoch_secs));
    }

    // Create a backup of the existing file before overwriting
    if path.exists() {
        let bkp = profiles_dir().join(format!("{}.json.bkp", profile.id));
        fs::copy(&path, &bkp).map_err(|e| format!("Failed to create profile backup: {}", e))?;
    }
    let json = serde_json::to_string_pretty(&to_write)
        .map_err(|e| format!("Failed to serialize profile: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write profile: {}", e))
}
```

- [ ] **Step 3: Backfill `created_at` from file mtime in `list_profiles`**

In `list_profiles`, when a parsed profile has `created_at == None`, fill it from the file's mtime (so creation-date sort works for legacy profiles). After parsing each profile, before pushing:

```rust
            let mut profile: Profile = serde_json::from_str(&contents)
                .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
            if profile.created_at.is_none() {
                profile.created_at = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
            }
            profiles.push(profile);
```

(Adjust the existing `let profile = ...; profiles.push(profile);` lines to the above. Keep the trailing `profiles.sort_by(|a, b| a.name.cmp(&b.name));` as the stable base order.)

- [ ] **Step 4: Add a test for created_at stamping**

Add to a `#[cfg(test)]` module in `profile_service.rs` a focused unit test for `now_epoch_secs` returning a plausibly-large value, OR — since `save_profile` touches the real filesystem/`$HOME` — keep this verification manual and instead assert `now_epoch_secs() > 1_600_000_000` (after 2020). Add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_epoch_secs_is_after_2020() {
        assert!(now_epoch_secs() > 1_600_000_000);
    }
}
```

- [ ] **Step 5: Build + test**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -5 && cargo build 2>&1 | tail -3 && cargo clippy 2>&1 | tail -5`
Expected: all pass, clean build, no warnings.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/profile_service.rs src-tauri/src/models/settings.rs
git commit -m "feat(profiles): stamp created_at on save, backfill from mtime on list; add profile_sort setting

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Frontend types

**Files:**
- Modify: `src/types.ts`

- [ ] **Step 1: Extend Profile and Settings**

In `src/types.ts`, in the `Profile` interface add (near the other optional fields):

```typescript
  /** Optional group/folder name. Empty/absent = ungrouped. */
  group?: string | null;
  /** Profile creation time, epoch seconds. Set by the backend. */
  created_at?: number | null;
```

In the `Settings` interface add:

```typescript
  /** Profile sort mode: "name" | "created". Null/absent = "name". */
  profile_sort?: "name" | "created" | null;
```

- [ ] **Step 2: Compile**

Run: `npx tsc 2>&1 | head -20`
Expected: 0 errors. Revert any unrelated regenerated `.js` (`git checkout -- src/i18n/index.js` if it appears).

- [ ] **Step 3: Commit**

```bash
git add src/types.ts src/types.js
git commit -m "feat(profiles): add group/created_at to Profile type and profile_sort to Settings type

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Grouped tree-view selector + sort toggle

**Files:**
- Modify: `src/components/profile-selector.ts`
- Modify: `src/styles.css`

**Constraint:** Keep the PUBLIC API and all existing behavior (init/reload/onConnect/onNew/onEdit/onDelete/getSelectedProfile/setConnected/setConnecting). Only `render()` and selection-handling change, plus two new private fields and a settings read/write for the sort mode. The element IDs `connect-btn`, `edit-profile-btn`, `delete-profile-btn`, `new-profile-btn` MUST be preserved (other methods + CSS reference them).

- [ ] **Step 1: Add state fields + sort persistence**

Add private fields:

```typescript
  private collapsedGroups = new Set<string>();
  private sortMode: "name" | "created" = "name";
```

In `init()`, after loading settings, set the sort mode:

```typescript
    this.sortMode = settings.profile_sort === "created" ? "created" : "name";
```

Add a method to persist the sort mode (read-merge-write so other settings are preserved):

```typescript
  private async persistSortMode(): Promise<void> {
    try {
      const settings = await api.getSettings();
      await api.saveSettings({ ...settings, profile_sort: this.sortMode });
    } catch {
      /* non-fatal: sort still applies for this session */
    }
  }
```

- [ ] **Step 2: Add grouping/sorting helpers**

Add these private helpers (use a constant for the ungrouped key):

```typescript
  /** Group profiles by their `group` field; ungrouped under the empty-string key. */
  private groupedProfiles(): Map<string, Profile[]> {
    const groups = new Map<string, Profile[]>();
    for (const p of this.profiles) {
      const key = (p.group ?? "").trim();
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(p);
    }
    // Sort within each group by the active mode.
    for (const list of groups.values()) {
      list.sort((a, b) => {
        if (this.sortMode === "created") {
          // Newest first; missing created_at treated as oldest.
          return (b.created_at ?? 0) - (a.created_at ?? 0);
        }
        return a.name.localeCompare(b.name);
      });
    }
    return groups;
  }

  /** Group keys in display order: named groups A–Z, ungrouped ("") last. */
  private orderedGroupKeys(groups: Map<string, Profile[]>): string[] {
    return [...groups.keys()].sort((a, b) => {
      if (a === "") return 1;   // ungrouped last
      if (b === "") return -1;
      return a.localeCompare(b);
    });
  }
```

- [ ] **Step 3: Rewrite `render()` as a grouped tree**

Replace the existing `render()` with:

```typescript
  private render(): void {
    const hasProfiles = this.profiles.length > 0;
    const hasSelection = this.selectedId !== null && hasProfiles;

    let treeHtml: string;
    if (!hasProfiles) {
      treeHtml = `<div class="profile-tree__empty">${t("profiles.noProfiles")}</div>`;
    } else {
      const groups = this.groupedProfiles();
      const keys = this.orderedGroupKeys(groups);
      treeHtml = keys
        .map((key) => {
          const list = groups.get(key)!;
          const label = key === "" ? t("profiles.ungrouped") : escapeHtml(key);
          const collapsed = this.collapsedGroups.has(key);
          const caret = collapsed ? "▸" : "▾";
          const rows = collapsed
            ? ""
            : list
                .map(
                  (p) =>
                    `<div class="profile-row${p.id === this.selectedId ? " profile-row--selected" : ""}" data-id="${escapeHtml(p.id)}" role="option" aria-selected="${p.id === this.selectedId}" title="${escapeHtml(p.username)}@${escapeHtml(p.host)}">${escapeHtml(p.name)}</div>`
                )
                .join("");
          return `
            <div class="profile-group">
              <div class="profile-group__header" data-group="${escapeHtml(key)}">
                <span class="profile-group__caret">${caret}</span>
                <span class="profile-group__name">${label}</span>
                <span class="profile-group__count">${list.length}</span>
              </div>
              <div class="profile-group__rows">${rows}</div>
            </div>`;
        })
        .join("");
    }

    const sortNameActive = this.sortMode === "name" ? " profile-sort__btn--active" : "";
    const sortCreatedActive = this.sortMode === "created" ? " profile-sort__btn--active" : "";

    this.container.innerHTML = `
      <div class="profile-selector">
        <div class="profile-selector__head">
          <label>${t("profiles.label")}</label>
          <div class="profile-sort" role="group" aria-label="${t("profiles.sortLabel")}">
            <button class="profile-sort__btn${sortNameActive}" id="profile-sort-name" title="${t("profiles.sortName")}">${t("profiles.sortNameShort")}</button>
            <button class="profile-sort__btn${sortCreatedActive}" id="profile-sort-created" title="${t("profiles.sortCreated")}">${t("profiles.sortCreatedShort")}</button>
          </div>
        </div>
        <div class="profile-tree" id="profile-tree" role="listbox">${treeHtml}</div>
        <div class="profile-mgmt-btns">
          <button class="btn-secondary" id="new-profile-btn">${t("profiles.new")}</button>
          <button class="btn-secondary" id="edit-profile-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.edit")}</button>
          <button class="btn-secondary btn-danger" id="delete-profile-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.delete")}</button>
        </div>
        <button id="connect-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.connect")}</button>
      </div>
    `;

    // Group header collapse/expand toggles
    this.container.querySelectorAll<HTMLElement>(".profile-group__header").forEach((h) => {
      h.addEventListener("click", () => {
        const key = h.dataset.group ?? "";
        if (this.collapsedGroups.has(key)) this.collapsedGroups.delete(key);
        else this.collapsedGroups.add(key);
        this.render();
      });
    });

    // Profile row selection (single click) + connect (double click)
    this.container.querySelectorAll<HTMLElement>(".profile-row").forEach((row) => {
      row.addEventListener("click", () => {
        this.selectedId = row.dataset.id ?? null;
        this.updateRowSelection();
        this.updateButtonStates();
      });
      row.addEventListener("dblclick", () => {
        this.selectedId = row.dataset.id ?? null;
        if (this.selectedId && this.onConnectCallback && !this.isConnected && !this.isConnecting) {
          this.onConnectCallback(this.selectedId);
        }
      });
    });

    // Sort toggle
    document.getElementById("profile-sort-name")?.addEventListener("click", () => {
      if (this.sortMode !== "name") { this.sortMode = "name"; void this.persistSortMode(); this.render(); }
    });
    document.getElementById("profile-sort-created")?.addEventListener("click", () => {
      if (this.sortMode !== "created") { this.sortMode = "created"; void this.persistSortMode(); this.render(); }
    });

    document.getElementById("new-profile-btn")?.addEventListener("click", () => this.onNewCallback?.());
    document.getElementById("edit-profile-btn")?.addEventListener("click", () => {
      const profile = this.getSelectedProfile();
      if (profile) this.onEditCallback?.(profile);
    });
    document.getElementById("delete-profile-btn")?.addEventListener("click", async () => {
      if (this.selectedId) await this.onDeleteCallback?.(this.selectedId);
    });
    document.getElementById("connect-btn")?.addEventListener("click", () => {
      if (this.selectedId && this.onConnectCallback) this.onConnectCallback(this.selectedId);
    });
  }

  /** Toggle the selected-row highlight without a full re-render. */
  private updateRowSelection(): void {
    this.container.querySelectorAll<HTMLElement>(".profile-row").forEach((row) => {
      const sel = row.dataset.id === this.selectedId;
      row.classList.toggle("profile-row--selected", sel);
      row.setAttribute("aria-selected", String(sel));
    });
  }
```

- [ ] **Step 4: Verify `updateButtonStates()` still finds the same IDs**

`updateButtonStates()` and `setConnected()`/`setConnecting()` reference `connect-btn`/`edit-profile-btn`/`delete-profile-btn` — these IDs are preserved in the new render, so those methods work unchanged. Do not modify them. The old `#profile-select` change listener is gone (replaced by row clicks) — confirm no other code references `#profile-select` (grep).

- [ ] **Step 5: Styles**

In `src/styles.css`, add (using existing CSS custom properties for colors; match the look of the existing `.profile-selector`):
- `.profile-selector__head` (flex row, label left, sort toggle right)
- `.profile-sort` + `.profile-sort__btn` + `.profile-sort__btn--active` (small segmented toggle; active uses accent/`var(--accent)` bg)
- `.profile-tree` (scrollable container, `max-height` with `overflow-y:auto`, border using `var(--border)`)
- `.profile-tree__empty` (muted text)
- `.profile-group__header` (clickable, `cursor:pointer`, caret + name + count; subtle hover bg)
- `.profile-group__caret`, `.profile-group__name`, `.profile-group__count` (count muted/small)
- `.profile-group__rows`
- `.profile-row` (clickable row, `cursor:pointer`, padding, hover bg) and `.profile-row--selected` (accent bg/`var(--accent)` + contrasting text), indented under the group header.
Keep it theme-consistent (test both dark + light via the existing `var(--...)` tokens).

- [ ] **Step 6: Compile + revert noise**

Run: `npx tsc 2>&1 | head -20` (0 errors), then `git status --porcelain | grep '\.js$'` and revert unrelated `.js`.

- [ ] **Step 7: Commit**

```bash
git add src/components/profile-selector.ts src/components/profile-selector.js src/styles.css
git commit -m "feat(profiles): grouped collapsible tree selector with persisted sort toggle

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Profile form group field

**Files:**
- Modify: `src/components/profile-form.ts`

- [ ] **Step 1: Add a group input with a datalist of existing groups**

In `mount()`, the form needs access to existing group names for the datalist. `mount` is currently synchronous; fetch groups before mounting. The simplest minimal change: in `show()`, keep it sync but have `mount` read groups lazily — instead, gather existing groups from `api.listProfiles()` inside `mount` is async-unfriendly. APPROACH: add a private field `private existingGroups: string[] = [];`, and in `show(profile?)` make it async-capable:

```typescript
  async show(profile?: Profile): Promise<void> {
    this.editingId = profile?.id ?? null;
    try {
      const all = await api.listProfiles();
      this.existingGroups = [...new Set(all.map((p) => (p.group ?? "").trim()).filter((g) => g.length > 0))].sort();
    } catch {
      this.existingGroups = [];
    }
    this.mount(profile);
  }
```

(Callers in `main.ts` call `profileForm.show(...)` without awaiting — that's fine; it stays fire-and-forget. Verify `show` callers don't depend on a sync return — they don't.)

Add the group form field to the `mount()` template, right after the name field (`#pf-name` block):

```typescript
          <div class="form-field">
            <label for="pf-group">${t("profileForm.labelGroup")}</label>
            <input id="pf-group" type="text" list="pf-group-list" value="${escHtml(profile?.group ?? "")}"
              placeholder="${t("profileForm.placeholderGroup")}" autocomplete="off">
            <datalist id="pf-group-list">
              ${this.existingGroups.map((g) => `<option value="${escHtml(g)}"></option>`).join("")}
            </datalist>
          </div>
```

- [ ] **Step 2: Read the group on save and preserve created_at**

In `handleSave`, read the group value:

```typescript
    const group = get("pf-group");
```

In the constructed `profile` object, add `group` and carry `created_at` forward (so editing never resets it):

```typescript
      group: group || undefined,
      created_at: existingProfile?.created_at ?? undefined,
```

(Place these in the `profile` object literal alongside the other fields. `existingProfile` is already fetched in edit mode; for new profiles it is `null` so `created_at` is `undefined` and the backend stamps it.)

- [ ] **Step 3: Add group to the SSH-import path (optional grouping)**

In `handleSshImport`, the imported `Profile` literal should include the new field so the type is complete and imported hosts are ungrouped by default. Add to that literal:

```typescript
        group: undefined,
```

(Leave `created_at` unset — the backend stamps it on save.)

- [ ] **Step 4: Compile + revert noise + commit**

Run: `npx tsc 2>&1 | head -20` (0 errors).
Revert unrelated `.js`.

```bash
git add src/components/profile-form.ts src/components/profile-form.js
git commit -m "feat(profiles): group field with datalist in profile form; preserve created_at on edit

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: i18n strings (all 6 locales)

**Files:**
- Modify: `src/i18n/{en,de,fr,nl,pl,ru}.ts` (+ regenerate `.js`)

- [ ] **Step 1: Add keys to the `profiles` and `profileForm` sections of every locale**

`profiles` section additions:

| key | en | de | fr | nl | pl | ru |
|---|---|---|---|---|---|---|
| ungrouped | Ungrouped | Ohne Gruppe | Sans groupe | Geen groep | Bez grupy | Без группы |
| sortLabel | Sort profiles | Profile sortieren | Trier les profils | Profielen sorteren | Sortuj profile | Сортировать профили |
| sortName | Sort A–Z | Alphabetisch sortieren | Trier de A à Z | Sorteer A–Z | Sortuj A–Z | Сортировать А–Я |
| sortNameShort | A–Z | A–Z | A–Z | A–Z | A–Z | А–Я |
| sortCreated | Sort by newest | Nach Datum sortieren | Trier par date | Sorteer op datum | Sortuj wg daty | Сортировать по дате |
| sortCreatedShort | Newest | Neueste | Récents | Nieuwste | Najnowsze | Новые |

`profileForm` section additions:

| key | en | de | fr | nl | pl | ru |
|---|---|---|---|---|---|---|
| labelGroup | Group | Gruppe | Groupe | Groep | Grupa | Группа |
| placeholderGroup | Optional group name | Optionaler Gruppenname | Nom de groupe (facultatif) | Optionele groepsnaam | Opcjonalna nazwa grupy | Имя группы (необязательно) |

(Match each file's exact nesting/quote/comma style; add the keys inside the existing `profiles` and `profileForm` objects. Keep `–` as an en-dash where shown.)

- [ ] **Step 2: Compile, revert noise**

Run: `npx tsc 2>&1 | head -10` → 0 errors (if `en.ts` is the source-of-truth type, ALL locales must get the keys). Revert any non-locale `.js`.

- [ ] **Step 3: Commit**

```bash
git add src/i18n/en.ts src/i18n/en.js src/i18n/de.ts src/i18n/de.js src/i18n/fr.ts src/i18n/fr.js src/i18n/nl.ts src/i18n/nl.js src/i18n/pl.ts src/i18n/pl.js src/i18n/ru.ts src/i18n/ru.js
git commit -m "i18n(profiles): group + sort-toggle strings (6 locales)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Full build + Phase 3 review

- [ ] **Step 1: Backend + frontend build**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -5 && cargo clippy 2>&1 | tail -5 && cd .. && npm run build 2>&1 | tail -6`
Expected: all green.

- [ ] **Step 2: Manual verification checklist (by the user)**

1. Profiles render grouped under their group headers; ungrouped profiles under an "Ungrouped" header shown last.
2. Clicking a group header collapses/expands it (caret flips); state holds while the app is open.
3. Clicking a row selects it (highlight); Edit/Delete/Connect enable; double-click connects.
4. Sort toggle A–Z ↔ Newest reorders within each group and persists across app restarts.
5. Editing a profile's Group moves it to that group after save; clearing the group moves it to Ungrouped.
6. A brand-new profile gets a creation date (sort-by-newest puts it on top); legacy profiles still sort sensibly (by file mtime).
7. Connect/Edit/Delete remain gated by connection state (locked while connected).

- [ ] **Step 3: Dispatch the Phase 3 review subagent (Opus)** — spec compliance + code quality + the public-API-preservation check (main.ts still wires correctly).

---

## Task 8: Docs + release (v1.6.0)

New user-facing feature → minor bump **1.5.0 → 1.6.0**. **README update is mandatory.**

**Files:** `README.md`, `CHANGELOG.md`, `CLAUDE.md`, `vault/MurmurSSH Optimization Roadmap.md`, `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.

- [ ] **Step 1: CHANGELOG** — `## [1.6.0]` section: profile groups (grouped collapsible tree selector), per-session collapse, persisted sort toggle (A–Z | Newest), group field in the profile form, `created_at` stamping with mtime backfill for legacy profiles.

- [ ] **Step 2: README (MANDATORY)** — update the Profiles usage section to describe groups + the group field; add a note about the grouped tree selector and the sort toggle; mention it in the features list near the top.

- [ ] **Step 3: CLAUDE.md** — add a Phase 12.0 (v1.6.0) entry under "Phases Complete" (group/created_at fields, save-stamp + mtime backfill, grouped tree selector preserving the public API, sort toggle persisted in Settings, group form field, i18n).

- [ ] **Step 4: Vault** — mark Phase 3 done in `vault/MurmurSSH Optimization Roadmap.md` with the manual-verification checklist.

- [ ] **Step 5: Version bump** — `1.6.0` in the three manifests.

- [ ] **Step 6: Commit, tag, push** (stage ONLY the release files — do NOT sweep unrelated `.gitignore`/`AGENTS.md`; revert `src/i18n/index.js` noise first):

```bash
git add README.md CHANGELOG.md CLAUDE.md "vault/MurmurSSH Optimization Roadmap.md" package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "release: v1.6.0 — profile groups and sorting"
git tag -a v1.6.0 -m "v1.6.0 — profile groups & sorting"
git push origin main --follow-tags
```

- [ ] **Step 7: gitnexus reindex** — `npx gitnexus analyze` (embeddings are 0; no `--embeddings` flag needed).

---

## Self-review notes (author)

- **Spec coverage:** P1 (group + created_at + backward-compat load) → Tasks 1,2; P2 (grouped tree + form group field + sort toggle persisted) → Tasks 3,4,5,6; review → Task 7; docs/release (README mandatory) → Task 8. Covered.
- **Backward compatibility:** legacy profiles deserialize (Task 1 test), and `created_at` backfills from mtime at list time without rewriting the file (Task 2). Sort-by-newest therefore works for old profiles.
- **Public API preserved:** selector keeps init/reload/onConnect/onNew/onEdit/onDelete/getSelectedProfile/setConnected/setConnecting and the button IDs, so `main.ts` (which calls all of these) needs no changes. The only removed surface is the internal `#profile-select` element (replaced by rows) — Task 4 Step 4 greps to confirm nothing else references it.
- **Type consistency:** TS `Profile.group`/`created_at` mirror Rust serde field names (snake_case `created_at`); `Settings.profile_sort` mirrors Rust `profile_sort`. Sort-mode union "name"|"created" consistent across selector, settings, and persistence.
- **`show()` becoming async:** callers use fire-and-forget; verified no caller depends on a return value. If any does, await it there.
