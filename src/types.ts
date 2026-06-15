export type AuthType = "key" | "agent" | "password";

export type UploadMode = "auto" | "confirm";

/** Connection protocol. Absent/undefined means "ssh" (backward compatible). */
export type Protocol = "ssh" | "sftp" | "ftp";

/**
 * How a credential (password or key passphrase) is persisted between app sessions.
 *
 * Security tiers, highest to lowest: never > local_machine > portable_profile.
 */
export type CredentialStorageMode = "never" | "local_machine" | "portable_profile";

export interface Profile {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_type: AuthType;
  /** Required when auth_type is "key", otherwise null. */
  key_path: string | null;
  default_remote_path: string | null;
  editor_command: string | null;
  upload_mode: UploadMode;
  /** Connection protocol. Absent/undefined means "ssh" (backward compatible). */
  protocol?: Protocol;
  /** Optional local working directory for uploads and downloads. */
  local_path?: string | null;
  /** How the credential is retained between sessions. Absent/undefined means "never". */
  credential_storage_mode?: CredentialStorageMode;
  /**
   * Credential stored inside the profile JSON (portable_profile mode only).
   * SECURITY WARNING: plaintext — anyone with access to the file can read it.
   */
  stored_secret_portable?: string;
  /**
   * Per-OS-user local browser paths (portable/shared profiles only).
   * Maps OS username → absolute local directory path.
   * Local-machine profiles use `local_path` instead.
   */
  local_paths_by_user?: Record<string, string>;
  /** Optional group/folder name. Empty/absent = ungrouped. */
  group?: string | null;
  /** Profile creation time, epoch seconds. Set by the backend. */
  created_at?: number | null;
}

export interface Settings {
  last_used_profile_id: string | null;
  /** Custom directory for profile JSON files. Null/absent = default path. */
  profiles_path?: string | null;
  /** UI theme: "dark" | "light" | "system". Null/absent = "system". */
  theme?: "dark" | "light" | "system" | null;
  /** Side the local file browser panel appears on. Null/absent = "left". */
  local_browser_position?: "left" | "right" | null;
  /** Max concurrent transfers the background queue runs. Null/absent = 2. Clamped 1..8. */
  max_concurrent_transfers?: number | null;
  /** Profile sort mode: "name" | "created". Null/absent = "name". */
  profile_sort?: "name" | "created" | null;
  /** Which profile group is expanded in the selector accordion (""=ungrouped, absent=none). */
  expanded_profile_group?: string | null;
}

export interface FileEntry {
  name: string;
  is_dir: boolean;
  size: number | null;
  /** Unix timestamp in seconds. */
  modified: number | null;
  /** Unix permission/mode bits (e.g. 0o644). Null when unavailable (e.g. FTP). */
  perm: number | null;
}

export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

/** A transfer queue job snapshot, mirrors Rust TransferJobView. */
export interface TransferJobView {
  id: number;
  profileId: string;
  kind: "upload" | "download" | "uploadDir" | "downloadDir";
  src: string;
  dst: string;
  filename: string;
  state: "queued" | "active" | "done" | "failed" | "cancelled";
  bytesDone: number;
  bytesTotal: number;
  error: string | null;
}

/** Emitted by the backend when a watched file changes and upload_mode is "confirm". */
export interface UploadReadyPayload {
  profile_id: string;
  local_path: string;
  remote_path: string;
}
