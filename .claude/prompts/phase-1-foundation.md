# Phase 1 – Foundation Prompt

Read these files first before making changes:
- PRD.md
- .claude/skills/product-scope.md
- .claude/skills/architecture-rules.md
- .claude/skills/linux-integration.md
- .claude/skills/open-source-guidelines.md
- CLAUDE.md

## Goal
Create the initial project foundation for MurmurSSH as a minimal open-source Linux desktop application.

## Requirements
- follow the PRD strictly
- stay within MVP scope
- do not add out-of-scope features
- optimize for Ubuntu Linux and future `.deb` packaging
- keep the GUI small and simple
- use the Linux system terminal for SSH, not a custom terminal implementation

## Tasks
1. Initialize the project foundation using the preferred stack from the PRD
2. Create a clean folder structure for app code
3. Add a minimal GUI shell with placeholder areas for:
   - profile selection
   - connect action
   - connection status
   - file browser area
4. Add profile model definitions and local persistence scaffolding
5. Add settings scaffolding for last-used-profile behavior
6. Add placeholders/interfaces for:
   - SSH launch service
   - SFTP service
   - workspace/edit flow
7. Ensure Tauri is configured for Linux builds and that packaging metadata supports `.deb` generation
8. Create an initial README with setup instructions

## Constraints
- no embedded terminal
- no cloud features
- no advanced SSH feature set
- no database unless absolutely necessary
- no over-engineering

## Deliverables
At the end of the phase, provide:
- list of created files
- short explanation of architecture choices
- anything still stubbed or intentionally deferred
