# AI access (MCP) — test version

MurmurSSH can let an AI client (Claude Code or any other MCP client) use your saved
SSH/SFTP profiles: list folders, read files, upload/download files and folders,
create/rename, delete and run commands. **Nothing works without a token created in
the app, and deleting or running commands always needs your click in MurmurSSH.**

## How it works

```
AI client ──stdio──▶ murmurssh --mcp ──Unix socket──▶ running MurmurSSH app ──SFTP/SSH──▶ server
                    (thin bridge, holds          (checks token, permissions,
                     only the token)              paths; asks you to confirm)
```

- The socket lives at `$XDG_RUNTIME_DIR/murmurssh/ai.sock` (folder `0700`, socket `0600`).
  There is no TCP port, so web pages cannot reach it.
- Passwords, passphrases and keys never leave the app. Password profiles work when the
  password is saved or you connected the profile once in this session.
- Unknown host keys are never accepted for the AI — connect the profile once yourself.

## Set up

1. Settings → **AI access** → switch **AI access on**.
2. **New token**: name, allowed profiles, permissions, optional remote folder
   (the AI stays inside it), local folder (needed for upload/download), expiry.
3. Copy the secret (shown once) or the ready command:
   ```bash
   claude mcp add -s user murmurssh -e MURMURSSH_TOKEN=mssh_… -- /usr/bin/murmurssh --mcp
   ```
   Use user scope (`-s user`): a project-scoped `.mcp.json` may end up in git.
4. Keep MurmurSSH running while the AI works.

## Permissions per token

| Permission | Tools | Confirmation in MurmurSSH |
|---|---|---|
| read | `murmur_list_dir`, `murmur_read_file` (≤ 512 KB text) | no |
| download | `murmur_download` (file or folder into the local folder) | overwrite always; new files when “confirm writes” |
| upload | `murmur_upload` (file or folder from the local folder) | overwrite always; otherwise when “confirm writes” |
| write | `murmur_mkdir`, `murmur_rename` | when “confirm writes” |
| delete | `murmur_delete` (files, folders recursively, symlinks only as link) | **always** |
| exec | `murmur_exec` (one line, 60 s, output ≤ 256 KB) | **always** (unless ssh_direct) |
| ⚠ ssh_direct | `murmur_ssh_command`; `murmur_list_profiles` adds `auth_type` + `key_path`; `murmur_exec` without click | **never** — enabling it in the token form needs an extra warned confirmation |

### ssh_direct (“Read profiles & start SSH on its own”)

Off by default. When on, the AI may open SSH sessions by itself:

- `murmur_ssh_command` returns a ready-to-run `ssh` argv / shell string for the AI's own
  shell. It pins the host key MurmurSSH already trusts (private known_hosts in
  `~/.config/murmurssh/run/ai-<profile>.known_hosts`, removed when AI access stops),
  uses `BatchMode=yes` (never prompts) and the same hardening as the terminal launch
  (no agent/X11 forwarding, no LocalCommand).
- Key profiles: the key path is used directly; a key with passphrase only works if it
  is loaded in ssh-agent. Agent profiles use ssh-agent. Password profiles reuse a
  MurmurSSH ControlMaster (started on demand from a saved password); the password
  itself is never handed out, and the session ends when MurmurSSH disconnects/exits.
- `murmur_exec` runs without a click (still one line, 60 s, audited).
- It does **not** grant delete/upload/write via the MurmurSSH tools.

What the AI does inside its own SSH session is invisible to MurmurSSH: no audit, no
remote-root limit. This equals giving the AI tool the key on this machine — enable it
only for trusted servers and tools.

`murmur_list_profiles` shows the profiles and permissions of the token, per profile the
effective local folder (`transfers_usable`) and hints (`notes`).

Token options:

- **All profiles (including future ones)**: the token applies to every SSH/SFTP profile,
  also ones created later — no need to edit the token.
- **Allow everything**: button in the token form that ticks all permissions; “SSH on its
  own” still asks for its extra warning. “Confirm uploads and writes” stays as it is.
- **Local folders**: relative local paths go to the token's local folder, or — if the
  token has none — to the token's own **staging folder**
  `~/.local/share/murmurssh/ai-staging/<token-id>` (0700, deleted when the token is
  revoked). Absolute paths may also point into the staging folder and, without a token
  folder, into the profile's local folder (the folder last used in the local browser
  panel). `/` and your home folder (or above) are always refused.
