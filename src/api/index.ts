import { invoke, Channel } from "@tauri-apps/api/core";
import type { FileEntry, Profile, Settings } from "../types";

/** Progress event streamed during file transfers. */
export interface TransferProgress {
  bytesDone: number;
  bytesTotal: number; // 0 = unknown (FTP or folder op)
  filename: string;
}

export type TransferChannel = Channel<TransferProgress>;

export async function listProfiles(): Promise<Profile[]> {
  return invoke("list_profiles");
}

export async function getProfile(id: string): Promise<Profile> {
  return invoke("get_profile", { id });
}

export async function saveProfile(profile: Profile): Promise<void> {
  return invoke("save_profile", { profile });
}

export async function deleteProfile(id: string): Promise<void> {
  return invoke("delete_profile", { id });
}

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export interface SshConfigEntry {
  host: string;
  hostname: string | null;
  user: string | null;
  port: number | null;
  identity_file: string | null;
}

/**
 * Parse ~/.ssh/config and return importable host entries.
 * Only non-wildcard Host stanzas with parseable fields are returned.
 * Throws a string error if the file is missing or unreadable.
 */
export async function parseSshConfig(): Promise<SshConfigEntry[]> {
  return invoke("parse_ssh_config");
}

export async function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}

/**
 * Launch an SSH terminal session for the profile.
 *
 * If `useRuntimeCopy` is true, the terminal uses the pre-created local copy
 * of the key file (in ~/.config/murmurssh/runtime-keys/) instead of the original.
 * This fixes "UNPROTECTED PRIVATE KEY FILE" rejections when keys are on mounted
 * or network filesystems. Requires `copyKeyForRuntime()` to have been called first.
 */
export async function launchSsh(
  profileId: string,
  useRuntimeCopy?: boolean
): Promise<void> {
  return invoke("launch_ssh", {
    profileId,
    useRuntimeCopy: useRuntimeCopy ?? false,
  });
}

/**
 * Check whether the SSH key for a profile needs a local runtime copy for
 * terminal compatibility. Returns true if the key file has group or other
 * permission bits set (OpenSSH rejects keys in that case).
 */
export async function checkKeyNeedsCopy(profileId: string): Promise<boolean> {
  return invoke("check_key_needs_copy", { profileId });
}

/**
 * Create a local runtime copy of the SSH key for terminal compatibility.
 * The copy is stored in ~/.config/murmurssh/runtime-keys/<profile_id> with
 * 0600 permissions. The original key file is never modified.
 * Must only be called after the user has explicitly accepted the copy prompt.
 */
export async function copyKeyForRuntime(profileId: string): Promise<void> {
  return invoke("copy_key_for_runtime", { profileId });
}

/**
 * Delete the runtime key copy for a profile.
 * Called on disconnect to clean up the temporary file immediately.
 */
export async function deleteRuntimeKey(profileId: string): Promise<void> {
  return invoke("delete_runtime_key", { profileId });
}

/**
 * Delete all runtime key copies (startup cleanup).
 * Removes leftover runtime keys from sessions that crashed or were force-killed.
 */
export async function cleanupRuntimeKeys(): Promise<void> {
  return invoke("cleanup_runtime_keys");
}

/**
 * Verify the connection to a profile, optionally supplying runtime credentials.
 * Credentials are held in session memory only — never written to disk.
 *
 * The backend returns structured error strings the caller must handle:
 * - "UNKNOWN_HOST:<fingerprint>" — call acceptHostKey() then retry
 * - "NEED_PASSWORD"              — call again with password supplied
 * - "NEED_PASSPHRASE"            — call again with passphrase supplied
 * - other strings                — connection or auth failure
 */
export async function connectSftp(
  profileId: string,
  password?: string,
  passphrase?: string
): Promise<void> {
  return invoke("connect_sftp", {
    profileId,
    password: password ?? null,
    passphrase: passphrase ?? null,
  });
}

/**
 * Save an accepted host key fingerprint to the local known_hosts file.
 * Called after the user chooses "Accept and save" in the host key dialog.
 */
export async function acceptHostKey(
  profileId: string,
  fingerprint: string
): Promise<void> {
  return invoke("accept_host_key", { profileId, fingerprint });
}

