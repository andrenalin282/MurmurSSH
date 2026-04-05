# Cursor Agent Prompt — MurmurSSH Website Content Update (v1.4.x)

You are updating the MurmurSSH project website to reflect the current stable release (v1.4.1).
Update content only — do not change layout, structure, styling, or assets unless explicitly instructed.

---

## Step 1 — Scan the project

Before making any changes:

1. List all files in the website project (HTML, Markdown, MDX, or similar content files).
2. Identify which files contain user-visible text about: version numbers, feature lists,
   installation instructions, screenshots, and release status.
3. Note every occurrence of old version strings (v1.3.x or earlier).

Do not edit anything yet.

---

## Step 2 — Update version strings

In every identified file:

- Replace any version string below v1.4.1 with **v1.4.1**.
- Update "latest release" badges or links to point to v1.4.1.
- If a download link includes a version number, update it to `murmurssh_1.4.1_amd64.deb`
  and `MurmurSSH_1.4.1_amd64.AppImage` (or the equivalent glob patterns).

---

## Step 3 — Update the feature list

Replace or extend any feature list to include the following. Use this as the authoritative source.

**Current feature set (v1.4.1):**

- SSH terminal sessions — key authentication, SSH agent, and password auth
- SFTP and FTP file browser — full file management (upload, download, rename, move, delete, new file/folder)
- **Split-pane layout** — local file browser alongside the remote browser; toggle on/off with toolbar button
- **Local file browser** — navigate local filesystem, drag files to remote to upload; position (left/right) configurable in Settings
- **Drag and drop** — drag files from local browser or OS file manager onto remote browser to upload; drag remote entries onto local browser to download
- **Download drop zone** — dashed drop area below action bar accepts dragged remote rows
- **Keyboard shortcuts** — F5 refresh, F2 rename, F11 terminal, Delete, Ctrl+A, Enter, Escape
- Real-time transfer progress with speed display (MB/s) and cancel button
- Multi-selection with Ctrl/Shift click; batch delete, download, and move
- Drag-and-drop move within the remote browser (onto folder rows or "..")
- Upload overwrite dialog (Yes / No / Cancel + Apply to all)
- Recursive folder upload, download, and delete
- Remote file editing — open in local editor, auto-upload or confirm on save
- Activity log panel (live connection and transfer events)
- Profile management — create, edit, delete, persist as local JSON; import from ~/.ssh/config
- Multilanguage UI — English and German; switchable in Settings
- Credential storage tiers: never / save locally (0600 file) / save in profile (portable)
- Host key verification — accept once or save permanently
- Per-profile settings: local path, remote path, editor command, upload mode
- **Local browser position** — choose left or right in Settings
- Per-user local path persistence in shared/portable profiles
- Dark, light, and system theme; SSH key runtime copy for key compatibility
- No cloud services, no telemetry — all data stays on the user's machine

**Suggested short tagline (if the site has one):**
> "A minimal, local-first SSH, SFTP, and FTP client for Linux. No cloud, no telemetry."

---

## Step 4 — Update the primary screenshot (if applicable)

If the site has an application screenshot in the hero or features section:

- Update the `alt` text to: `"MurmurSSH v1.4.1 — split-pane file browser"`
- If a new screenshot is available at `murmurssh-home-screen.png`, replace the `src` reference.
- Do not change image dimensions, CSS classes, or surrounding layout.

---

## Step 5 — Installation instructions

Ensure both install paths are accurate:

### .deb
```bash
sudo dpkg -i murmurssh_*.deb
```

### AppImage
```bash
chmod +x MurmurSSH_*.AppImage
./MurmurSSH_*.AppImage
```

The AppImage bundles all dependencies. A Wayland compatibility fix (GPU renderer) is applied
automatically — no manual configuration is required.

Runtime dependencies for `.deb`: `libwebkit2gtk-4.1-0`, `libgtk-3-0`.

---

## Step 6 — What's new in v1.4.x section

If the site has a "What's new" or "Changelog highlights" section, add or replace with:

### v1.4.0 — Local file browser & keyboard shortcuts

**Local file browser**
A second browser panel now shows your local filesystem alongside the remote file list —
similar to the classic FileZilla split-pane layout. Toggle it with the new toolbar icon.
When the panel is hidden, the icon is highlighted so you always know the state.

**Drag and drop downloads**
Drag remote files or folders from the right panel onto the local browser to download them
directly to your current local directory. A "Drop here to download" zone below the action
bar also accepts drops.

**Keyboard shortcuts**
Common operations are now accessible without the mouse: F5 to refresh, F2 to rename,
F11 for the terminal, Delete to delete, Ctrl+A to select all, Enter to open, Escape to clear.
Full list in the Help dialog.

**Panel position setting**
The local browser panel can be placed on the left or right side of the remote browser —
change it any time in Settings → Local browser position.

---

## Step 7 — Final review checklist

Before finishing, verify:

- [ ] All displayed version numbers show v1.4.1
- [ ] Feature list includes the local file browser and keyboard shortcuts
- [ ] Drag-and-drop (both directions) is mentioned
- [ ] "What's new" or highlights section reflects v1.4.x if present
- [ ] Install instructions for `.deb` and AppImage are correct
- [ ] No layout, CSS, or structural changes were made
- [ ] No assets were deleted
- [ ] No links to external services were added

---

## Constraints

- Modify content only — do not touch layout, CSS, component structure, or routing.
- Do not add new pages or sections not already present in the site.
- Do not fabricate features, benchmarks, or comparisons not provided above.
- Keep tone professional and clear — this is open-source project documentation.
- If you are unsure whether a change is within scope, leave the content as-is and flag it.
