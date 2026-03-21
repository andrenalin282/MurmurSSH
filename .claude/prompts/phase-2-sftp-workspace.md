# Phase 2 – SFTP and Workspace Core

Read these files first:
- CLAUDE.md
- PRD.md
- .claude/skills/product-scope.md
- .claude/skills/architecture-rules.md
- .claude/skills/linux-integration.md
- .claude/skills/open-source-guidelines.md
- .claude/prompts/phase-1-foundation.md

## Goal

Implement the first usable MurmurSSH core features:
- real SFTP connectivity
- remote directory browsing
- single-file upload and download
- remote text file opening through a local workspace
- local file change detection
- upload-on-save behavior based on profile upload mode

Stay strictly within MVP scope.

## Current Repo State

Phase 1 already created:
- Tauri + Rust + Vite + TypeScript foundation
- minimal GUI shell
- profile persistence
- settings persistence
- SSH terminal launch service
- stubbed SFTP service
- stubbed workspace/edit service

Build on the existing structure. Do not replace the architecture.

## Required Implementation Tasks

### 1. Implement SFTP backend service
Implement the existing SFTP service using an appropriate Rust SSH/SFTP library.

Requirements:
- connect using profile data
- support SSH key authentication
- keep the implementation simple and maintainable
- avoid over-engineering session abstractions

Support these operations:
- list remote directory entries
- upload one file
- download one file
- delete one remote file
- rename one remote file
- create one remote directory

### 2. Wire the existing SFTP commands to real service logic
Replace current stub behavior in:
- backend services
- Tauri commands
- frontend API wrappers where needed

### 3. Make the file browser functional
Update the existing file browser so it can:
- load remote directory contents
- refresh listing
- trigger upload
- trigger download
- trigger open-for-edit for supported text files
- show basic error states clearly

Keep the UI minimal.

### 4. Implement workspace/edit flow
Implement the existing workspace service for remote text files.

Required flow:
1. user selects a remote text file
2. file is downloaded to the MurmurSSH workspace directory
3. file is opened with the configured editor command if present, otherwise with a sensible Linux-native default
4. the local file is watched for changes
5. when the file is saved:
   - if upload mode is `auto`, upload immediately
   - if upload mode is `confirm`, require confirmation before upload

### 5. Workspace and local file handling
Use a workspace path under:
- `~/.config/murmurssh/workspace/`

Requirements:
- ensure directories are created automatically
- avoid backup/versioning logic
- keep naming predictable and simple
- handle repeated opens of the same remote file safely

### 6. File watching
Implement local file change detection for edited workspace files.

Requirements:
- detect meaningful file changes
- avoid duplicate upload storms where reasonably possible
- keep implementation simple
- document any limitations clearly if needed

### 7. Editor launching
Use a Linux-native approach.

Requirements:
- if profile has `editorCommand`, use it
- otherwise use a sensible default approach for Linux desktop environments
- do not build an embedded editor

### 8. Error handling
Provide clear error handling for:
- authentication failure
- connection failure
- missing key path
- file not found
- upload/download failure
- unsupported file edit flow

Keep messages understandable.

### 9. Documentation
Update `README.md` to reflect:
- SFTP support
- workspace editing flow
- current limitations
- any required system packages or assumptions

## Constraints

- do not add a built-in terminal
- do not add profile management UI beyond what is necessary for current flow
- do not add multi-file transfer logic unless it is nearly free
- do not add sync logic
- do not add backup/versioning
- do not add cloud features
- do not add AppImage support
- do not introduce a database
- do not replace the current architecture
- do not add unnecessary dependencies

## Explicit Non-Goals

The following are out of scope for this phase:
- port forwarding
- password manager behavior
- secret storage redesign
- diff viewer
- drag and drop enhancements
- tabbed editor
- built-in code editor
- advanced transfer queue
- directory sync
- profile import/export
- UI redesign/polish beyond minimal functional updates

## Deliverables

At the end of this phase, provide:
1. a list of changed files
2. a short summary of how SFTP is implemented
3. a short summary of how workspace editing works
4. any intentionally deferred items
5. any required manual setup steps for local development

## Validation Checklist

Before finishing, verify that:
- the project still builds
- remote directory listing works
- single-file upload works
- single-file download works
- open-for-edit works for text files
- file change detection triggers upload behavior
- `auto` and `confirm` upload modes behave differently
- README matches the implemented state
