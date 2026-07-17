# GitHub Release Update Check — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let MurmurSSH check GitHub Releases for a newer version on startup (opt-out, default on) and via a Settings “Check now” button, showing a dialog with a link to Releases when an update exists.

**Architecture:** A Rust `update_service` calls `GET https://api.github.com/repos/andrenalin282/MurmurSSH/releases/latest` (via `ureq`), strips an optional leading `v` from `tag_name`, compares semver triples to `app.package_info().version`, and returns an `UpdateCheckResult`. Frontend reuses that command for startup (silent unless update) and Settings (always status feedback). No auto-download.

**Tech Stack:** Rust (Tauri 2, `ureq`, `serde`), vanilla TypeScript frontend, six-locale i18n, `settings.json` persistence.

**Spec:** `docs/superpowers/specs/2026-07-17-github-update-check-design.md`

## Global Constraints

- Notify-only: dialog + `open_url(release_url)`; no download/install.
- Startup check default **on** (`check_updates_on_startup` absent/`None` ⇒ `true`).
- Startup: dialog only if `update_available`; errors and “up to date” are silent.
- Manual check: always update status line; dialog if update found; no parallel checks.
- Endpoint: `https://api.github.com/repos/andrenalin282/MurmurSSH/releases/latest` only.
- Ship as **v1.7.2** (version bump + tag + push; CI creates draft release).
- Update README + CHANGELOG before release push.
- Follow existing patterns: one concern per service/command file; `Option` settings with serde skip; i18n keys in all of `en,de,fr,nl,pl,ru`.

---

## File Structure

**Backend (create):**
- `src-tauri/src/services/update_service.rs` — HTTP fetch, tag parse, semver compare, `check_latest`.
- `src-tauri/src/commands/update.rs` — thin `check_for_updates` command.
- `src-tauri/src/models/update.rs` — `UpdateCheckResult` DTO.

**Backend (modify):**
- `src-tauri/Cargo.toml` — add `ureq`
- `src-tauri/src/services/mod.rs`, `commands/mod.rs`, `models/mod.rs`, `models/settings.rs`, `lib.rs`

**Frontend (modify):**
- `src/types.ts`, `src/api/index.ts`, `src/components/settings-dialog.ts`, `src/main.ts`
- Create: `src/components/update-dialog.ts`
- `src/i18n/{en,de,fr,nl,pl,ru}.ts`
- `README.md`, `CHANGELOG.md`
- Version files → `1.7.2`: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`

---

### Task 1: Settings field + UpdateCheckResult types

**Files:**
- Modify: `src-tauri/src/models/settings.rs`
- Create: `src-tauri/src/models/update.rs`
- Modify: `src-tauri/src/models/mod.rs`
- Modify: `src/types.ts`

**Interfaces:**
- Produces: `Settings.check_updates_on_startup: Option<bool>`; `UpdateCheckResult { update_available, current_version, latest_version, release_url }`

- [ ] **Step 1: Add Rust settings field** after `editor_by_extension`:

```rust
    /// When true (default), check GitHub Releases once after app start.
    /// Absent/None = true (opt-out).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_updates_on_startup: Option<bool>,
```

- [ ] **Step 2: Create `src-tauri/src/models/update.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
}
```

Register: `pub mod update;` in `models/mod.rs`.

- [ ] **Step 3: TypeScript mirrors** in `src/types.ts`:

```ts
  check_updates_on_startup?: boolean | null;
```

```ts
export interface UpdateCheckResult {
  update_available: boolean;
  current_version: string;
  latest_version: string;
  release_url: string;
}
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/models/settings.rs src-tauri/src/models/update.rs src-tauri/src/models/mod.rs src/types.ts
git commit -m "feat(settings): add check_updates_on_startup and UpdateCheckResult"
```

---

### Task 2: `update_service` pure helpers (TDD)

**Files:**
- Create: `src-tauri/src/services/update_service.rs`
- Modify: `src-tauri/src/services/mod.rs`

**Interfaces:**
- Produces: `strip_v_prefix`, `parse_semver`, `is_newer`

- [ ] **Step 1: Register** `pub mod update_service;`

- [ ] **Step 2: Implement helpers + tests**

```rust
pub fn strip_v_prefix(tag: &str) -> &str {
    tag.strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag)
}

pub fn parse_semver(s: &str) -> Result<(u64, u64, u64), String> {
    let core = s.split(&['-', '+'][..]).next().unwrap_or(s).trim();
    let mut parts = core.split('.');
    let major = parts.next().ok_or_else(|| format!("invalid version: {s}"))?
        .parse::<u64>().map_err(|_| format!("invalid version: {s}"))?;
    let minor = parts.next().ok_or_else(|| format!("invalid version: {s}"))?
        .parse::<u64>().map_err(|_| format!("invalid version: {s}"))?;
    let patch = parts.next().ok_or_else(|| format!("invalid version: {s}"))?
        .parse::<u64>().map_err(|_| format!("invalid version: {s}"))?;
    Ok((major, minor, patch))
}

