import * as api from "../api/index";
function escapeHtml(str) {
    return str
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
export class ProfileSelector {
    constructor(containerId) {
        this.profiles = [];
        this.selectedId = null;
        this.isConnected = false;
        this.onConnectCallback = null;
        this.onNewCallback = null;
        this.onEditCallback = null;
        this.onDeleteCallback = null;
        const el = document.getElementById(containerId);
        if (!el)
            throw new Error(`Element #${containerId} not found`);
        this.container = el;
    }
    async init() {
        this.profiles = await api.listProfiles();
        const settings = await api.getSettings();
        this.selectedId = settings.last_used_profile_id;
        // Fall back to first profile if last-used is absent or no longer valid
        if (!this.selectedId || !this.profiles.find((p) => p.id === this.selectedId)) {
            this.selectedId = this.profiles[0]?.id ?? null;
        }
        this.render();
        return this.selectedId;
    }
    /**
     * Reload profiles from backend and re-render.
     * If selectId is given, selects that profile.
     * If selectId is omitted and the current selection is still valid, it is preserved.
     * If the current selection no longer exists (e.g. after delete), falls back to first profile.
     * If no selection is active but profiles exist (e.g. after first-ever create), selects first.
     */
    async reload(selectId) {
        this.profiles = await api.listProfiles();
        if (selectId !== undefined) {
            // Explicit selection requested (e.g. after create/edit)
            this.selectedId = selectId;
        }
        else if (this.selectedId && !this.profiles.find((p) => p.id === this.selectedId)) {
            // Current selection was deleted — fall back to first
            this.selectedId = this.profiles[0]?.id ?? null;
        }
        else if (!this.selectedId && this.profiles.length > 0) {
            // No selection active but profiles now exist — pick first
            this.selectedId = this.profiles[0].id;
        }
        this.render();
    }
    onConnect(callback) {
        this.onConnectCallback = callback;
    }
    onNew(callback) {
        this.onNewCallback = callback;
    }
    onEdit(callback) {
        this.onEditCallback = callback;
    }
    onDelete(callback) {
        this.onDeleteCallback = callback;
    }
    getSelectedProfile() {
        return this.profiles.find((p) => p.id === this.selectedId) ?? null;
    }
    /**
     * Reflect active connection state on the Connect, Edit, and Delete buttons.
     * When connected: Connect is disabled (prevent double-connect), Edit and Delete
     * are locked to prevent modifying the active profile.
     * When disconnected: buttons are restored based on current selection state.
     */
    setConnected(isConnected) {
        this.isConnected = isConnected;
        const hasSelection = this.selectedId !== null && this.profiles.length > 0;
        const connectBtn = document.getElementById("connect-btn");
        const editBtn = document.getElementById("edit-profile-btn");
        const deleteBtn = document.getElementById("delete-profile-btn");
        if (connectBtn) {
            connectBtn.disabled = isConnected || !hasSelection;
            connectBtn.textContent = isConnected ? "Connected" : "Connect";
        }
        if (editBtn)
            editBtn.disabled = isConnected || !hasSelection;
        if (deleteBtn)
            deleteBtn.disabled = isConnected || !hasSelection;
    }
    /**
     * Update the disabled state of Edit / Delete / Connect buttons to match the
     * current selectedId without triggering a full re-render. Called after the
     * user changes the dropdown selection.
     */
    updateButtonStates() {
        const hasSelection = this.selectedId !== null && this.profiles.length > 0;
        const editBtn = document.getElementById("edit-profile-btn");
        const deleteBtn = document.getElementById("delete-profile-btn");
        const connectBtn = document.getElementById("connect-btn");
        // Edit and Delete remain locked while a session is active
        if (editBtn)
            editBtn.disabled = this.isConnected || !hasSelection;
        if (deleteBtn)
            deleteBtn.disabled = this.isConnected || !hasSelection;
        if (connectBtn)
            connectBtn.disabled = this.isConnected || !hasSelection;
    }
    render() {
        const hasProfiles = this.profiles.length > 0;
        const hasSelection = this.selectedId !== null && hasProfiles;
        const options = hasProfiles
            ? this.profiles
                .map((p) => `<option value="${escapeHtml(p.id)}" ${p.id === this.selectedId ? "selected" : ""}>${escapeHtml(p.name)}</option>`)
                .join("")
            : '<option value="">No profiles saved</option>';
        this.container.innerHTML = `
      <div class="profile-selector">
        <label for="profile-select">Profile</label>
        <select id="profile-select" ${!hasProfiles ? "disabled" : ""}>${options}</select>
        <div class="profile-mgmt-btns">
          <button class="btn-secondary" id="new-profile-btn">New</button>
          <button class="btn-secondary" id="edit-profile-btn" ${!hasSelection ? "disabled" : ""}>Edit</button>
          <button class="btn-secondary btn-danger" id="delete-profile-btn" ${!hasSelection ? "disabled" : ""}>Delete</button>
        </div>
        <button id="connect-btn" ${!hasSelection ? "disabled" : ""}>Connect</button>
      </div>
    `;
        document.getElementById("profile-select")?.addEventListener("change", (e) => {
            this.selectedId = e.target.value || null;
            // Update button states immediately — no full re-render needed
            this.updateButtonStates();
        });
        document.getElementById("new-profile-btn")?.addEventListener("click", () => {
            this.onNewCallback?.();
        });
        document.getElementById("edit-profile-btn")?.addEventListener("click", () => {
            const profile = this.getSelectedProfile();
            if (profile)
                this.onEditCallback?.(profile);
        });
        document.getElementById("delete-profile-btn")?.addEventListener("click", async () => {
            if (this.selectedId)
                await this.onDeleteCallback?.(this.selectedId);
        });
        document.getElementById("connect-btn")?.addEventListener("click", () => {
            if (this.selectedId && this.onConnectCallback) {
                this.onConnectCallback(this.selectedId);
            }
        });
    }
}