/**
 * Trust a host key for this session only — does NOT write to known_hosts.
 * Called after the user chooses "Accept once" in the host key dialog.
 * On next app launch the host will appear unknown again.
 */
export async function acceptHostKeyOnce(
  profileId: string,
  fingerprint: string
): Promise<void> {
  return invoke("accept_host_key_once", { profileId, fingerprint });
}

/**
 * Clear only the in-memory session credential cache for a profile.
 * Does NOT touch any persistent storage (saved passwords or portable profile fields).
 * Use on disconnect so the next connect reloads from persistent storage or re-prompts.
 */
export async function clearSessionCredentials(profileId: string): Promise<void> {
  return invoke("clear_session_credentials", { profileId });
}

export async function checkPathExists(path: string): Promise<boolean> {
  return invoke("check_path_exists", { path });
}

export async function openProfileFolder(): Promise<void> {
  return invoke("open_profile_folder");
}

export async function getProfilesPath(): Promise<string> {
  return invoke("get_profiles_path");
}

/**
 * Resolve the server-side effective SFTP home directory using realpath(".").
 * Returns the absolute path the SFTP server reports as the initial working
 * directory (typically the user's home). Falls back to "/" on any server error.
 * Called when a profile has no explicit default_remote_path configured.
 */
export async function getSftpHome(profileId: string): Promise<string> {
  return invoke("get_sftp_home", { profileId });
}

export async function listDirectory(
  profileId: string,
  path: string
): Promise<FileEntry[]> {
  return invoke("list_directory", { profileId, path });
}

/**
 * Check whether a path exists on the remote server via SFTP stat().
 * Returns true if accessible (file or directory), false if not found.
 * Used before upload to detect conflicts for the overwrite dialog.
 */
export async function remoteFileExists(
  profileId: string,
  remotePath: string
): Promise<boolean> {
  return invoke("remote_file_exists", { profileId, remotePath });
}

/**
 * Upload raw bytes to a remote path.
 * Read the file in JS with FileReader, then pass the bytes here.
 */
export async function uploadFileBytes(
  profileId: string,
  remotePath: string,
  content: number[]
): Promise<void> {
  return invoke("upload_file_bytes", { profileId, remotePath, content });
}

/**
 * Upload a local path (file or directory) to a remote destination.
 * Automatically handles both files and directories.
 * Used by drag-and-drop upload where the item type isn't known ahead of time.
 */
export async function uploadPath(
  profileId: string,
  localPath: string,
  remotePath: string,
  onProgress: TransferChannel
): Promise<void> {
  return invoke("upload_path", { profileId, localPath, remotePath, onProgress });
}

/**
 * Recursively upload a local directory to a remote destination path.
 */
export async function uploadDirectory(
  profileId: string,
  localPath: string,
  remotePath: string,
  onProgress: TransferChannel
): Promise<void> {
  return invoke("upload_directory", { profileId, localPath, remotePath, onProgress });
}

/**
 * Upload a local file path to a remote path.
 */
export async function uploadFile(
  profileId: string,
  localPath: string,
  remotePath: string,
  onProgress: TransferChannel
): Promise<void> {
  return invoke("upload_file", { profileId, localPath, remotePath, onProgress });
}

/**
 * Download a remote file to ~/Downloads/<filename>.
 * Returns the local path where the file was saved.
 */
export async function downloadFile(
  profileId: string,
  remotePath: string,
  onProgress: TransferChannel
): Promise<string> {
  return invoke("download_file", { profileId, remotePath, onProgress });
}

/**
 * Download a remote file to a user-specified local path.
 */
export async function downloadFileTo(
  profileId: string,
  remotePath: string,
  localPath: string,
  onProgress: TransferChannel
): Promise<void> {
  return invoke("download_file_to", { profileId, remotePath, localPath, onProgress });
}

export async function deleteFile(
  profileId: string,
  remotePath: string
): Promise<void> {
  return invoke("delete_file", { profileId, remotePath });
}

/**
 * Recursively download a remote directory to a local destination path.
 */
export async function downloadDirectory(
  profileId: string,
  remotePath: string,
  localPath: string,
  onProgress: TransferChannel
): Promise<void> {
  return invoke("download_directory", { profileId, remotePath, localPath, onProgress });
}

