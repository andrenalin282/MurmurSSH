You are acting as a senior Rust/Tauri engineer and security-focused desktop application architect.

We are continuing development of the Linux desktop application **MurmurSSH**.

The project is implemented through Phase 4 and currently in a usable alpha state.

Your task is to implement **Phase 5 — Credential Storage Modes, Profile Portability, and Beta Usability Improvements**.

This phase extends authentication usability while preserving the existing architecture, minimal philosophy, and Linux-first design.

---

# CRITICAL RULES

## DO NOT

- Do NOT redesign the UI
- Do NOT add cloud features
- Do NOT add a database
- Do NOT embed a terminal
- Do NOT add port forwarding
- Do NOT add multi-session UI
- Do NOT introduce heavy frameworks
- Do NOT break existing working profile compatibility
- Do NOT remove current security protections

## MUST PRESERVE

- Tauri 2 + Rust + Vanilla TypeScript architecture
- services → commands → UI layering
- local-only configuration
- minimal UX
- Ubuntu / Linux-first behavior
- .deb compatibility
- open-source maintainability

---

# PROJECT CONTEXT

MurmurSSH is a minimal SSH + SFTP client for Linux (Ubuntu), similar in spirit to Bitvise/Termius but intentionally simple.

Already implemented:

- GUI profile management
- SSH launch via system terminal
- SFTP browsing
- Upload/download
- Remote editing via workspace
- Upload-on-save
- In-app dialogs
- Auth type support
- Runtime password/passphrase flow
- Host key verification / known_hosts support
- Connection robustness improvements

Profiles stored under:

~/.config/murmurssh/profiles/*.json

Known hosts stored locally.

Workspace stored under:

~/.config/murmurssh/workspace/

---

# PHASE 5 OBJECTIVES

This phase focuses on:

1. Credential storage modes
2. Profile portability behavior
3. SSH key / password usability refinement
4. Beta-level usability improvements
5. Clear security-tier UX

This is NOT a major feature expansion phase.

---

# REQUIRED IMPLEMENTATIONS

## 1. Authentication Type UX Finalization

Ensure profile form cleanly supports:

- Password authentication
- SSH key authentication

Requirements:

- auth_type dropdown must be clear and stable
- when auth_type = password:
  - hide or disable private key path field
- when auth_type = ssh_key:
  - show private key path field
  - allow browsing via file picker
  - allow manual path entry if desired

---

## 2. Credential Storage Modes

Implement explicit credential save behavior for both password auth and ssh key passphrase handling.

### Required storage modes

- `never`
- `local_machine`
- `portable_profile`

These modes apply to:
- password (for password auth)
- passphrase (for encrypted SSH key auth)

### Behavior

#### never
- user is prompted at connection time
- secret is held only in memory for current operation/session
- nothing written to disk

#### local_machine
- secret may be stored only for this machine
- profile remains usable, but secret should not be portable to another computer
- this must be presented as the higher security option

#### portable_profile
- secret is stored in a way that allows the profile to be used on another computer
- this is intentionally weaker security
- UI must clearly warn the user before saving in this mode

### Important

Do NOT silently treat `portable_profile` as secure.
The UI must clearly communicate:

- portable profile storage is less secure
- local_machine is more secure
- never is safest

---

## 3. Profile Data Model Updates

Extend profile model as needed while preserving backward compatibility.

Profile should support fields needed for:

- auth_type
- key path
- secret storage mode metadata
- portable/local distinction

Requirements:

- old profiles must continue to load
- default behavior for older profiles must remain sensible
- migration should be minimal and automatic if possible

---

## 4. Secret Storage Backend

Implement secret handling for the required storage modes.

### local_machine mode

Preferred behavior:
- store secret locally in a machine-specific way
- it must not be intended for profile portability

### portable_profile mode

Required behavior:
- secret travels with the profile so another PC can use it
- this must be clearly marked as lower security

### Important design rule

If there is no truly secure portable storage without additional complexity, choose the minimal reversible implementation, but:

- label it clearly as weaker security
- avoid misleading wording
- do not present it as equivalent to local_machine mode

---

## 5. Save Password / Passphrase UX

When the user enters a password or passphrase, allow a save choice dialog such as:

- Do not save
- Save only on this PC
- Save in profile (usable on other PCs)

This should be reusable for:
- password auth
- key passphrase auth

Keep the UI minimal and consistent.

---

## 6. SSH Key Usability Improvements

Improve ssh key handling UX:

- allow file picker selection
- allow manual path editing
- validate path exists when required
- support keeping the path in profile
- preserve compatibility with existing key-based profiles

Do NOT copy private key files into app storage.

---

## 7. Optional Host Privacy Improvement

If straightforward and low-risk, add optional hostname hashing for the local known_hosts storage.

Rules:

- do not break existing known_hosts behavior
- do not over-engineer this
- skip this if it would significantly complicate implementation

This is optional, not mandatory.

---

## 8. Beta Usability Improvements

Allow minor refinements that improve real usability:

- clearer credential prompts
- clearer warnings for security tiers
- better disabled states
- better feedback when a saved secret is being used
- small polish around auth/profile flows

Do NOT redesign layout.

---

# SECURITY REQUIREMENTS

- Never imply that portable_profile storage is as secure as local_machine storage
- Never silently store passwords or passphrases
- Do not log secrets
- Do not leak secrets into error messages
- Do not copy SSH keys into app-managed storage
- Preserve current host key verification logic

---

# NON-GOALS

Explicitly do NOT implement:

- cloud sync
- profile sync service
- database storage
- password manager integration beyond what is minimally needed
- key generation
- port forwarding
- session tabs
- multi-host dashboards
- drag-and-drop features
- import/export wizard
- major release packaging changes

---

# IMPLEMENTATION GUIDELINES

- Prefer extending existing profile/auth dialogs instead of inventing new architecture
- Prefer modifying existing services rather than adding many new modules
- Keep code easy for open-source contributors to follow
- Keep data structures explicit
- Add comments only where they prevent confusion

---

# EXPECTED OUTPUT

Implement Phase 5 directly in the codebase.

After completion, provide:

1. Summary of changes
2. Files modified
3. New files added
4. New dependencies
5. Profile compatibility / migration notes
6. Security notes
7. Remaining limitations

---

# FINAL PRIORITY

Usability + clarity + honest security semantics > abstraction > feature count

Do not over-engineer.
