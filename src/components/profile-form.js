import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api/index";
import { t } from "../i18n/index";

function escHtml(s) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function sanitizeId(name) {
  return (
    name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "profile"
  );
}

function requiresKeyPath(authType) {
  return authType === "key";
}

export class ProfileForm {
  constructor() {
    this.overlay = null;
    this.editingId = null;
    this.onSavedCallback = null;
  }

  onSaved(cb) {
    this.onSavedCallback = cb;
  }

  show(profile) {
    this.editingId = profile?.id ?? null;
    this.mount(profile);
  }

  renderSavedCredentialSection(profile) {
    const mode = profile?.credential_storage_mode;
    if (!mode || mode === "never" || profile?.auth_type !== "password") return "";

    const modeLabel =
      mode === "local_machine"
        ? t("profileForm.credLocalMachine")
        : t("profileForm.credPortable");

    return `
      <div class="form-field saved-credential-section">
        <label>${t("profileForm.labelSavedCredential")}</label>
        <div class="saved-credential-info">
          <span>${modeLabel}</span>
          <button type="button" class="btn-secondary btn-small" id="pf-clear-cred">${t("profileForm.clearCredential")}</button>
        </div>
      </div>
    `;
  }

  mount(profile) {
    const isEdit = profile !== undefined;
    const authType = profile?.auth_type ?? "key";

    this.overlay = document.createElement("div");
    this.overlay.className = "modal-overlay";
    this.overlay.innerHTML = `
      <div class="modal modal--form" role="dialog" aria-modal="true">
        <div class="modal__title">${isEdit ? t("profileForm.titleEdit") : t("profileForm.titleNew")}</div>
        <form id="pf-form">
          <div class="form-field">
            <label for="pf-name">${t("profileForm.labelName")}</label>
            <input id="pf-name" type="text" value="${escHtml(profile?.name ?? "")}"
              placeholder="${t("profileForm.placeholderName")}" autocomplete="off">
          </div>
          <div class="form-field">
            <label for="pf-host">${t("profileForm.labelHost")}</label>
            <input id="pf-host" type="text" value="${escHtml(profile?.host ?? "")}"
              placeholder="${t("profileForm.placeholderHost")}" autocomplete="off">
          </div>
          <div class="form-field">
            <label for="pf-port">${t("profileForm.labelPort")}</label>
            <input id="pf-port" type="number" value="${profile?.port ?? 22}"
              min="1" max="65535">
          </div>
          <div class="form-field">
            <label for="pf-username">${t("profileForm.labelUsername")}</label>
            <input id="pf-username" type="text" value="${escHtml(profile?.username ?? "")}"
              placeholder="user" autocomplete="off">
          </div>
          <div class="form-field">
            <label for="pf-auth-type">${t("profileForm.labelAuth")}</label>
            <select id="pf-auth-type">
              <option value="key" ${authType === "key" ? "selected" : ""}>${t("profileForm.authKey")}</option>
              <option value="agent" ${authType === "agent" ? "selected" : ""}>${t("profileForm.authAgent")}</option>
              <option value="password" ${authType === "password" ? "selected" : ""}>${t("profileForm.authPassword")}</option>
            </select>
          </div>
          <div class="form-field" id="pf-key-row"${!requiresKeyPath(authType) ? ' style="display:none"' : ""}>
            <label for="pf-key-path">${t("profileForm.labelKeyPath")}</label>
            <div class="form-field__row">
              <input id="pf-key-path" type="text" value="${escHtml(profile?.key_path ?? "")}"
                placeholder="${t("profileForm.placeholderKeyPath")}" autocomplete="off">
              <button type="button" class="btn-secondary" id="pf-browse">${t("common.browse")}</button>
            </div>
          </div>
          <div class="form-field">
            <label for="pf-remote-path">${t("profileForm.labelRemotePath")}</label>
            <input id="pf-remote-path" type="text" value="${escHtml(profile?.default_remote_path ?? "")}"
              placeholder="${t("profileForm.placeholderRemotePath")}">
          </div>
          <div class="form-field">
            <label for="pf-local-path">${t("profileForm.labelLocalPath")}</label>
            <div class="form-field__row">
              <input id="pf-local-path" type="text" value="${escHtml(profile?.local_path ?? "")}"
                placeholder="${t("profileForm.placeholderLocalPath")}">
              <button type="button" class="btn-secondary" id="pf-browse-local">${t("common.browse")}</button>
            </div>
          </div>
          <div class="form-field">
            <label for="pf-editor">${t("profileForm.labelEditor")}</label>
            <input id="pf-editor" type="text" value="${escHtml(profile?.editor_command ?? "")}"
              placeholder="${t("profileForm.placeholderEditor")}">
          </div>
          <div class="form-field">
            <label for="pf-upload-mode">${t("profileForm.labelUploadMode")}</label>
            <select id="pf-upload-mode">
              <option value="confirm" ${(profile?.upload_mode ?? "confirm") === "confirm" ? "selected" : ""}>
                ${t("profileForm.uploadConfirm")}
              </option>
              <option value="auto" ${profile?.upload_mode === "auto" ? "selected" : ""}>
                ${t("profileForm.uploadAuto")}
              </option>
            </select>
          </div>
          ${isEdit ? this.renderSavedCredentialSection(profile) : ""}
          <div class="form-error" id="pf-error" style="display:none"></div>
          <div class="modal__actions">
            <button type="button" class="btn-secondary" id="pf-cancel">${t("common.cancel")}</button>
            ${!isEdit ? `<button type="button" class="btn-secondary" id="pf-import-ssh">${t("profileForm.importSsh")}</button>` : ""}
            <button type="submit" id="pf-save">${t("common.save")}</button>
          </div>
        </form>
      </div>
    `;

    document.body.appendChild(this.overlay);

    this.overlay.querySelector("#pf-auth-type")?.addEventListener("change", (e) => {
      const val = e.target.value;
      const row = this.overlay?.querySelector("#pf-key-row");
      if (row) row.style.display = requiresKeyPath(val) ? "" : "none";
      const credSection = this.overlay?.querySelector(".saved-credential-section");
      if (credSection) credSection.style.display = val === "password" ? "" : "none";
    });

    this.overlay.querySelector("#pf-browse")?.addEventListener("click", async () => {
      const result = await open({ multiple: false, directory: false });
      if (typeof result === "string") {
        const input = this.overlay?.querySelector("#pf-key-path");
        if (input) input.value = result;
      }
    });

    this.overlay.querySelector("#pf-browse-local")?.addEventListener("click", async () => {
      const result = await open({ multiple: false, directory: true });
      if (typeof result === "string") {
        const input = this.overlay?.querySelector("#pf-local-path");
        if (input) input.value = result;
      }
    });

    this.overlay.querySelector("#pf-clear-cred")?.addEventListener("click", async () => {
      if (!this.editingId) return;
      try {
        await api.clearCredential(this.editingId);
        const updated = await api.getProfile(this.editingId);
        this.close();
        this.mount(updated);
      } catch (err) {
        this.showError(t("profileForm.errorClearFailed", { error: String(err) }));
      }
    });

    this.overlay.querySelector("#pf-cancel")?.addEventListener("click", () => this.close());
    this.overlay.querySelector("#pf-import-ssh")?.addEventListener("click", () => this.handleSshImport());

    this.overlay.querySelector("#pf-form")?.addEventListener("submit", async (e) => {
      e.preventDefault();
      await this.handleSave(isEdit);
    });
  }