/**
 * Recursively delete a remote directory and all of its contents.
 * The caller must confirm with the user before invoking this.
 */
export async function deleteDirectory(
  profileId: string,
  remotePath: string
): Promise<void> {
  return invoke("delete_directory", { profileId, remotePath });
}

export async function renameFile(
  profileId: string,
  fromPath: string,
  toPath: string
): Promise<void> {
  return invoke("rename_file", { profileId, fromPath, toPath });
}

export async function createDirectory(
  profileId: string,
  path: string
): Promise<void> {
  return invoke("create_directory", { profileId, path });
}

export async function openForEdit(
  profileId: string,
  remotePath: string
): Promise<void> {
  return invoke("open_for_edit", { profileId, remotePath });
}

/**
 * Establish a background SSH session for terminal SSO.
 * Must be called after connectSftp() succeeds.
 * Non-fatal: if this fails the terminal still works, just re-prompts.
 */
export async function startSshSession(profileId: string): Promise<void> {
  return invoke("start_ssh_session", { profileId });
}

/**
 * Stop the SSH SSO session for a profile (called on disconnect).
 * Kills the ControlMaster or ssh-agent process and removes the socket.
 */
export async function stopSshSession(profileId: string): Promise<void> {
  return invoke("stop_ssh_session", { profileId });
}

/**
 * Persist a credential for a profile with the given storage mode.
 *
 * mode "local_machine"   — plaintext file at ~/.config/murmurssh/secrets/<id>, 0600 permissions.
 *                          Does not travel with the profile. Machine-local only.
 * mode "portable_profile" — stored inside the profile JSON. Portable but LESS SECURE.
 *                           Anyone with access to the profile file can read the credential.
 * mode "never"           — no-op.
 */
export async function saveCredential(
  profileId: string,
  secret: string,
  mode: string
): Promise<void> {
  return invoke("save_credential", { profileId, secret, mode });
}

/**
 * Remove any stored credential for a profile (both local machine file and portable field).
 * The next connection will prompt for credentials again.
 */
export async function clearCredential(profileId: string): Promise<void> {
  return invoke("clear_credential", { profileId });
}

/** Close the application cleanly. */
export async function quitApp(): Promise<void> {
  return invoke("quit_app");
}

/** Return the application version string (e.g. "1.0.0"). */
export async function getAppVersion(): Promise<string> {
  return invoke("get_app_version");
}

/** Open a URL in the system default browser using xdg-open. Only https/http allowed. */
export async function openUrl(url: string): Promise<void> {
  return invoke("open_url", { url });
}

// ── Local filesystem ──────────────────────────────────────────────────────────

/** List the contents of a local directory. Sorted: dirs first, then files (alphabetical). */
export async function listLocalDirectory(path: string): Promise<FileEntry[]> {
  return invoke("list_local_directory", { path });
}

/** Return the current user's home directory ($HOME). */
export async function getHomeDir(): Promise<string> {
  return invoke("get_home_dir");
}

/** Return the current OS username ($USER / $LOGNAME). */
export async function getCurrentUser(): Promise<string> {
  return invoke("get_current_user");
}

/**
 * Return the saved local browser start path for a profile + the current OS user.
 * Falls back to $HOME if nothing is saved or the saved path no longer exists.
 */
export async function getLocalBrowserPath(profileId: string): Promise<string> {
  return invoke("get_local_browser_path", { profileId });
}

/**
 * Persist the local browser path for the profile + current OS user.
 * Portable profiles save per-user; local-machine profiles save to local_path.
 */
export async function saveLocalBrowserPath(profileId: string, path: string): Promise<void> {
  return invoke("save_local_browser_path", { profileId, path });
}

/** Rename a local file or directory within the same parent directory. */
export async function renameLocalFile(fromPath: string, toPath: string): Promise<void> {
  return invoke("rename_local_file", { fromPath, toPath });
}

/**
 * Open a local file with the system default app, or with a custom editor command.
 * If editor is null/"", falls back to xdg-open.
 */
export async function openLocalFile(path: string, editor: string | null): Promise<void> {
  return invoke("open_local_file", { path, editor: editor ?? null });
}
