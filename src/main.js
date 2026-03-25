import "./styles.css";
import { listen } from "@tauri-apps/api/event";
import * as api from "./api/index";
import { FileBrowser } from "./components/file-browser";
import { ProfileForm } from "./components/profile-form";
import { ProfileSelector } from "./components/profile-selector";
import { SettingsDialog } from "./components/settings-dialog";
import { StatusBar } from "./components/status-bar";
import { showConfirm } from "./components/dialog";
import { showOverwriteDialog } from "./components/dialog";
import { showHostKeyDialog, showPasswordPrompt, showPassphrasePrompt, } from "./components/credential-dialog";
import { t, getAvailableLocales, setLocale, getLocale } from "./i18n/index";
// ── Help / About dialog ───────────────────────────────────────────────────────
async function showHelpDialog() {
    // Fetch the version from the backend (reads tauri.conf.json at build time)
    let version = "";
    try {
        version = await api.getAppVersion();
    }
    catch {
        // Non-fatal — version display is informational only
    }
    const versionLine = version
        ? `<p style="color:var(--fg-subtle);font-size:12px;">${t("app.helpVersion", { version })}</p>`
        : "";
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    overlay.innerHTML = `
    <div class="modal modal--form" role="dialog" aria-modal="true">
      <div class="modal__title">${t("app.helpTitle")}</div>
      <div class="modal__body">
        ${versionLine}
        ${t("app.helpBodyHtml")}
        <p style="margin-top:12px;">
          <a href="#" id="help-website-link" style="color:var(--accent);">${t("app.helpWebsite")}</a>
          &nbsp;·&nbsp;
          <a href="#" id="help-github-link" style="color:var(--accent);">${t("app.helpGitHub")}</a>
          &nbsp;·&nbsp;
          <a href="#" id="help-issues-link" style="color:var(--accent);">${t("app.helpReportIssue")}</a>
          &nbsp;·&nbsp;
          <a href="#" id="help-releases-link" style="color:var(--accent);">${t("app.helpReleases")}</a>
        </p>
      </div>
      <div class="modal__actions">
        <button id="help-close">${t("app.helpClose")}</button>
      </div>
    </div>
  `;
    document.body.appendChild(overlay);
    overlay.querySelector("#help-close")?.addEventListener("click", () => overlay.remove());
    overlay.querySelector("#help-website-link")?.addEventListener("click", (e) => {
        e.preventDefault();
        openExternalUrl("https://murmurssh.kai-schultka.de");
    });
    overlay.querySelector("#help-github-link")?.addEventListener("click", (e) => {
        e.preventDefault();
        openExternalUrl("https://github.com/andrenalin282/MurmurSSH");
    });
    overlay.querySelector("#help-issues-link")?.addEventListener("click", (e) => {
        e.preventDefault();
        openExternalUrl("https://github.com/andrenalin282/MurmurSSH/issues");
    });
    overlay.querySelector("#help-releases-link")?.addEventListener("click", (e) => {
        e.preventDefault();
        openExternalUrl("https://github.com/andrenalin282/MurmurSSH/releases");
    });
}
function openExternalUrl(url) {
    // Open URL via backend xdg-open (Linux-native, no additional plugin needed)
    api.openUrl(url).catch(() => {
        // Non-fatal fallback — silently ignore if xdg-open is unavailable
    });
}
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
let connectingProfileId = null;
// Surface file browser messages in the status bar
fileBrowser.setStatusCallback((msg, isError) => {
    statusBar.set(isError ? "error" : "connected", msg);
});
// Disconnect: stop SSH SSO session, clear session credentials, clean up runtime keys
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
        try {
            await api.deleteRuntimeKey(profileId);
        }
        catch {
            // Non-fatal — runtime key cleanup is best-effort
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
// ── Language button ───────────────────────────────────────────────────────────
function renderLanguageButton(container) {
    const locales = getAvailableLocales();
    const current = getLocale();
    // Build a minimal dropdown or just a status button when only one locale exists
    if (locales.length <= 1) {
        container.innerHTML = `
      <button class="btn-secondary sidebar-lang-btn" id="lang-btn"
        title="${t("language.tooltip")}" aria-label="${t("language.tooltip")}">
        ${t("app.languageBtn")}
      </button>
    `;
        // Single locale: button is informational only, no action needed
        return;
    }
    // Multiple locales: render a simple dropdown
    const options = locales
        .map((l) => `<option value="${l.key}" ${l.key === current ? "selected" : ""}>${l.label}</option>`)
        .join("");
    container.innerHTML = `
    <div class="sidebar-lang-select">
      <select id="lang-select" title="${t("language.tooltip")}" aria-label="${t("language.tooltip")}">
        ${options}
      </select>
    </div>
  `;
    container.querySelector("#lang-select")?.addEventListener("change", (e) => {
        const key = e.target.value;
        setLocale(key);
        // Reload the page to apply the new locale across all components
        window.location.reload();
    });
}
// Sidebar footer: folder, settings, language, help, donate, quit buttons
const sidebarFooter = document.getElementById("sidebar-footer");
if (sidebarFooter) {
    sidebarFooter.innerHTML = `
    <div class="sidebar-footer-btns">
      <button class="btn-secondary" id="open-folder-btn" title="${t("app.openProfilesBtnTitle")}">${t("app.openProfilesBtn")}</button>
      <button class="btn-secondary" id="settings-btn" title="${t("app.settingsBtn")}">${t("app.settingsBtn")}</button>
      <div id="lang-btn-container"></div>
    </div>
    <div class="sidebar-footer-btns sidebar-footer-btns--row2">
    <button class="btn-secondary btn-quit" id="quit-btn" title="Quit MurmurSSH" aria-label="Quit MurmurSSH">🚪</button>
      <button class="btn-secondary" id="help-btn" title="About MurmurSSH" aria-label="About MurmurSSH">❓</button>
      <a href="#" id="donate-btn" class="btn-secondary sidebar-donate-btn" title="Spend me a coffee">☕ </a>
    </div>
  `;
    const langContainer = document.getElementById("lang-btn-container");
    if (langContainer) {
        renderLanguageButton(langContainer);
    }
    document.getElementById("open-folder-btn")?.addEventListener("click", async () => {
        try {
            await api.openProfileFolder();
        }
        catch (err) {
            statusBar.set("error", t("app.cannotOpenFolder", { error: String(err) }));
        }
    });
    document.getElementById("settings-btn")?.addEventListener("click", () => {
        settingsDialog.show();
    });
    document.getElementById("help-btn")?.addEventListener("click", () => {
        showHelpDialog();
    });
    document.getElementById("donate-btn")?.addEventListener("click", (e) => {
        e.preventDefault();
        openExternalUrl("https://www.paypal.com/paypalme/kaischultka");
    });
    document.getElementById("quit-btn")?.addEventListener("click", async () => {
        try {
            await api.quitApp();
        }
        catch {
            // Fallback: if command fails, try window close
            window.close();
        }
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
    const confirmed = await showConfirm(
        t("profiles.deleteMsg", { name }),
        t("profiles.deleteTitle")
    );
    if (!confirmed)
        return;
    try {
        await api.deleteProfile(profileId);
        await profileSelector.reload();
    }
    catch (err) {
        statusBar.set("error", t("profiles.deleteFailed", { error: String(err) }));
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
            statusBar.set("connecting", t("connection.verifyingHostKey"));
            const decision = await showHostKeyDialog(host, fingerprint);
            if (decision === "cancel") {
                statusBar.set("error", t("connection.hostKeyNotTrusted"));
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
                statusBar.set("error", t("connection.failedToTrustKey", { error: String(trustErr) }));
                return false;
            }
            // Retry the same connection with the same credentials — no re-prompt.
            return verifyConnection(profileId, password, passphrase);
        }
        if (err === "NEED_PASSWORD") {
            const profile = profileSelector.getSelectedProfile();
            statusBar.set("connecting", t("connection.awaitingPassword"));
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
            statusBar.set("connecting", t("connection.awaitingPassphrase"));
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
    const confirmed = await showConfirm(
        t("app.uploadReadyMsg", { filename, remotePath: remote_path }),
        t("app.uploadFileTitle")
    );
    if (!confirmed)
        return;
    // Keep overwrite behavior consistent with all other upload paths.
    try {
        const exists = await api.remoteFileExists(profile_id, remote_path);
        if (exists) {
            const overwrite = await showOverwriteDialog(filename);
            if (overwrite.action === "cancel") {
                statusBar.set("connected", t("app.uploadCancelled"));
                return;
            }
            if (overwrite.action === "no") {
                statusBar.set("connected", t("app.uploadSkipped"));
                return;
            }
        }
    }
    catch {
        // Non-fatal: if conflict check fails, proceed with upload attempt.
    }
    try {
        await api.uploadFile(profile_id, local_path, remote_path);
        statusBar.set("connected", t("app.uploadedFile", { filename }));
    }
    catch (err) {
        statusBar.set("error", t("app.uploadFailed", { error: String(err) }));
    }
});
// Auto-upload mode: backend uploaded without asking, just show confirmation
listen("upload-complete", (event) => {
    const filename = event.payload.split("/").pop() ?? event.payload;
    statusBar.set("connected", t("app.autoUploaded", { filename }));
});
// Auto-upload mode: backend upload failed, surface the error
listen("upload-error", (event) => {
    statusBar.set("error", t("app.autoUploadFailed", { error: event.payload }));
});
profileSelector.onConnect(async (profileId) => {
    const profile = profileSelector.getSelectedProfile();
    if (!profile)
        return;
    // Prevent double-connect
    if (connectedProfileId !== null || connectingProfileId !== null)
        return;
    connectingProfileId = profileId;
    profileSelector.setConnecting(true);
    statusBar.set("connecting", t("connection.connectingStatus"));
    // Verify SFTP connection (host key + auth) before browsing
    const ok = await verifyConnection(profileId);
    if (!ok) {
        connectingProfileId = null;
        profileSelector.setConnecting(false);
        return;
    }
    // Establish SSH session for terminal SSO (non-fatal if it fails)
    try {
        await api.startSshSession(profileId);
    }
    catch {
        // Session setup failure is non-fatal — terminal will re-prompt for credentials
    }
    // Update centralized connection state
    connectedProfileId = profileId;
    connectingProfileId = null;
    profileSelector.setConnecting(false);
    profileSelector.setConnected(true);
    // Read current settings before writing to preserve profiles_path, theme, etc.
    try {
        const currentSettings = await api.getSettings();
        await api.saveSettings({ ...currentSettings, last_used_profile_id: profileId });
    }
    catch {
        // Non-fatal — last-used profile restore on next launch will just fall back to first
    }
    statusBar.set("connected", t("connection.connectedTo", { host: profile.host }));
    // Determine the initial remote path for the file browser.
    // If the profile has an explicit remote path configured, use it.
    // Otherwise, ask the SFTP server for its effective start directory (realpath("."))
    // so the browser opens at the user's actual home instead of the filesystem root.
    let startPath = profile.default_remote_path ?? "";
    if (!startPath) {
        try {
            startPath = await api.getSftpHome(profileId);
        }
        catch {
            // Non-fatal — fall back to root if realpath resolution fails
            startPath = "/";
        }
    }
    fileBrowser.setProfile(profileId, startPath, profile.local_path ?? null);
    try {
        await fileBrowser.refresh();
    }
    catch {
        // Non-fatal: browser refresh reports its own inline error.
    }
});
profileSelector.init().then(async (lastUsedId) => {
    // Clean up any leftover runtime keys from a previous session that crashed
    // or was force-killed before completing the disconnect/quit cleanup path.
    try {
        await api.cleanupRuntimeKeys();
    }
    catch {
        // Non-fatal — startup cleanup is best-effort
    }
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
