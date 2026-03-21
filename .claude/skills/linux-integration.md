# Linux Integration Rules – MurmurSSH

## Platform Priority
Primary platform is Ubuntu Linux desktop.

## SSH Terminal Strategy
Do not build a custom terminal emulator for MVP.

Use the system terminal where possible.
Preferred approach:
- detect and use `x-terminal-emulator` first on Debian/Ubuntu systems
- allow fallback handling if needed later

SSH should be launched through the native `ssh` command.

## Authentication
Prioritize:
- SSH key authentication
- optional ssh-agent support if simple

Do not spend MVP time on complex password automation.

## Filesystem
Use Linux-friendly local storage paths.

Expected config base:
- `~/.config/murmurssh/`

Expected areas:
- profiles
- settings
- workspace
- logs

## Editor Integration
Remote files should be downloaded to a local workspace/cache location.
Editing may use:
- system default editor
- configured external editor

The app should watch the local file and upload changes back based on profile settings.

## Packaging Target
Official packaging target for MVP:
- `.deb`

Do not optimize for AppImage in the initial implementation.

## Desktop Behavior
The app should behave like a small Linux desktop utility:
- fast startup
- minimal UI
- no account prompts
- no cloud dependency
