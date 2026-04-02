# Cursor Agent Prompt — MurmurSSH Website Content Update

You are updating the MurmurSSH project website to reflect the current stable release (v1.1.0).
The site previously described an early beta (~v0.14). Your task is to update the content only —
do not change layout, structure, styling, or assets unless explicitly instructed below.

---

## Step 1 — Scan the project

Before making any changes:

1. List all files in the website project (HTML, Markdown, MDX, or similar content files).
2. Identify which files contain user-visible text about: version numbers, feature lists,
   installation instructions, screenshots, and release status.
3. Note every occurrence of beta language: "beta", "early beta", "experimental", "pre-release",
   "work in progress", or version strings below v1.0.

Do not edit anything yet. Build a list of files and locations that need changes.

---

## Step 2 — Replace outdated release status language

In every file identified above:

- Remove or reword any phrase that describes MurmurSSH as beta, experimental, or pre-release.
- Replace with language that presents MurmurSSH as a **stable, open-source SSH and SFTP client
  for Linux**.
- Update any displayed version number to **v1.1.0**.
- If a "latest release" badge or link exists, update it to point to v1.1.0.

Examples of replacements:
| Old | New |
|-----|-----|
| "early beta" | "stable release" |
| "experimental SSH client" | "SSH and SFTP client for Linux" |
| "v0.1.x" / "0.14" / any pre-1.0 version | "v1.1.0" |
| "not yet ready for production" | _(remove entirely)_ |

---

## Step 3 — Update feature descriptions

Replace or rewrite any feature list or feature section to accurately reflect what MurmurSSH
currently does. Use the list below as the authoritative source. Do not invent features that are
not in this list.

**Features to include:**

- SSH terminal sessions with support for key authentication, SSH agent, and password auth
- SFTP file browser with full file management (upload, download, rename, delete, move)
- Multi-selection of files and folders
- Drag-and-drop move operations within the file browser
- Upload and download for both individual files and entire folders (recursive)
- Transfer progress indicators with cancel support
- Overwrite protection dialog on upload (with apply-to-all option)
- Editable path bar for direct navigation
- Breadcrumb-style path display
- Profile management — create, edit, delete, and persist SSH connection profiles
- Import profiles from `~/.ssh/config`
- Multilanguage UI — English and German; language switchable in Settings
- Icon-based interface with context menus for file operations (right-click)
- Credential storage tiers: never save / save locally / save in profile
- Host key verification with accept-once or accept-and-save options
- Per-profile settings: local path, remote path, editor command, upload mode
- No cloud services, no telemetry — all data stays on the user's machine

If the site has a short tagline or description, a suitable replacement is:
> "A minimal, local-first SSH and SFTP client for Linux. No cloud, no telemetry."

---

## Step 4 — Update the primary screenshot

A file named `murmurssh-home-screen.png` is available.

1. Find where the current application screenshot is displayed (likely the hero section or a
   features/overview section).
2. Replace the existing screenshot reference with `murmurssh-home-screen.png`.
3. Update the `alt` text to: `"MurmurSSH v1.1.0 — SFTP file browser"` (or similar accurate description).
4. Do not change the image dimensions, CSS classes, or surrounding layout — only the `src`/path
   and `alt` attributes.

If no screenshot currently exists, add `murmurssh-home-screen.png` in the most prominent
content area (hero or feature overview), using the same styling conventions as other images on
the page.

---

## Step 5 — Update Linux compatibility and installation instructions

### AppImage section

Locate any AppImage installation instructions and update them to:

1. State that the AppImage is the portable install option — no installation required.
2. Include these accurate instructions:
   ```
   chmod +x MurmurSSH_*.AppImage
   ./MurmurSSH_*.AppImage
   ```
3. Add a short compatibility note (use plain language, no low-level detail):
   > **Wayland compatibility:** MurmurSSH v1.1.0 includes a fix for GPU rendering issues
   > on Wayland-based systems (e.g., Arch Linux with a Wayland compositor). No manual
   > configuration is required — the AppImage handles this automatically.
   > X11 systems are unaffected.

### .deb section

Ensure the `.deb` instructions are present and accurate:
```
sudo dpkg -i murmurssh_*.deb
```

### Remove obsolete steps

Delete any setup steps that reference:
- Manual environment variable configuration
- Workarounds the user previously had to apply manually
- Beta disclaimers in the install section
- Any dependency list that no longer matches (current runtime deps: `libwebkit2gtk-4.1-0`, `libgtk-3-0`)

---

## Step 6 — Documentation pages (if present)

If the site includes separate documentation pages (e.g., a "Getting Started", "FAQ", or
"Configuration" page):

1. Apply the same version and status language updates from Steps 2–3.
2. Update any feature references to match the current feature list.
3. If a "Known Limitations" or "Known Issues" section exists, you may keep it but remove
   items that have been resolved (Wayland/AppImage crash is now fixed in v1.1.0).
4. Do not rewrite entire pages — only correct what is factually outdated.

---

## Step 7 — Final review checklist

Before finishing, verify:

- [ ] No remaining occurrences of "beta", "experimental", "pre-release" in user-visible text
- [ ] Version shown everywhere is v1.1.0
- [ ] Feature list matches Step 3 — no invented or missing features
- [ ] Screenshot points to `murmurssh-home-screen.png`
- [ ] AppImage instructions include the Wayland compatibility note
- [ ] `.deb` instructions are present and correct
- [ ] No layout, CSS, or structural changes were made
- [ ] No assets were deleted
- [ ] No links to external services were added

---

## Constraints

- Modify content only — do not touch layout, CSS, component structure, or routing.
- Do not add new pages or sections not already present in the site.
- Do not delete any image or asset files.
- Do not fabricate features, benchmarks, or comparisons not provided above.
- Keep tone professional and clear — this is open-source project documentation, not a
  marketing page.
- If you are unsure whether a change is within scope, leave the content as-is and add a
  comment flagging it for review.
