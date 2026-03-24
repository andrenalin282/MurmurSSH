import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api/index";
import type { SshConfigEntry } from "../api/index";
import type { AuthType, Profile, UploadMode } from "../types";

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function sanitizeId(name: string): string {
  return (
    name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "profile"
  );
}

/** Returns true if the given auth type requires a key path */
function requiresKeyPath(authType: AuthType): boolean {
  return authType === "key";
}

export class ProfileForm {
  private overlay: HTMLElement | null = null;
  private editingId: string | null = null;
  private onSavedCallback: ((savedId: string) => Promise<void>) | null = null;

  onSaved(cb: (savedId: string) => Promise<void>): void {
    this.onSavedCallback = cb;
  }

  show(profile?: Profile): void {
    this.editingId = profile?.id ?? null;
    this.mount(profile);
  }

  private renderSavedCredentialSection(profile?: Profile): string {
    const mode = profile?.credential_storage_mode;
    // Only password auth profiles can have a stored credential. SSH key passphrases
    // are never saved, so there is nothing to show or clear for key/agent auth.
    if (!mode || mode === "never" || profile?.auth_type !== "password") return "";

    const modeLabel =
      mode === "local_machine"
        ? "Saved on this PC (local machine file)"
        : "Saved in profile file — <strong>less secure / portable</strong>";

    return `
      <div class="form-field saved-credential-section">
        <label>Saved Credential</label>
        <div class="saved-credential-info">
          <span>${modeLabel}</span>
          <button type="button" class="btn-secondary btn-small" id="pf-clear-cred">Clear</button>
        </div>
      </div>
    `;
  }

