# Phase 3 – Profile Management and Usability Baseline

Read these files first:
- CLAUDE.md
- PRD.md
- .claude/skills/product-scope.md
- .claude/skills/architecture-rules.md
- .claude/skills/linux-integration.md
- .claude/skills/open-source-guidelines.md
- .claude/prompts/phase-1-foundation.md
- .claude/prompts/phase-2-sftp-workspace.md

## Goal

Make MurmurSSH usable by real users without manual JSON editing by implementing graphical profile management and minimal usability improvements.

Stay strictly within MVP scope.

## Current Repo State

Phase 1 and 2 already provide:
- working Tauri foundation
- profile persistence backend
- SSH launch via system terminal
- functional SFTP file browser
- workspace-based remote editing
- minimal GUI shell

Profiles currently must be created manually as JSON files.

## Required Implementation Tasks

### 1. Profile Management UI

Implement a simple graphical interface to:

- create a new profile
- edit an existing profile
- delete a profile
- select active profile
- persist changes using existing backend services

Keep the UI minimal and consistent with the current design.

Required fields:

- display name
- host
- port
- username
- authentication type (key-based initially)
- SSH private key path
- default remote path (optional)
- editor command (optional)
- upload mode (auto / confirm)

Do not introduce advanced SSH options.

### 2. File Picker for SSH Key Path

Provide a native file selection dialog to choose the private key file.

Requirements:

- use a Linux-native file dialog via Tauri APIs
- store absolute path
- validate existence of the file
- do not copy the key into app storage

### 3. Profile Validation

Before saving or connecting, validate:

- required fields are present
- port is numeric and valid
- key path exists when key auth is selected

Provide clear error messages in the UI.

### 4. Last Used Profile Behavior

Ensure:

- last used profile is saved in settings
- application restores it on startup
- profile selector reflects this correctly

### 5. Connection Feedback

Improve user feedback during connect attempts:

- show connecting state
- show success or failure
- display clear error messages on failure

Keep visuals simple.

### 6. Replace Browser Confirm Dialogs

Replace use of `window.confirm()` with simple in-app dialogs or modals.

Requirements:

- minimal design
- reusable component if possible
- no heavy UI frameworks

### 7. Safe Profile Deletion

Implement confirmation before deleting a profile.

Deletion should remove only the profile file, not workspace data.

### 8. Minor Usability Improvements

Allow small adjustments that improve usability without expanding scope, such as:

- disabling actions when no profile is selected
- clearer empty states
- basic layout polish

Do not redesign the UI.

### 9. Documentation

Update README.md to explain:

- how to create profiles
- supported authentication method
- any limitations

## Constraints

- do not introduce cloud sync
- do not add password storage
- do not implement advanced SSH configuration
- do not add multi-profile connection tabs
- do not redesign the entire UI
- do not add heavy UI libraries
- do not introduce a database
- do not change existing architecture
- maintain compatibility with `.deb` packaging

## Explicit Non-Goals

Out of scope for this phase:

- port forwarding
- password authentication UX
- key generation
- profile import/export
- secrets management
- session tabs
- connection history
- theming system
- drag-and-drop enhancements
- advanced notifications
- host key management UI

## Deliverables

At completion, provide:

1. list of changed files
2. summary of profile management implementation
3. description of validation approach
4. any deferred items
5. updated setup or usage notes

## Validation Checklist

Before finishing, verify that:

- a user can create a profile via GUI
- a profile can be edited and saved
- a profile can be deleted safely
- the SSH key can be selected via file dialog
- the last used profile loads automatically
- connection attempts show feedback
- application still builds and runs on Linux
