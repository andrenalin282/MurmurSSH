export type AuthType = "key" | "agent" | "password";

export type UploadMode = "auto" | "confirm";

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
  /** How the credential is retained between sessions. Absent/undefined means "never". */
  credential_storage_mode?: CredentialStorageMode;
  /**
   * Credential stored inside the profile JSON (portable_profile mode only).
   * SECURITY WARNING: plaintext — anyone with access to the file can read it.
   */
  stored_secret_portable?: string;
}

export interface Settings {
  last_used_profile_id: string | null;
}

export interface FileEntry {
  name: string;
  is_dir: boolean;
  size: number | null;
  /** Unix timestamp in seconds. */
  modified: number | null;
}

export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

/** Emitted by the backend when a watched file changes and upload_mode is "confirm". */
export interface UploadReadyPayload {
  profile_id: string;
  local_path: string;
  remote_path: string;
}
