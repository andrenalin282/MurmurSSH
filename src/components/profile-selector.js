import * as api from "../api/index";
import { t } from "../i18n/index";

function escapeHtml(str) {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export class ProfileSelector {
  constructor(containerId) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;
    this.profiles = [];
    this.selectedId = null;
    this.isConnected = false;
    this.isConnecting = false;
    this.onConnectCallback = null;
    this.onNewCallback = null;
    this.onEditCallback = null;
    this.onDeleteCallback = null;
  }

  async init() {
    this.profiles = await api.listProfiles();
    const settings = await api.getSettings();
    this.selectedId = settings.last_used_profile_id;

    if (!this.selectedId || !this.profiles.find((p) => p.id === this.selectedId)) {
      this.selectedId = this.profiles[0]?.id ?? null;
    }

    this.render();
    return this.selectedId;
  }

  async reload(selectId) {
    this.profiles = await api.listProfiles();

    if (selectId !== undefined) {
      this.selectedId = selectId;
    } else if (this.selectedId && !this.profiles.find((p) => p.id === this.selectedId)) {
      this.selectedId = this.profiles[0]?.id ?? null;
    } else if (!this.selectedId && this.profiles.length > 0) {
      this.selectedId = this.profiles[0].id;
    }

    this.render();
  }

  onConnect(callback) { this.onConnectCallback = callback; }
  onNew(callback) { this.onNewCallback = callback; }
  onEdit(callback) { this.onEditCallback = callback; }
  onDelete(callback) { this.onDeleteCallback = callback; }

  getSelectedProfile() {
    return this.profiles.find((p) => p.id === this.selectedId) ?? null;
  }

  setConnected(isConnected) {
    this.isConnected = isConnected;
    if (!isConnected) this.isConnecting = false;
    const hasSelection = this.selectedId !== null && this.profiles.length > 0;
    const connectBtn = document.getElementById("connect-btn");
    const editBtn = document.getElementById("edit-profile-btn");
    const deleteBtn = document.getElementById("delete-profile-btn");

    if (connectBtn) {
      connectBtn.disabled = isConnected || this.isConnecting || !hasSelection;
      connectBtn.textContent = isConnected
        ? t("profiles.connected")
        : this.isConnecting
        ? t("profiles.connecting")
        : t("profiles.connect");
    }
    if (editBtn) editBtn.disabled = isConnected || this.isConnecting || !hasSelection;
    if (deleteBtn) deleteBtn.disabled = isConnected || this.isConnecting || !hasSelection;
  }

  setConnecting(isConnecting) {
    this.isConnecting = isConnecting;
    if (isConnecting) this.isConnected = false;
    this.updateButtonStates();
    const connectBtn = document.getElementById("connect-btn");
    if (connectBtn) {
      connectBtn.textContent = this.isConnecting
        ? t("profiles.connecting")
        : this.isConnected
        ? t("profiles.connected")
        : t("profiles.connect");
    }
  }

  updateButtonStates() {
    const hasSelection = this.selectedId !== null && this.profiles.length > 0;
    const editBtn = document.getElementById("edit-profile-btn");
    const deleteBtn = document.getElementById("delete-profile-btn");
    const connectBtn = document.getElementById("connect-btn");
    if (editBtn) editBtn.disabled = this.isConnected || this.isConnecting || !hasSelection;
    if (deleteBtn) deleteBtn.disabled = this.isConnected || this.isConnecting || !hasSelection;
    if (connectBtn) {
      connectBtn.disabled = this.isConnected || this.isConnecting || !hasSelection;
      connectBtn.textContent = this.isConnected
        ? t("profiles.connected")
        : this.isConnecting
        ? t("profiles.connecting")
        : t("profiles.connect");
    }
  }

  render() {
    const hasProfiles = this.profiles.length > 0;
    const hasSelection = this.selectedId !== null && hasProfiles;

    const options = hasProfiles
      ? this.profiles
          .map(
            (p) =>
              `<option value="${escapeHtml(p.id)}" ${p.id === this.selectedId ? "selected" : ""}>${escapeHtml(p.name)}</option>`
          )
          .join("")
      : `<option value="">${t("profiles.noProfiles")}</option>`;

    this.container.innerHTML = `
      <div class="profile-selector">
        <label for="profile-select">${t("profiles.label")}</label>
        <select id="profile-select" ${!hasProfiles ? "disabled" : ""}>${options}</select>
        <div class="profile-mgmt-btns">
          <button class="btn-secondary" id="new-profile-btn">${t("profiles.new")}</button>
          <button class="btn-secondary" id="edit-profile-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.edit")}</button>
          <button class="btn-secondary btn-danger" id="delete-profile-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.delete")}</button>
        </div>
        <button id="connect-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.connect")}</button>
      </div>
    `;

    document.getElementById("profile-select")?.addEventListener("change", (e) => {
      this.selectedId = e.target.value || null;
      this.updateButtonStates();
    });

    document.getElementById("new-profile-btn")?.addEventListener("click", () => {
      this.onNewCallback?.();
    });

    document.getElementById("edit-profile-btn")?.addEventListener("click", () => {
      const profile = this.getSelectedProfile();
      if (profile) this.onEditCallback?.(profile);
    });

    document.getElementById("delete-profile-btn")?.addEventListener("click", async () => {
      if (this.selectedId) await this.onDeleteCallback?.(this.selectedId);
    });

    document.getElementById("connect-btn")?.addEventListener("click", () => {
      if (this.selectedId && this.onConnectCallback) {
        this.onConnectCallback(this.selectedId);
      }
    });
  }
}