  private mount(profile?: Profile): void {
    const isEdit = profile !== undefined;
    const authType: AuthType = profile?.auth_type ?? "key";

    this.overlay = document.createElement("div");
    this.overlay.className = "modal-overlay";
    this.overlay.innerHTML = `
      <div class="modal modal--form" role="dialog" aria-modal="true">
        <div class="modal__title">${isEdit ? "Edit Profile" : "New Profile"}</div>
        <form id="pf-form">
          <div class="form-field">
            <label for="pf-name">Display Name *</label>
            <input id="pf-name" type="text" value="${escHtml(profile?.name ?? "")}"
              placeholder="My Server" autocomplete="off">
          </div>
          <div class="form-field">
            <label for="pf-host">Host *</label>
            <input id="pf-host" type="text" value="${escHtml(profile?.host ?? "")}"
              placeholder="192.168.1.100" autocomplete="off">
          </div>
          <div class="form-field">
            <label for="pf-port">Port *</label>
            <input id="pf-port" type="number" value="${profile?.port ?? 22}"
              min="1" max="65535">
          </div>
          <div class="form-field">
            <label for="pf-username">Username *</label>
            <input id="pf-username" type="text" value="${escHtml(profile?.username ?? "")}"
              placeholder="user" autocomplete="off">
          </div>
          <div class="form-field">
            <label for="pf-auth-type">Authentication</label>
            <select id="pf-auth-type">
              <option value="key" ${authType === "key" ? "selected" : ""}>SSH Key</option>
              <option value="agent" ${authType === "agent" ? "selected" : ""}>SSH Agent</option>
              <option value="password" ${authType === "password" ? "selected" : ""}>Password</option>
            </select>
          </div>
          <div class="form-field" id="pf-key-row"${!requiresKeyPath(authType) ? ' style="display:none"' : ""}>
            <label for="pf-key-path">Private Key Path *</label>
            <div class="form-field__row">
              <input id="pf-key-path" type="text" value="${escHtml(profile?.key_path ?? "")}"
                placeholder="/home/user/.ssh/id_ed25519" autocomplete="off">
              <button type="button" class="btn-secondary" id="pf-browse">Browse…</button>
            </div>
          </div>
          <div class="form-field">
            <label for="pf-remote-path">Default Remote Path</label>
            <input id="pf-remote-path" type="text" value="${escHtml(profile?.default_remote_path ?? "")}"
              placeholder="/home/user">
          </div>
          <div class="form-field">
            <label for="pf-local-path">Local Path</label>
            <div class="form-field__row">
              <input id="pf-local-path" type="text" value="${escHtml(profile?.local_path ?? "")}"
                placeholder="/home/user/projects  (optional)">
              <button type="button" class="btn-secondary" id="pf-browse-local">Browse…</button>
            </div>
          </div>
          <div class="form-field">
            <label for="pf-editor">Editor Command</label>
            <input id="pf-editor" type="text" value="${escHtml(profile?.editor_command ?? "")}"
              placeholder="code  (blank = system default)">
          </div>
          <div class="form-field">
            <label for="pf-upload-mode">Upload Mode</label>
            <select id="pf-upload-mode">
              <option value="confirm" ${(profile?.upload_mode ?? "confirm") === "confirm" ? "selected" : ""}>
                Confirm before upload
              </option>
              <option value="auto" ${profile?.upload_mode === "auto" ? "selected" : ""}>
                Auto-upload on save
              </option>
            </select>
          </div>
          ${isEdit ? this.renderSavedCredentialSection(profile) : ""}
          <div class="form-error" id="pf-error" style="display:none"></div>
          <div class="modal__actions">
            <button type="button" class="btn-secondary" id="pf-cancel">Cancel</button>
            ${!isEdit ? '<button type="button" class="btn-secondary" id="pf-import-ssh">Import from SSH config…</button>' : ""}
            <button type="submit" id="pf-save">Save</button>
          </div>
        </form>
      </div>
    `;

    document.body.appendChild(this.overlay);

    // Auth type toggle — show/hide key path row; hide saved credential section when
    // switching away from password (the actual cleanup happens on save, but hiding
    // it immediately prevents confusing UX while the form is open).
    this.overlay.querySelector("#pf-auth-type")?.addEventListener("change", (e) => {
      const val = (e.target as HTMLSelectElement).value as AuthType;
      const row = this.overlay?.querySelector<HTMLElement>("#pf-key-row");
      if (row) row.style.display = requiresKeyPath(val) ? "" : "none";
      const credSection = this.overlay?.querySelector<HTMLElement>(".saved-credential-section");
      if (credSection) credSection.style.display = val === "password" ? "" : "none";
    });

    // File picker for private key
    this.overlay.querySelector("#pf-browse")?.addEventListener("click", async () => {
      const result = await open({ multiple: false, directory: false });
      if (typeof result === "string") {
        const input = this.overlay?.querySelector<HTMLInputElement>("#pf-key-path");
        if (input) input.value = result;
      }
    });

    // Folder picker for local working directory
    this.overlay.querySelector("#pf-browse-local")?.addEventListener("click", async () => {
      const result = await open({ multiple: false, directory: true });
      if (typeof result === "string") {
        const input = this.overlay?.querySelector<HTMLInputElement>("#pf-local-path");
        if (input) input.value = result;
      }
    });

    // Clear saved credential button (edit mode only, shown when a credential is stored)
    this.overlay.querySelector("#pf-clear-cred")?.addEventListener("click", async () => {
      if (!this.editingId) return;
      try {
        await api.clearCredential(this.editingId);
        // Re-render the form with the updated (credential-cleared) profile
        const updated = await api.getProfile(this.editingId);
        this.close();
        this.mount(updated);
      } catch (err) {
        this.showError(`Failed to clear credential: ${err}`);
      }
    });

    this.overlay.querySelector("#pf-cancel")?.addEventListener("click", () => this.close());

    // SSH config import (new profile mode only)
    this.overlay.querySelector("#pf-import-ssh")?.addEventListener("click", () => this.handleSshImport());

    this.overlay.querySelector("#pf-form")?.addEventListener("submit", async (e) => {
      e.preventDefault();
      await this.handleSave(isEdit);
    });
  }

  // ── SSH Config Import ────────────────────────────────────────────────────

