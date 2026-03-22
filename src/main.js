import "./styles.css";
import { listen } from "@tauri-apps/api/event";
import * as api from "./api/index";
import { FileBrowser } from "./components/file-browser";
import { ProfileForm } from "./components/profile-form";
import { ProfileSelector } from "./components/profile-selector";
import { SettingsDialog } from "./components/settings-dialog";
import { StatusBar } from "./components/status-bar";
import { showConfirm } from "./components/dialog";
import { showHostKeyDialog, showPasswordPrompt, showPassphrasePrompt, } from "./components/credential-dialog";
let currentTheme = "system";
const systemThemeQuery = window.matchMedia("(prefers-color-scheme: dark)");
/**
 * Apply a theme by toggling the "theme-light" class on <html>.
 * Default (no class) = dark theme. "theme-light" class = light theme.
 * "system" follows the OS prefers-color-scheme media query.
 */
function applyTheme(theme) {
    currentTheme = theme;
    const isDark = theme === "dark" || (theme === "system" && systemThemeQuery.matches);
    document.documentElement.classList.toggle("theme-light", !isDark);
}
// React to OS-level theme changes when in system mode
systemThemeQuery.addEventListener("change", () => {
    if (currentTheme === "system") {
        applyTheme("system");
    }
});
const profileSelector = new ProfileSelector("profile-selector");
const statusBar = new StatusBar("status-bar");
const fileBrowser = new FileBrowser("file-browser");
const profileForm = new ProfileForm();
const settingsDialog = new SettingsDialog();
// Centralized connection state — single source of truth for connected profile.
// Updated in onConnect (after SFTP + file browser ready) and onDisconnect.
let connectedProfileId = null;
// Surface file browser messages in the status bar
fileBrowser.setStatusCallback((msg, isError) => {
    statusBar.set(isError ? "error" : "connected", msg);
});
// Disconnect: stop SSH SSO session, clear session credentials, reset connection state
fileBrowser.onDisconnect(async () => {
    const profileId = connectedProfileId;
    connectedProfileId = null;
    profileSelector.setConnected(false);
    if (profileId) {
        try {
            await api.stopSshSession(profileId);
        }
        catch {
            // Non-fatal
        }
        try {
            await api.clearSessionCredentials(profileId);
        }
        catch {
            // Non-fatal — session cache cleanup is best-effort
        }
    }
    statusBar.set("disconnected");
});
// After a profile is saved, reload the selector and select the saved profile
profileForm.onSaved(async (savedId) => {
    await profileSelector.reload(savedId);
});
// After settings are applied: apply theme immediately, then reload profiles
settingsDialog.onApplied(async (savedSettings) => {
    applyTheme(savedSettings.theme ?? "system");
    await profileSelector.reload();
});
// Sidebar footer: folder + settings buttons
const sidebarFooter = document.getElementById("sidebar-footer");
if (sidebarFooter) {
    sidebarFooter.innerHTML = `
    <div class="sidebar-footer-btns">
      <button class="btn-secondary" id="open-folder-btn" title="Open profile folder">📁 Profiles</button>
      <button class="btn-secondary" id="settings-btn" title="Settings">⚙ Settings</button>
    </div>
  `;
    document.getElementById("open-folder-btn")?.addEventListener("click", async () => {
        try {
            await api.openProfileFolder();
        }
        catch (err) {
            statusBar.set("error", `Cannot open folder: ${err}`);
        }
    });
    document.getElementById("settings-btn")?.addEventListener("click", () => {
        settingsDialog.show();
    });
}
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
    const confirmed = await showConfirm(`Delete profile "${name}"? This only removes the profile, not any workspace files.`, "Delete Profile");
    if (!confirmed)
        return;
    try {
        await api.deleteProfile(profileId);
        await profileSelector.reload();
    }
    catch (err) {
        statusBar.set("error", `Could not delete profile: ${err}`);
    }
});
/**
 * Attempt to verify the SFTP connection for the given profile.
 * Handles host key verification and credential prompts through in-app dialogs.
 * Returns true on success, false if the user cancelled or connection failed.
 *
 * Note: SSH terminal sessions are launched separately and handle their own auth
 * interactively in the terminal window.
 */
