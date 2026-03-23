import { invoke } from "@tauri-apps/api/core";
import type { FileEntry, Profile, Settings } from "../types";

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
 * Recursively upload a local directory to a remote destination path.
 * The remote_path is the full destination path (e.g. /home/user/mydir).
 * Creates the directory and its entire contents on the remote server.
 * Existing remote directories are tolerated (not treated as errors).
 */
export async function uploadDirectory(
  profileId: string,
  localPath: string,
  remotePath: string
): Promise<void> {
  return invoke("upload_directory", { profileId, localPath, remotePath });
}

/**
 * Upload a local file path to a remote path.
 * Used by the workspace confirm flow after the user approves.
 */
export async function uploadFile(
  profileId: string,
  localPath: string,
  remotePath: string
): Promise<void> {
  return invoke("upload_file", { profileId, localPath, remotePath });
}

/**
 * Download a remote file to ~/Downloads/<filename>.
 * Returns the local path where the file was saved.
 */
export async function downloadFile(
  profileId: string,
  remotePath: string
): Promise<string> {
  return invoke("download_file", { profileId, remotePath });
}

/**
 * Download a remote file to a user-specified local path.
 * Used after the user picks a save location via the save dialog.
 */
export async function downloadFileTo(
  profileId: string,
  remotePath: string,
  localPath: string
): Promise<void> {
  return invoke("download_file_to", { profileId, remotePath, localPath });
}

export async function deleteFile(
  profileId: string,
  remotePath: string
): Promise<void> {
  return invoke("delete_file", { profileId, remotePath });
}

/**
 * Recursively download a remote directory to a local destination path.
 * The local_path is the full destination path (e.g. /home/user/mydir).
 * Creates the directory and its full contents locally.
 */
export async function downloadDirectory(
  profileId: string,
  remotePath: string,
  localPath: string
): Promise<void> {
  return invoke("download_directory", { profileId, remotePath, localPath });
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