  async handleSshImport() {
    const errorEl = this.overlay?.querySelector("#pf-error");

    let entries;
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
        errorEl.textContent = t("import.noEntries");
        errorEl.style.display = "";
      }
      return;
    }

    if (errorEl) errorEl.style.display = "none";

    const selected = await this.showImportModal(entries);
    if (!selected || selected.length === 0) return;

    const existing = await api.listProfiles().catch(() => []);
    const existingIds = new Set(existing.map((p) => p.id));

    let lastSavedId = null;
    let created = 0;
    const skipped = [];
    const failed = [];

    for (const entry of selected) {
      const name = entry.host;
      if (!entry.user || !entry.user.trim()) {
        skipped.push(`${name}: ${t("import.missingUsername")}`);
        continue;
      }
      const baseId = sanitizeId(name);

      let id = baseId;
      let suffix = 2;
      while (existingIds.has(id)) {
        id = `${baseId}-${suffix}`;
        suffix++;
      }

      const profile = {
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
        created++;
      } catch {
        failed.push(`${name}: ${t("import.saveFailed")}`);
      }
    }

    this.close();

    if (lastSavedId) {
      await this.onSavedCallback?.(lastSavedId);
    }

    await this.showImportResultModal(created, skipped, failed);
  }

  showImportResultModal(created, skipped, failed) {
    return new Promise((resolve) => {
      const modal = document.createElement("div");
      modal.className = "modal-overlay";

      const details = [];
      if (skipped.length > 0) {
        details.push(`<div><strong>${t("import.resultDetailsSkipped")}</strong><br>${skipped.map((s) => escHtml(s)).join("<br>")}</div>`);
      }
      if (failed.length > 0) {
        details.push(`<div style="margin-top:8px;"><strong>${t("import.resultDetailsFailed")}</strong><br>${failed.map((f) => escHtml(f)).join("<br>")}</div>`);
      }

      modal.innerHTML = `
        <div class="modal" role="dialog" aria-modal="true">
          <div class="modal__title">${t("import.resultTitle")}</div>
          <div class="modal__body">
            ${t("import.resultCreated", { count: created })}<br>
            ${t("import.resultSkipped", { count: skipped.length })}<br>
            ${t("import.resultFailed", { count: failed.length })}
            ${details.length > 0 ? `<div style="margin-top:10px">${details.join("")}</div>` : ""}
          </div>
          <div class="modal__actions">
            <button id="ssh-import-result-ok">${t("common.ok")}</button>
          </div>
        </div>
      `;

      document.body.appendChild(modal);
      modal.querySelector("#ssh-import-result-ok")?.addEventListener("click", () => {
        modal.remove();
        resolve();
      });
    });
  }

  showImportModal(entries) {
    return new Promise((resolve) => {
      const modal = document.createElement("div");
      modal.className = "modal-overlay";

      const rows = entries
        .map((entry, i) => {
          const hostDisplay = entry.hostname && entry.hostname !== entry.host
            ? `${escHtml(entry.host)} → ${escHtml(entry.hostname)}`
            : escHtml(entry.host);
          const meta = [];
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

      const introText = entries.length === 1
        ? t("import.introOne")
        : t("import.introMany", { count: entries.length });

      modal.innerHTML = `
        <div class="modal modal--form" role="dialog" aria-modal="true">
          <div class="modal__title">${t("import.modalTitle")}</div>
          <div class="modal__body ssh-import__intro">
            ${introText}
          </div>
          <div class="ssh-import__list">${rows}</div>
          <div class="modal__actions">
            <button class="btn-secondary" id="ssh-import-cancel">${t("common.cancel")}</button>
            <button id="ssh-import-confirm">${t("import.importSelected")}</button>
          </div>
        </div>
      `;

      document.body.appendChild(modal);

      const cleanup = (result) => {
        modal.remove();
        resolve(result);
      };

      modal.querySelector("#ssh-import-cancel")?.addEventListener("click", () => cleanup(null));

      modal.querySelector("#ssh-import-confirm")?.addEventListener("click", () => {
        const checked = modal.querySelectorAll('input[name="ssh-entry"]:checked');
        const selected = Array.from(checked).map((cb) => entries[parseInt(cb.value, 10)]);
        cleanup(selected);
      });
    });
  }

  async handleSave(isEdit) {
    const get = (id) =>
      this.overlay?.querySelector(`#${id}`)?.value.trim() ?? "";

    const name = get("pf-name");
    const host = get("pf-host");
    const portStr = get("pf-port");
    const username = get("pf-username");
    const authType = get("pf-auth-type");
    const keyPath = get("pf-key-path");
    const remotePath = get("pf-remote-path");
    const localPath = get("pf-local-path");
    const editorCommand = get("pf-editor");
    const uploadMode = get("pf-upload-mode");

    const errors = [];
    if (!name) errors.push(t("profileForm.errorNameRequired"));
    if (!host) errors.push(t("profileForm.errorHostRequired"));
    const port = parseInt(portStr, 10);
    if (isNaN(port) || port < 1 || port > 65535) {
      errors.push(t("profileForm.errorPortInvalid"));
    }
    if (!username) errors.push(t("profileForm.errorUsernameRequired"));
    if (authType === "key") {
      if (!keyPath) {
        errors.push(t("profileForm.errorKeyRequired"));
      } else {
        const exists = await api.checkPathExists(keyPath);
        if (!exists) errors.push(t("profileForm.errorKeyNotFound", { path: keyPath }));
      }
    }

    if (errors.length > 0) {
      this.showError(errors.join(" "));
      return;
    }

    const id = isEdit ? this.editingId : sanitizeId(name);

    if (!isEdit) {
      const existing = await api.listProfiles();
      if (existing.some((p) => p.id === id)) {
        this.showError(t("profileForm.errorProfileExists"));
        return;
      }
    }

    const existingProfile = isEdit ? await api.getProfile(id).catch(() => null) : null;

    const isAuthSwitchAwayFromPassword =
      existingProfile?.auth_type === "password" && authType !== "password";

    if (isAuthSwitchAwayFromPassword && this.editingId) {
      try {
        await api.clearCredential(this.editingId);
      } catch {
        // Non-fatal
      }
    }

    const profile = {
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

  showError(msg) {
    const el = this.overlay?.querySelector("#pf-error");
    if (el) {
      el.textContent = msg;
      el.style.display = "";
    }
  }

  close() {
    this.overlay?.remove();
    this.overlay = null;
  }
}
