You are acting as a senior Rust/Tauri engineer and security-focused software architect.

We are continuing development of the Linux desktop application **MurmurSSH**.

The project is fully implemented through Phase 3 and currently in a functional alpha state.

Your task is to implement **Phase 4 — Robustness, Security, and Authentication Improvements**.

This phase MUST strengthen the existing system without expanding product scope or altering architecture.

---

# CRITICAL RULES

## DO NOT

- Do NOT redesign the UI
- Do NOT introduce new major features
- Do NOT add cloud capabilities
- Do NOT add a database
- Do NOT embed a terminal
- Do NOT implement port forwarding
- Do NOT implement multi-session UI
- Do NOT store passwords persistently
- Do NOT break existing profile format compatibility
- Do NOT introduce heavy dependencies

## MUST PRESERVE

- Minimal philosophy
- Local-only configuration
- Open-source friendliness
- Linux-first design
- .deb compatibility
- Existing architecture (services → commands → UI)
- Vanilla TypeScript frontend
- Tauri 2 + Rust backend
- System terminal for SSH

---

# PROJECT CONTEXT

MurmurSSH is a minimal SSH + SFTP client for Linux (Ubuntu), similar to Bitvise or Termius but intentionally simple.

Implemented features:

- Profile management via GUI
- SSH launch using system terminal
- SFTP browsing
- Upload/download
- Remote editing via local workspace
- File watchers with upload-on-save
- JSON-based profile storage
- No cloud features
- No database

Profiles stored under:

~/.config/murmurssh/profiles/*.json

Workspace stored under:

~/.config/murmurssh/workspace/

---

# PHASE 4 OBJECTIVES

Focus strictly on:

1) Authentication improvements  
2) Security baseline improvements  
3) Connection robustness  
4) Workspace stability  
5) Real-world usability fixes  

This phase should make the application safe and reliable for everyday use.

---

# REQUIRED IMPLEMENTATIONS

## 1. Multiple Authentication Types

Add support for:

- SSH Key authentication (existing)
- Password authentication (NEW)

### Profile Changes

Profiles must include:

auth_type: "ssh_key" | "password"

Rules:

- Existing profiles default to "ssh_key"
- Passwords MUST NOT be stored in profile files
- Private key path required only for ssh_key type

### UI Changes

Profile form must include an authentication dropdown.

When auth_type = password:

- Hide or disable key path field
- Do not require key path validation

---

## 2. Runtime Password Prompt

When connecting using password authentication:

- Prompt user for password at connection time
- Use in-app modal dialog (NOT browser prompt)
- Store password in memory only for current operation
- Never write password to disk
- Clear password after use

---

## 3. SSH Key Passphrase Support

Support encrypted private keys.

Behavior:

- Detect when key requires passphrase
- Prompt user at connection time
- Use session-only memory
- Do NOT store passphrase in profile
- Do NOT cache across sessions

---

## 4. Host Key Verification (Known Hosts)

Implement a safe baseline similar to SSH known_hosts.

### Requirements

Before establishing SSH/SFTP connection:

- Retrieve server host key fingerprint
- Check against local known hosts store
- If unknown:

  - Show warning dialog
  - Display fingerprint
  - Allow user to Accept or Reject

If accepted:

- Save host entry locally

If rejected:

- Abort connection

### Storage

Store accepted host keys locally under:

~/.config/murmurssh/known_hosts

Format may be simplified (no need to match OpenSSH exactly).

Do NOT implement a management UI.

---

## 5. Connection Robustness Improvements

Improve error handling for:

- Authentication failures
- Network failures
- Host key mismatch
- Timeout conditions
- Invalid credentials
- Missing key files

Requirements:

- No panics or crashes
- Clear user-facing error messages
- Graceful failure paths
- Maintain application stability

---

## 6. Workspace Stability Improvements

Strengthen remote editing workflow.

### Required Fixes

- Prevent duplicate file watchers for same file
- Safely handle editor closing
- Handle deleted or moved temp files
- Avoid infinite upload loops
- Ensure watcher cleanup when file no longer tracked

Upload behavior must remain consistent with Phase 2 design.

---

## 7. Minor Usability Improvements

Allowed small enhancements discovered during testing:

- Better disabled states for buttons
- Clear loading indicators
- Connection progress feedback
- Improved error display
- Prevent invalid actions during active operations

Do NOT redesign layout.

---

# BACKEND IMPLEMENTATION GUIDELINES

## Authentication Handling

Update SSH and SFTP services to:

- Support both auth types
- Request runtime credentials when needed
- Return structured errors for missing credentials
- Retry connection after credentials supplied

## Known Hosts Service

Implement a small service responsible for:

- Loading known hosts file
- Checking host key matches
- Adding new host entries

Keep implementation minimal.

## Error Types

Introduce structured error types where useful, for example:

UnknownHostKey  
HostKeyMismatch  
PasswordRequired  
PassphraseRequired  
AuthenticationFailed  
NetworkError  

---

# FRONTEND IMPLEMENTATION GUIDELINES

## Dialogs

Use existing in-app modal system.

Create dialogs for:

- Password input
- Passphrase input
- Unknown host confirmation

Do NOT use window.confirm or browser prompts.

## Status Feedback

Provide clear feedback during connection attempts.

Examples:

"Connecting…"  
"Awaiting password…"  
"Verifying host key…"  
"Authentication failed"  

---

# NON-GOALS

Explicitly DO NOT implement:

- Port forwarding
- Multi-tab sessions
- Sync features
- Credential storage systems
- Key generation tools
- Profile import/export
- Drag-and-drop features
- Plugin system
- Windows/macOS support
- Major UI changes

---

# EXPECTED OUTPUT

Implement Phase 4 changes directly in the codebase.

After implementation, provide:

1) Summary of changes  
2) Files modified  
3) New dependencies (if any)  
4) Migration notes (if applicable)  
5) Any remaining limitations  

Keep solutions minimal, maintainable, and production-safe.

---

# FINAL PRIORITY

Security > Stability > Simplicity > Features

Do not over-engineer.

Phase 4 is about making the existing MVP trustworthy and robust.