- Remote paths may be absolute, `~`, `~/…` or relative (resolved against the login
  directory on the server); the remote-root check applies after resolution.

Rules that always apply:

- Remote paths must lie in the token’s remote folder, also after the server resolves
  symlinks. The remote folder itself and `/` can never be deleted or moved.
- Local paths must lie in the token’s local folder (not `/`, not your home folder
  itself). `.git`, `.ssh`, `.gnupg` are never read or written.
- Folder transfers do not follow symlinks; skipped entries are reported.
- Folders are only uploaded/downloaded to a new target path (no merging).
- Commands are **not** limited to the remote folder — the dialog says so.
- A confirmation request waits **90 s** (below Claude Code's 120 s tool timeout). The AI
  gets distinct answers: **denied by the user** (final, do not retry) vs. **timed out**
  (nobody clicked; may be retried). Each waiting request restores a minimised MurmurSSH
  window, flashes it, marks its title (“⚠ AI confirmation waiting”) and sends a desktop
  notification (`notify-send`, critical); the dialog shows the seconds left and the dock
  icon carries a badge with the number of waiting requests (Ubuntu dock). The timeout
  answer says whether the dialog was actually **painted** (the GUI reports it after the
  first animation frame) — “not displayed” points at a minimised/hidden window or an app
  that was still starting. An expired request leaves a normal desktop notification.
- With several MurmurSSH windows open, only the **first** one serves AI access — its
  window shows the dialogs.
- **Always allow**: the confirmation dialog can turn an approval into a standing
  approval for this token + profile — for this action type or for everything except
  delete (delete only via its own tick), valid 1 hour, until MurmurSSH restarts, or
  permanently — delete never permanently (1 hour or until restart only). Standing approvals are listed under the token in Settings → AI access and
  can be revoked there; `murmur_list_profiles` shows them as `always_allowed`.
- With ssh_direct, file-tool actions (delete; writes when “confirm writes” is on) still ask —
  as a guard against mistakes, not as a security boundary, since the AI can do the same
  through its own SSH session.
- Commands run without a pty: `stdout` and `stderr` are separate, line endings are `\n`.
  A command that hits the 60 s limit may keep running on the server.
- `murmur_list_profiles` is compact (id, name, `user@host`, port if not 22, remote path,
  local folder, `always_allowed` — always present, `[]` when none; no key paths) and
  takes an optional `query` filter (id/name/host substring).
  Transfer answers report `files`, `dirs` and `bytes`.
- The Approve button is
  enabled after 1.5 s and only reacts to a mouse click (not Enter/Space), so typing
  in another window cannot approve anything. MurmurSSH does not steal focus.
- Revoking a token or switching AI access off also denies a dialog that is still open.
- Every request is logged to `~/.config/murmurssh/ai_audit.jsonl` (`0600`; rotated at
  5 MB to `.1`). It contains paths and commands, never secrets or file contents.

## Limits of this protection (read this)

The AI client runs as **your** user. If it can also run arbitrary shell commands
(as Claude Code can), it could read files MurmurSSH can read — e.g. saved passwords in
`~/.config/murmurssh/secrets/` — and use `ssh` directly, bypassing MurmurSSH.
MurmurSSH only reads `ai_access.json` at start, so editing it at runtime grants
nothing, but a restart would load it.

Recommended for Claude Code (`~/.claude/settings.json`):

```json
{
  "permissions": {
    "deny": [
      "Read(~/.config/murmurssh/**)",
      "Edit(~/.config/murmurssh/**)",
      "Write(~/.config/murmurssh/**)"
    ]
  }
}
```

and do not auto-approve shell commands for sessions that hold a MurmurSSH token. For a
hard boundary run the AI client in a sandbox that cannot read `~/.config/murmurssh`.

## Local test

A throw-away SSH server: `docker run -d --name murmur-sshd -p 127.0.0.1:2222:2222 -e
PUID=$(id -u) -e PGID=$(id -g) -e USER_NAME=murmur -e PUBLIC_KEY="$(cat ~/.ssh/id_ed25519.pub)"
lscr.io/linuxserver/openssh-server` → profile `127.0.0.1:2222`, user `murmur`, key auth.
Inspect the tools with `npx @modelcontextprotocol/inspector` (command `murmurssh`,
argument `--mcp`, env `MURMURSSH_TOKEN`).