  private async handleSshImport(): Promise<void> {
    const errorEl = this.overlay?.querySelector<HTMLElement>("#pf-error");

    let entries: SshConfigEntry[];
    try {
      entries = await api.parseSshConfig();
    } catch (err) {
      if (errorEl) {
        errorEl.textContent = String(err);
        errorEl.style.display = "";
      }
      return;
    }

    if (entries.length === 0) {
      if (errorEl) {
        errorEl.textContent = "No importable host entries found in ~/.ssh/config.";
        errorEl.style.display = "";
      }
      return;
    }

    // Hide any previous error while the import modal is open
    if (errorEl) errorEl.style.display = "none";

    const selected = await this.showImportModal(entries);
    if (!selected || selected.length === 0) return;

    // Gather existing profile IDs to avoid duplicates
    const existing = await api.listProfiles().catch(() => [] as { id: string }[]);
    const existingIds = new Set(existing.map((p) => p.id));

    let lastSavedId: string | null = null;
    //const skipped: string[] = [];
    const failed: string[] = [];

    for (const entry of selected) {
      const name = entry.host;
      const baseId = sanitizeId(name);

      // Find a unique ID (suffix -2, -3, ... if needed)
      let id = baseId;
      let suffix = 2;
      while (existingIds.has(id)) {
        id = `${baseId}-${suffix}`;
        suffix++;
      }

      const profile: Profile = {
        id,
        name,
        host: entry.hostname ?? entry.host,
        port: entry.port ?? 22,
        username: entry.user ?? "",
        auth_type: entry.identity_file ? "key" : "agent",
        key_path: entry.identity_file ?? null,
        default_remote_path: null,
        local_path: null,
        editor_command: null,
        upload_mode: "confirm",
      };

      try {
        await api.saveProfile(profile);
        existingIds.add(id);
        lastSavedId = id;
      } catch {
        failed.push(name);
      }
    }

    this.close();

    if (lastSavedId) {
      await this.onSavedCallback?.(lastSavedId);
    }

    // Surface errors after closing (status bar or alert)
    if (failed.length > 0) {
      // Non-critical: just log — the modal is gone
      console.warn("SSH import: failed to save profiles for:", failed);
    }
  }

  /**
   * Show a modal listing SSH config entries with checkboxes.
   * Returns the selected entries, or null/[] if cancelled.
   */
  private showImportModal(entries: SshConfigEntry[]): Promise<SshConfigEntry[] | null> {
    return new Promise((resolve) => {
      const modal = document.createElement("div");
      modal.className = "modal-overlay";

      const rows = entries
        .map((entry, i) => {
          const hostDisplay = entry.hostname && entry.hostname !== entry.host
            ? `${escHtml(entry.host)} → ${escHtml(entry.hostname)}`
            : escHtml(entry.host);
          const meta: string[] = [];
          if (entry.user) meta.push(`user: ${escHtml(entry.user)}`);
          if (entry.port && entry.port !== 22) meta.push(`port: ${entry.port}`);
          if (entry.identity_file) meta.push("key auth");
          return `
            <label class="ssh-import__row">
              <input type="checkbox" name="ssh-entry" value="${i}" checked>
              <span class="ssh-import__host">${hostDisplay}</span>
              ${meta.length > 0 ? `<span class="ssh-import__meta">${meta.join(" · ")}</span>` : ""}
            </label>`;
        })
        .join("");

      modal.innerHTML = `
        <div class="modal modal--form" role="dialog" aria-modal="true">
          <div class="modal__title">Import from SSH Config</div>
          <div class="modal__body ssh-import__intro">
            Found ${entries.length} host ${entries.length === 1 ? "entry" : "entries"} in ~/.ssh/config.
            Select the profiles to create:
          </div>
          <div class="ssh-import__list">${rows}</div>
          <div class="modal__actions">
            <button class="btn-secondary" id="ssh-import-cancel">Cancel</button>
            <button id="ssh-import-confirm">Import Selected</button>
          </div>
        </div>
      `;

      document.body.appendChild(modal);

      const cleanup = (result: SshConfigEntry[] | null) => {
        modal.remove();
        resolve(result);
      };

      modal.querySelector("#ssh-import-cancel")?.addEventListener("click", () => cleanup(null));

      modal.querySelector("#ssh-import-confirm")?.addEventListener("click", () => {
        const checked = modal.querySelectorAll<HTMLInputElement>('input[name="ssh-entry"]:checked');
        const selected = Array.from(checked).map((cb) => entries[parseInt(cb.value, 10)]);
        cleanup(selected);
      });
    });
  }

