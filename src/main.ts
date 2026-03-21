import "./styles.css";
import { listen } from "@tauri-apps/api/event";
import * as api from "./api/index";
import { FileBrowser } from "./components/file-browser";
import { ProfileForm } from "./components/profile-form";
import { ProfileSelector } from "./components/profile-selector";
import { StatusBar } from "./components/status-bar";
import { showConfirm } from "./components/dialog";
import type { UploadReadyPayload } from "./types";

const profileSelector = new ProfileSelector("profile-selector");
const statusBar = new StatusBar("status-bar");
const fileBrowser = new FileBrowser("file-browser");
const profileForm = new ProfileForm();

// Surface file browser messages in the status bar
fileBrowser.setStatusCallback((msg, isError) => {
  statusBar.set(isError ? "error" : "connected", msg);
});

// After a profile is saved, reload the selector
profileForm.onSaved(async () => {
  await profileSelector.reload();
});

// Wire profile management buttons
profileSelector.onNew(() => {
  profileForm.show();
});

profileSelector.onEdit((profile) => {
  profileForm.show(profile);
});

profileSelector.onDelete(async (profileId) => {
  const profile = profileSelector.getSelectedProfile();
  const name = profile?.name ?? profileId;
  const confirmed = await showConfirm(
    `Delete profile "${name}"? This only removes the profile, not any workspace files.`,
    "Delete Profile"
  );
  if (!confirmed) return;
  try {
    await api.deleteProfile(profileId);
    await profileSelector.reload();
  } catch (err) {
    statusBar.set("error", `Could not delete profile: ${err}`);
  }
});

// Handle confirm-mode upload: backend detected a save, ask the user
listen<UploadReadyPayload>("upload-ready", async (event) => {
  const { profile_id, local_path, remote_path } = event.payload;
  const filename = remote_path.split("/").pop() ?? remote_path;
  const confirmed = await showConfirm(
    `Upload changes to "${filename}" back to the server?\n${remote_path}`,
    "Upload File"
  );
  if (confirmed) {
    try {
      await api.uploadFile(profile_id, local_path, remote_path);
      statusBar.set("connected", `Uploaded ${filename}`);
    } catch (err) {
      statusBar.set("error", `Upload failed: ${err}`);
    }
  }
});

// Auto-upload mode: backend uploaded without asking, just show confirmation
listen<string>("upload-complete", (event) => {
  const filename = event.payload.split("/").pop() ?? event.payload;
  statusBar.set("connected", `Auto-uploaded ${filename}`);
});

// Auto-upload mode: backend upload failed, surface the error
listen<string>("upload-error", (event) => {
  statusBar.set("error", `Auto-upload failed: ${event.payload}`);
});

profileSelector.onConnect(async (profileId: string) => {
  const profile = profileSelector.getSelectedProfile();
  if (!profile) return;

  // Validate key path exists before attempting connection
  if (profile.auth_type === "key" && profile.key_path) {
    const exists = await api.checkPathExists(profile.key_path);
    if (!exists) {
      statusBar.set("error", `SSH key not found: ${profile.key_path}`);
      return;
    }
  }

  statusBar.set("connecting");

  try {
    await api.launchSsh(profileId);
    await api.saveSettings({ last_used_profile_id: profileId });
    statusBar.set("connected", `Connected to ${profile.host}`);
    fileBrowser.setProfile(profileId, profile.default_remote_path ?? "/");
    await fileBrowser.refresh();
  } catch (err) {
    statusBar.set("error", String(err));
  }
});

profileSelector.init().then((lastUsedId) => {
  if (lastUsedId) {
    const profile = profileSelector.getSelectedProfile();
    if (profile) {
      fileBrowser.setProfile(lastUsedId, profile.default_remote_path ?? "/");
    }
  }
});
