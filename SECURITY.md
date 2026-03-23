# Security Policy

## Scope

This policy applies to the MurmurSSH application and its source code.

---

## Supported Versions

Only the latest release receives security fixes.

| Version | Supported |
|---------|-----------|
| latest  | Yes       |
| older   | No        |

---

## Reporting a Vulnerability

If you discover a security vulnerability, please **do not open a public GitHub issue**.

Instead, report it by opening a [GitHub Security Advisory](../../security/advisories/new) in this repository. This keeps the details private until a fix is available.

If you are unsure whether something qualifies as a security issue, err on the side of reporting it privately.

Please include:
- A clear description of the vulnerability
- Steps to reproduce, if applicable
- The potential impact you see
- Your suggested fix, if you have one (optional)

We will acknowledge your report as soon as possible and work to address it promptly.

---

## Security Notes

- All profile data, credentials, and settings are stored **locally only**. Nothing is sent to any remote server by MurmurSSH itself.
- SSH key passphrases are **never** written to disk. They are prompted at connection time and discarded after use.
- Saved passwords (when the user opts in) are stored as **plaintext** — either in a local file with `0600` permissions, or inside the profile JSON. Neither option provides encryption. Users who require stronger protection should not use the save-password feature.
- Host key fingerprints are verified before connecting. Users are warned about unknown or changed host keys.
- MurmurSSH uses your system's SSH tooling (`ssh`, `ssh-agent`) for terminal sessions. The security of those sessions depends on your SSH configuration.