  private async handleSave(isEdit: boolean): Promise<void> {
    const get = (id: string) =>
      this.overlay?.querySelector<HTMLInputElement | HTMLSelectElement>(`#${id}`)?.value.trim() ?? "";

    const name = get("pf-name");
    const host = get("pf-host");
    const portStr = get("pf-port");
    const username = get("pf-username");
    const authType = get("pf-auth-type") as AuthType;
    const keyPath = get("pf-key-path");
    const remotePath = get("pf-remote-path");
    const localPath = get("pf-local-path");
    const editorCommand = get("pf-editor");
    const uploadMode = get("pf-upload-mode") as UploadMode;

    // Validate
    const errors: string[] = [];
    if (!name) errors.push("Display name is required.");
    if (!host) errors.push("Host is required.");
    const port = parseInt(portStr, 10);
    if (isNaN(port) || port < 1 || port > 65535) {
      errors.push("Port must be a number between 1 and 65535.");
    }
    if (!username) errors.push("Username is required.");
    if (authType === "key") {
      if (!keyPath) {
        errors.push("Private key path is required for key authentication.");
      } else {
        const exists = await api.checkPathExists(keyPath);
        if (!exists) errors.push(`Key file not found: ${keyPath}`);
      }
    }

    if (errors.length > 0) {
      this.showError(errors.join(" "));
      return;
    }

    const id = isEdit ? this.editingId! : sanitizeId(name);

    // Check for ID conflict when creating a new profile
    if (!isEdit) {
      const existing = await api.listProfiles();
      if (existing.some((p) => p.id === id)) {
        this.showError("A profile with a similar name already exists. Choose a different display name.");
        return;
      }
    }

    // When editing, preserve existing credential storage fields so they aren't wiped.
    // These are managed via the Clear button and at connect time.
    const existingProfile = isEdit ? await api.getProfile(id).catch(() => null) : null;

    // If auth type is switching away from password, clear any stored password credential.
    // Passphrases are never saved, so a switch in the other direction needs no cleanup.
    const isAuthSwitchAwayFromPassword =
      existingProfile?.auth_type === "password" && authType !== "password";

    if (isAuthSwitchAwayFromPassword && this.editingId) {
      try {
        await api.clearCredential(this.editingId);
      } catch {
        // Non-fatal — backend also does this cleanup in save_profile
      }
    }

    const profile: Profile = {
      id,
      name,
      host,
      port,
      username,
      auth_type: authType,
      key_path: authType === "key" ? keyPath : null,
      default_remote_path: remotePath || null,
      local_path: localPath || null,
      editor_command: editorCommand || null,
      upload_mode: uploadMode,
      // Carry forward credential fields only when NOT switching away from password auth.
      // When switching away, these fields must be cleared so the saved profile is clean.
      credential_storage_mode: isAuthSwitchAwayFromPassword
        ? undefined
        : existingProfile?.credential_storage_mode,
      stored_secret_portable: isAuthSwitchAwayFromPassword
        ? undefined
        : existingProfile?.stored_secret_portable,
    };

    try {
      await api.saveProfile(profile);
      this.close();
      await this.onSavedCallback?.(id);
    } catch (err) {
      this.showError(String(err));
    }
  }

  private showError(msg: string): void {
    const el = this.overlay?.querySelector<HTMLElement>("#pf-error");
    if (el) {
      el.textContent = msg;
      el.style.display = "";
    }
  }

  private close(): void {
    this.overlay?.remove();
    this.overlay = null;
  }
}
