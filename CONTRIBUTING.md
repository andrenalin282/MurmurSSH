# Contributing to MurmurSSH

Thank you for your interest in contributing. MurmurSSH is a small open-source project and all contributions are welcome — bug reports, fixes, documentation improvements, or platform testing.

---

## Before You Start

- Read `PRD.md` to understand what this project is and is not.
- Check existing [Issues](../../issues) to avoid duplicate work.
- For any non-trivial change, **open an issue first** to discuss the approach.

---

## How to Contribute

### Reporting Bugs

Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.yml).
Include steps to reproduce, expected vs. actual behavior, and your system info.

### Requesting Features

Use the [feature request template](.github/ISSUE_TEMPLATE/feature_request.yml).
Explain the problem you're solving, not just the solution.

### Pull Requests

- Keep pull requests **focused and small** — one concern per PR.
- Base your branch on `main`.
- Follow the commit convention below.
- Describe what you changed and why in the PR description.
- If your change touches a UI flow, briefly describe how you tested it.

---

## Development Setup

You need [Rust](https://rustup.rs) (stable toolchain) and Node.js 18+.

```bash
git clone https://github.com/andrenalin282/MurmurSSH.git
cd MurmurSSH
npm install
npm run tauri dev
```

System dependencies (Ubuntu/Debian):

```bash
sudo apt install \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssh2-1-dev
```

---

## Commit Convention

Use short, English commit messages with a lowercase type prefix:

| Prefix | When to use |
|--------|-------------|
| `feat:` | New user-visible functionality |
| `fix:` | Bug fix |
| `docs:` | Documentation only |
| `refactor:` | Internal improvement, no behavior change |
| `chore:` | Maintenance, repo housekeeping |
| `build:` | Build or packaging changes |
| `ci:` | CI/CD workflow changes |
| `test:` | Tests only |

**Format:** `<type>: <short description>`

Good examples:
```
feat: add rename file action to browser toolbar
fix: prevent duplicate connect when button clicked twice
docs: update contributing guide with system dependencies
```

Avoid: `update`, `fixes`, `stuff`, `wip`, mixed-topic commits.

---

## Architecture Notes

- Frontend: Vanilla TypeScript, no framework. Components in `src/components/`.
- Backend: Rust via Tauri 2. Services in `src-tauri/src/services/`, commands in `src-tauri/src/commands/`.
- Profiles are plain JSON files in `~/.config/murmurssh/profiles/`.
- SSH key passphrases are never saved to disk under any circumstance.
- Keep business logic in services, not in UI components or Tauri command handlers.

---

## Code Style

- Keep it simple and readable over clever.
- Match the existing style in the file you are editing.
- Avoid adding new dependencies without discussion.
- No embedded database — JSON files are the persistence layer.
