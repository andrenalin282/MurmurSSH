# Product Scope Rules – MurmurSSH

## Mission
Build MurmurSSH as a minimal open-source Linux SSH/SFTP desktop client with a very small GUI and strict MVP discipline.

## Core Scope
Allowed:
- local profile management
- SSH launch through the Linux system terminal
- SFTP file browser
- upload/download
- remote text file edit flow via local cached file
- automatic upload on save or upload confirmation
- SSH key authentication
- `.deb` packaging
- last-used-profile restore on startup

## Explicitly Out of Scope
Do not add these unless the user explicitly changes scope:
- built-in custom terminal emulator
- cloud sync
- account system
- telemetry
- secret vault
- plugin system
- port forwarding
- remote sync engine
- team features
- backup/versioning for edited files
- support for non-SSH/SFTP protocols
- AppImage
- Windows/macOS support

## Product Philosophy
- prefer less code over more code
- prefer stable and clear over feature-rich
- prefer native Linux behavior over custom reimplementation
- prefer local files over databases for MVP
- prefer maintainability for open-source contributors

## Workflow Rules
- the user must always work from a saved profile
- no quick connect flow in MVP
- always restore the last used profile on startup
- SSH should open in the system terminal, not an embedded terminal widget
- SFTP editing should use local cached files and re-upload on save/confirm

## Decision Bias
When in doubt:
1. choose the simpler implementation
2. choose the more Linux-native implementation
3. choose the option with fewer dependencies
4. reject feature creep