async function verifyConnection(profileId, password, passphrase) {
    try {
        await api.connectSftp(profileId, password, passphrase);
        return true;
    }
    catch (rawErr) {
        const err = String(rawErr);
        if (err.startsWith("UNKNOWN_HOST:")) {
            const fingerprint = err.slice("UNKNOWN_HOST:".length);
            const profile = profileSelector.getSelectedProfile();
            const host = profile?.host ?? profileId;
            statusBar.set("connecting", "Verifying host key…");
            const decision = await showHostKeyDialog(host, fingerprint);
            if (decision === "cancel") {
                statusBar.set("error", "Connection cancelled: host key not trusted.");
                return false;
            }
            // Record trust — either session-only or persistently in known_hosts
            try {
                if (decision === "accept_save") {
                    await api.acceptHostKey(profileId, fingerprint);
                }
                else {
                    // accept_once: in-memory only, no disk write
                    await api.acceptHostKeyOnce(profileId, fingerprint);
                }
            }
            catch (trustErr) {
                statusBar.set("error", `Failed to trust host key: ${trustErr}`);
                return false;
            }
            // Retry the same connection with the same credentials — no re-prompt.
            return verifyConnection(profileId, password, passphrase);
        }
        if (err === "NEED_PASSWORD") {
            const profile = profileSelector.getSelectedProfile();
            statusBar.set("connecting", "Awaiting password…");
            const result = await showPasswordPrompt(profile?.username ?? "", profile?.host ?? profileId);
            if (result === null) {
                statusBar.set("disconnected");
                return false;
            }
            const ok = await verifyConnection(profileId, result.secret, passphrase);
            if (ok && result.saveMode !== "never") {
                try {
                    await api.saveCredential(profileId, result.secret, result.saveMode);
                }
                catch {
                    // Non-fatal — credential save failure doesn't break the connection
                }
            }
            return ok;
        }
        if (err === "NEED_PASSPHRASE") {
            const profile = profileSelector.getSelectedProfile();
            statusBar.set("connecting", "Awaiting passphrase…");
            // showPassphrasePrompt returns string | null — no save mode, passphrases are runtime-only
            const pp = await showPassphrasePrompt(profile?.key_path ?? "SSH key");
            if (pp === null) {
                statusBar.set("disconnected");
                return false;
            }
            // Passphrases are never saved — pass to connect and discard after use
            return verifyConnection(profileId, password, pp);
        }
        // All other errors — display and stop
        statusBar.set("error", err);
        return false;
    }
}
// Handle confirm-mode upload: backend detected a save, ask the user
listen("upload-ready", async (event) => {
    const { profile_id, local_path, remote_path } = event.payload;
    const filename = remote_path.split("/").pop() ?? remote_path;
    const confirmed = await showConfirm(`Upload changes to "${filename}" back to the server?\n${remote_path}`, "Upload File");
    if (confirmed) {
        try {
            await api.uploadFile(profile_id, local_path, remote_path);
            statusBar.set("connected", `Uploaded ${filename}`);
        }
        catch (err) {
            statusBar.set("error", `Upload failed: ${err}`);
        }
    }
});
// Auto-upload mode: backend uploaded without asking, just show confirmation
listen("upload-complete", (event) => {
    const filename = event.payload.split("/").pop() ?? event.payload;
    statusBar.set("connected", `Auto-uploaded ${filename}`);
});
// Auto-upload mode: backend upload failed, surface the error
listen("upload-error", (event) => {
    statusBar.set("error", `Auto-upload failed: ${event.payload}`);
});
profileSelector.onConnect(async (profileId) => {
    const profile = profileSelector.getSelectedProfile();
    if (!profile)
        return;
    // Prevent double-connect
    if (connectedProfileId !== null)
        return;
    statusBar.set("connecting", "Connecting…");
    // Verify SFTP connection (host key + auth) before browsing
    const ok = await verifyConnection(profileId);
    if (!ok)
        return;
    // Establish SSH session for terminal SSO (non-fatal if it fails)
    try {
        await api.startSshSession(profileId);
    }
    catch {
        // Session setup failure is non-fatal — terminal will re-prompt for credentials
    }
    // Update centralized connection state
    connectedProfileId = profileId;
    profileSelector.setConnected(true);
    // Read current settings before writing to preserve profiles_path, theme, etc.
    try {
        const currentSettings = await api.getSettings();
        await api.saveSettings({ ...currentSettings, last_used_profile_id: profileId });
    }
    catch {
        // Non-fatal — last-used profile restore on next launch will just fall back to first
    }
    statusBar.set("connected", `Connected to ${profile.host}`);
    fileBrowser.setProfile(profileId, profile.default_remote_path ?? "/", profile.local_path ?? null);
    await fileBrowser.refresh();
});
profileSelector.init().then(async (lastUsedId) => {
    // Load and apply persisted theme on startup
    try {
        const settings = await api.getSettings();
        applyTheme(settings.theme ?? "system");
    }
    catch {
        // Non-fatal — default theme (dark) stays active
    }
    if (lastUsedId) {
        const profile = profileSelector.getSelectedProfile();
        if (profile) {
            fileBrowser.setProfile(lastUsedId, profile.default_remote_path ?? "/", profile.local_path ?? null);
        }
    }
});
