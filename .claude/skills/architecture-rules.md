# Architecture Rules – MurmurSSH

## General
Keep the architecture simple, modular, and easy for open-source contributors to understand.

## High-Level Structure
Use a clear separation between:
- UI layer
- application state / orchestration
- backend system operations
- SSH/SFTP services
- local profile persistence

## Preferred Technical Direction
- Tauri for desktop shell
- Rust for backend/native operations
- frontend kept lightweight
- no unnecessary frameworks beyond what is needed for a small GUI

## Architecture Constraints
- do not put backend logic into UI components
- do not mix profile persistence logic into file browser views
- do not hardcode paths that should be configurable
- do not introduce a database for MVP unless clearly justified
- do not over-engineer abstractions before real need exists

## Suggested Modules
- profile service
- settings service
- SSH launch service
- SFTP service
- workspace/edit service
- file watcher service

## Persistence Rules
Profiles should be stored as individual local files where practical.
Use JSON as the preferred format for both profile files and settings.

## Error Handling
- fail clearly
- return actionable messages
- avoid silent failure
- avoid generic catch-all behavior without logging

## Dependency Rules
- keep dependencies minimal
- prefer well-maintained libraries
- avoid large dependencies for small tasks

## Packaging
Implementation choices should not block `.deb` packaging later.
