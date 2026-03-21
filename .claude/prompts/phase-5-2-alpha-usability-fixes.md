You are acting as a senior Rust/Tauri engineer, frontend usability engineer, and QA-focused software maintainer.

We are continuing development of the Linux desktop application **MurmurSSH**.

The project is implemented through Phase 5.1 and is currently in a usable alpha state.

Your task is to implement **Phase 5.2 — Alpha Usability Fixes, State Corrections, and Pre-Test Stabilization**.

This phase is NOT for new product features.
It is for fixing real usability issues and stabilizing the application so it can be tested cleanly.

---

# CRITICAL RULES

## DO NOT

- Do NOT redesign the application
- Do NOT add major new features
- Do NOT change the architecture
- Do NOT add cloud features
- Do NOT add a database
- Do NOT add embedded terminal functionality
- Do NOT add import/export
- Do NOT expand scope beyond usability/stability fixes

## MUST PRESERVE

- Tauri 2 + Rust + Vanilla TypeScript + Vite
- services → commands → UI layering
- Linux-first behavior
- .deb compatibility
- minimal UI philosophy
- open-source maintainability
- existing auth and credential behavior from previous phases

---

# PROJECT CONTEXT

MurmurSSH is a minimal SSH + SFTP desktop client for Linux (Ubuntu).

Already implemented:

- Profile management
- SSH launch via system terminal
- SFTP browsing
- Upload/download
- Workspace editing
- Runtime auth prompts
- Password storage modes
- Passphrase runtime-only behavior
- Known hosts
- Basic credential hardening

The app starts successfully and is close to practical alpha testing.

---

# PHASE 5.2 GOALS

Focus strictly on:

1. Fixing profile-selection state issues
2. Fixing disabled/unavailable action buttons
3. Correcting form/dropdown visual consistency
4. Cleaning warnings and obvious code hygiene issues
5. Making the app ready for reliable manual testing

---

# REQUIRED IMPLEMENTATIONS

## 1. Selected Profile State Fix

Investigate and fix the issue where:

- a profile appears selected in the dropdown
- but Edit and/or Connect remain unavailable or disabled

Requirements:

- profile selection state must be reliable
- selected profile must propagate correctly through UI state
- Edit must enable when a valid profile is selected
- Connect must enable when a valid profile is selected
- Delete must behave consistently with the selected profile
- initial load / restored last-used profile must also update UI correctly

Check for:
- stale state
- missing rerender/update hooks
- async load order issues
- mismatch between dropdown value and internal selected-profile object

---

## 2. Profile Action Availability Audit

Audit all profile-related action states.

Verify and fix if needed:

- New is always usable
- Edit only enabled when a profile is selected
- Delete only enabled when a profile is selected
- Connect only enabled when selected profile is valid for current auth type
- disabled states update immediately after:
  - selecting a profile
  - creating a profile
  - editing a profile
  - deleting a profile
  - restoring last used profile

---

## 3. Form Dropdown Theme Consistency

Fix the visual issue where dropdown/select controls in the profile form appear light while the rest of the UI is dark.

Requirements:

- authentication dropdown should match dark theme
- upload mode dropdown should match dark theme
- any other select/option controls in the modal should match dark theme as far as practical
- text should remain readable
- styling should remain simple and maintainable

Do not introduce UI frameworks.
Use minimal CSS/DOM-compatible styling.

---

## 4. Form Usability Pass

Review the profile form for obvious alpha blockers.

Fix only clear usability issues such as:

- misleading placeholder values
- fields not enabling/disabling correctly
- auth-type changes not updating dependent fields immediately
- saved credential section showing in the wrong cases
- validation messages not updating clearly

Keep the UI minimal.

---

## 5. Warning Cleanup

Remove or resolve current obvious warnings where appropriate.

Known examples:

- unused import: `Read`
- dead code: `sanitize_id`

Requirements:

- remove unused imports
- remove dead code if truly unused
- or wire it correctly if it should exist
- leave the codebase cleaner than before

---

## 6. Pre-Test Stability Pass

Perform a focused alpha test-readiness pass on key flows:

- app startup
- profile restore on launch
- profile selection
- create/edit/delete profile
- connect button enablement
- profile form interactions
- auth type switching
- credential section visibility
- no obvious broken states after save/cancel/delete

Fix only real blockers found during this pass.

---

# VALIDATION REQUIREMENTS

Before finishing, verify and report:

1. Selected profile enables Edit/Delete correctly
2. Selected valid profile enables Connect correctly
3. Last-used restored profile also updates button states correctly
4. Form dropdowns visually fit the dark theme
5. Auth-type changes update form fields immediately
6. No obvious stale UI state remains after create/edit/delete
7. Current warnings are removed or reduced appropriately
8. App remains buildable and runnable

---

# NON-GOALS

Explicitly do NOT implement:

- new authentication systems
- secret storage redesign
- keyring integration
- port forwarding
- import/export
- drag-and-drop
- tabs
- sync
- release packaging overhaul
- major visual redesign

---

# IMPLEMENTATION GUIDELINES

- Prefer fixing current state flow over rewriting components
- Prefer small, focused changes
- Keep components understandable for open-source contributors
- Do not over-engineer state management
- Use direct, explicit logic rather than abstraction-heavy solutions

---

# EXPECTED OUTPUT

Implement Phase 5.2 directly in the codebase.

After completion, provide:

1. Summary of fixes
2. Files changed
3. Root cause of the selected-profile/button-state issue
4. Any UI styling changes made
5. Warnings resolved
6. Any remaining known issues
