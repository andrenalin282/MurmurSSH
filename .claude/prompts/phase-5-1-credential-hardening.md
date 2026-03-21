# Phase 5.1 – Credential Hardening and Secret Cleanup

Read these files first:
- CLAUDE.md
- PRD.md
- .claude/skills/product-scope.md
- .claude/skills/architecture-rules.md
- .claude/skills/linux-integration.md
- .claude/skills/open-source-guidelines.md

## Goal

Refine the recently added credential-saving behavior so it is safer, internally consistent, and aligned with product rules:

- passwords may be saved
- passphrases must never be saved
- portable profile mode must remain available
- credential clearing and deletion behavior must be complete and reliable

Do not redesign the feature. Harden and correct the current implementation.

## Current Product Rules

### Passwords
Passwords may be handled in three modes:
- do_not_save
- save_locally
- portable_profile

Portable profile mode must remain available because profiles may be synced between multiple office PCs.

Portable mode must remain clearly labeled as less secure / portable.

Do not make misleading claims about encryption or strong protection if the stored secret is still recoverable without an additional user secret.

### Passphrases
Passphrases for encrypted SSH keys must never be saved.

Rules:
- always ask at connection time when required
- keep only in memory for the current connection flow
- do not write to profile JSON
- do not persist in local credential storage
- do not persist in portable profile storage

## Required Implementation Tasks

### 1. Separate password and passphrase persistence rules
Ensure the code clearly distinguishes between:
- password authentication secrets
- SSH key passphrases

Password secrets may follow the configured credential storage mode.

SSH key passphrases must always be runtime-only and must never be stored.

### 2. Audit and correct save paths
Review all save/load flows and ensure:
- passphrases cannot accidentally be saved through generic credential-saving logic
- only password auth can result in persistent credential storage
- connect flows enforce the correct rule path

### 3. Harden clear_credential behavior
Ensure clear_credential fully removes all persisted credential data for the target profile.

This includes:
- local stored credential data
- portable stored credential fields in profile JSON
- related metadata fields
- stale UI state after clearing

### 4. Profile deletion cleanup
Ensure deleting a profile also removes any associated saved credential data.

Requirements:
- no orphaned local secret files/entries remain
- no stale portable secret fields remain
- deletion remains profile-only and predictable

### 5. Auth-mode switching cleanup
If a profile is changed from password auth to SSH key auth:
- remove any previously saved password data if appropriate
- ensure UI does not still show saved-password state

If a profile is changed from SSH key auth to password auth:
- do not carry passphrase assumptions into password storage

Keep behavior explicit and safe.

### 6. Frontend state cleanup
Review the frontend flow and ensure:
- secrets are not kept longer than needed in component state
- cleared credentials immediately update the UI
- saved credential labels reflect actual backend state
- no misleading status is shown

### 7. Portable mode messaging
Keep portable mode available, but ensure the wording remains honest:
- clearly less secure
- portable by design
- suitable only if the user accepts the risk

Do not falsely imply that salt/obfuscation alone provides meaningful protection.

### 8. Documentation update
Update README.md and CLAUDE.md so they clearly state:
- passwords may be saved
- passphrases are never saved
- portable profile mode is less secure
- clearing credentials fully removes saved password data for that profile

## Validation Requirements

Before finishing, verify and report:

1. Password auth with do_not_save works
2. Password auth with save_locally works
3. Password auth with portable_profile works
4. SSH key auth with passphrase always prompts and never saves
5. clear_credential removes all saved password data
6. deleting a profile removes any associated persisted password data
7. switching auth modes does not leave stale secret state behind
8. UI reflects the real saved/cleared state correctly

## Constraints

- do not remove portable profile mode
- do not add OS keyring integration in this phase
- do not redesign the whole credential system
- do not add cloud features
- do not add password generation
- do not add import/export
- do not add misleading security wording
- do not break existing profile compatibility unless absolutely necessary

## Deliverables

At completion, provide:

1. list of changed files
2. summary of credential behavior after hardening
3. explanation of how passphrases are prevented from being stored
4. explanation of clear/delete cleanup behavior
5. any remaining known limitations