pub fn is_newer(latest: &str, current: &str) -> Result<bool, String> {
    let l = parse_semver(strip_v_prefix(latest))?;
    let c = parse_semver(strip_v_prefix(current))?;
    Ok(l > c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_v() {
        assert_eq!(strip_v_prefix("v1.7.2"), "1.7.2");
        assert_eq!(strip_v_prefix("1.7.2"), "1.7.2");
    }

    #[test]
    fn compare_newer_equal_older() {
        assert_eq!(is_newer("v1.8.0", "1.7.2").unwrap(), true);
        assert_eq!(is_newer("1.7.2", "1.7.2").unwrap(), false);
        assert_eq!(is_newer("1.7.1", "1.7.2").unwrap(), false);
    }

    #[test]
    fn reject_bad_tag() {
        assert!(is_newer("nope", "1.0.0").is_err());
    }
}
```

- [ ] **Step 3: Run** `cd src-tauri && cargo test update_service -- --nocapture` — expect PASS

- [ ] **Step 4: Commit** `feat(update): add semver helpers for GitHub release check`

---

### Task 3: HTTP fetch + Tauri command

**Files:**
- Modify: `src-tauri/Cargo.toml` — `ureq = "2"`
- Modify: `src-tauri/src/services/update_service.rs`
- Create: `src-tauri/src/commands/update.rs`
- Modify: `commands/mod.rs`, `lib.rs`

**Interfaces:**
- Produces: `check_latest(current: &str) -> Result<UpdateCheckResult, String>`; command `check_for_updates`

- [ ] **Step 1: Add** `ureq = "2"` under `[dependencies]`

- [ ] **Step 2: Implement `check_latest`** — GET releases/latest, User-Agent `MurmurSSH/{version}`, Accept `application/vnd.github+json`, 8s timeout, parse `tag_name`/`html_url`, return `UpdateCheckResult`.

- [ ] **Step 3: Command**

```rust
#[tauri::command]
pub fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateCheckResult, String> {
    let current = app.package_info().version.to_string();
    update_service::check_latest(&current)
}
```

Register in `lib.rs` near `get_app_version`.

- [ ] **Step 4: Verify** `cargo test update_service && cargo build`

- [ ] **Step 5: Commit** `feat(update): check latest GitHub release via ureq`

---

### Task 4: Frontend API + i18n

**Files:** `src/api/index.ts`, `src/i18n/{en,de,fr,nl,pl,ru}.ts`

- [ ] **Step 1:** `export async function checkForUpdates(): Promise<UpdateCheckResult>`

- [ ] **Step 2: en.ts keys**

```ts
checkUpdatesOnStartup: "Check for updates on startup",
checkUpdatesNow: "Check now",
updateStatusChecking: "Checking…",
updateStatusUpToDate: "You're up to date ({version})",
updateStatusAvailable: "Version {version} is available",
updateStatusError: "Could not check for updates",
```

```ts
updateAvailableTitle: "Update available",
updateAvailableBody: "MurmurSSH <strong>{latest}</strong> is published. You are on <strong>{current}</strong>.",
updateOpenReleases: "Open releases",
updateLater: "Later",
```

- [ ] **Step 3: Translate** same keys in de/fr/nl/pl/ru

- [ ] **Step 4: Commit** `feat(update): API wrapper and i18n for update check`

---

### Task 5: Settings UI + update dialog

**Files:**
- Create: `src/components/update-dialog.ts`
- Modify: `src/components/settings-dialog.ts`

- [ ] **Step 1: `showUpdateAvailableDialog(result)`** — modal with Later + Open releases (`api.openUrl`)

- [ ] **Step 2: Settings “Updates” section** — checkbox (`check_updates_on_startup !== false`), Check now button, status `#update-check-status`, in-flight guard

- [ ] **Step 3: Persist** checkbox on Apply

- [ ] **Step 4: Commit** `feat(settings): updates section with check-now and dialog`

---

### Task 6: Startup check

**Files:** `src/main.ts`

- [ ] **Step 1:** After init settings load, if `check_updates_on_startup !== false`, call `checkForUpdates()`; on `update_available` show dialog; swallow errors.

- [ ] **Step 2:** `npx tsc --noEmit` — expect clean

- [ ] **Step 3: Commit** `feat(update): check GitHub releases on startup when enabled`

---

### Task 7: README + CHANGELOG

- [ ] **Step 1: README** bullet under “What it does” for update check (startup toggle + Check now + Releases link, no auto-install)

- [ ] **Step 2: CHANGELOG** `## [1.7.2] - 2026-07-17` Added section

- [ ] **Step 3: Commit** `docs: README and CHANGELOG for GitHub update check`

---

### Task 8: Production review + release v1.7.2

- [ ] **Step 1: Verify** `cargo test update_service && cargo build` + `npx tsc --noEmit`

- [ ] **Step 2: Code review** branch vs pre-feature; fix Critical/Important; re-verify

- [ ] **Step 3: Bump** `1.7.1` → `1.7.2` in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`; commit `release: v1.7.2 — GitHub update check`

- [ ] **Step 4: Tag + push**

```bash
git tag -a v1.7.2 -m "MurmurSSH v1.7.2"
git push origin main
git push origin v1.7.2
```

CI creates a draft release with .deb/.AppImage. Publish when ready: `gh release edit v1.7.2 --draft=false` (after assets appear).

- [ ] **Step 5: Confirm** `gh release view v1.7.2`

---

## Spec coverage

| Spec requirement | Task |
|------------------|------|
| Default-on startup setting | 1, 5, 6 |
| Check now + status | 5 |
| Update dialog + open releases | 5 |
| Silent startup unless update | 6 |
| SemVer / strip v | 2 |
| GitHub latest API | 3 |
| i18n ×6 | 4 |
| README + CHANGELOG | 7 |
| v1.7.2 release | 8 |
