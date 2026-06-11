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
        this.profiles = [];
        this.selectedId = null;
        this.isConnected = false;
        this.isConnecting = false;
        this.onConnectCallback = null;
        this.onNewCallback = null;
        this.onEditCallback = null;
        this.onDeleteCallback = null;
        this.collapsedGroups = new Set();
        this.sortMode = "name";
        const el = document.getElementById(containerId);
        if (!el)
            throw new Error(`Element #${containerId} not found`);
        this.container = el;
    }
    async init() {
        this.profiles = await api.listProfiles();
        const settings = await api.getSettings();
        this.selectedId = settings.last_used_profile_id;
        this.sortMode = settings.profile_sort === "created" ? "created" : "name";
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
        if (!isConnected)
            this.isConnecting = false;
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
        if (editBtn)
            editBtn.disabled = isConnected || this.isConnecting || !hasSelection;
        if (deleteBtn)
            deleteBtn.disabled = isConnected || this.isConnecting || !hasSelection;
    }
    /**
     * Lock profile actions while a connection attempt is in progress.
     * Prevents double-connect and profile mutations during "connecting".
     */
    setConnecting(isConnecting) {
        this.isConnecting = isConnecting;
        if (isConnecting)
            this.isConnected = false;
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
    /**
     * Update the disabled state of Edit / Delete / Connect buttons to match the
     * current selectedId without triggering a full re-render. Called after the
     * user changes the tree selection.
     */
    updateButtonStates() {
        const hasSelection = this.selectedId !== null && this.profiles.length > 0;
        const editBtn = document.getElementById("edit-profile-btn");
        const deleteBtn = document.getElementById("delete-profile-btn");
        const connectBtn = document.getElementById("connect-btn");
        // Edit and Delete remain locked while a session is active
        if (editBtn)
            editBtn.disabled = this.isConnected || this.isConnecting || !hasSelection;
        if (deleteBtn)
            deleteBtn.disabled = this.isConnected || this.isConnecting || !hasSelection;
        if (connectBtn) {
            connectBtn.disabled = this.isConnected || this.isConnecting || !hasSelection;
            connectBtn.textContent = this.isConnected
                ? t("profiles.connected")
                : this.isConnecting
                    ? t("profiles.connecting")
                    : t("profiles.connect");
        }
    }
    async persistSortMode() {
        try {
            const settings = await api.getSettings();
            await api.saveSettings({ ...settings, profile_sort: this.sortMode });
        }
        catch (e) {
            console.warn("[ProfileSelector] Failed to persist sort mode:", e);
        }
    }
    /** Group profiles by their `group` field; ungrouped under the empty-string key. */
    groupedProfiles() {
        const groups = new Map();
        for (const p of this.profiles) {
            const key = (p.group ?? "").trim();
            if (!groups.has(key))
                groups.set(key, []);
            groups.get(key).push(p);
        }
        for (const list of groups.values()) {
            list.sort((a, b) => {
                if (this.sortMode === "created") {
                    return (b.created_at ?? 0) - (a.created_at ?? 0); // newest first
                }
                return a.name.localeCompare(b.name);
            });
        }
        return groups;
    }
    /** Group keys in display order: named groups A–Z, ungrouped ("") last. */
    orderedGroupKeys(groups) {
        return [...groups.keys()].sort((a, b) => {
            if (a === "")
                return 1;
            if (b === "")
                return -1;
            return a.localeCompare(b);
        });
    }
    render() {
        const hasProfiles = this.profiles.length > 0;
        const hasSelection = this.selectedId !== null && hasProfiles;
        let treeHtml;
        if (!hasProfiles) {
            treeHtml = `<div class="profile-tree__empty">${t("profiles.noProfiles")}</div>`;
        }
        else {
            const groups = this.groupedProfiles();
            const keys = this.orderedGroupKeys(groups);
            treeHtml = keys
                .map((key) => {
                const list = groups.get(key);
                const label = key === "" ? t("profiles.ungrouped") : escapeHtml(key);
                const collapsed = this.collapsedGroups.has(key);
                const caret = collapsed ? "▸" : "▾";
                const rows = collapsed
                    ? ""
                    : list
                        .map((p) => `<div class="profile-row${p.id === this.selectedId ? " profile-row--selected" : ""}" data-id="${escapeHtml(p.id)}" role="option" aria-selected="${p.id === this.selectedId}" title="${escapeHtml(p.username)}@${escapeHtml(p.host)}">${escapeHtml(p.name)}</div>`)
                        .join("");
                return `
            <div class="profile-group">
              <div class="profile-group__header" data-group="${escapeHtml(key)}">
                <span class="profile-group__caret">${caret}</span>
                <span class="profile-group__name">${label}</span>
                <span class="profile-group__count">${list.length}</span>
              </div>
              <div class="profile-group__rows">${rows}</div>
            </div>`;
            })
                .join("");
        }
        const sortNameActive = this.sortMode === "name" ? " profile-sort__btn--active" : "";
        const sortCreatedActive = this.sortMode === "created" ? " profile-sort__btn--active" : "";
        this.container.innerHTML = `
      <div class="profile-selector">
        <div class="profile-selector__head">
          <label id="profile-tree-label">${t("profiles.label")}</label>
          <div class="profile-sort" role="group" aria-label="${t("profiles.sortLabel")}">
            <button class="profile-sort__btn${sortNameActive}" id="profile-sort-name" title="${t("profiles.sortName")}">${t("profiles.sortNameShort")}</button>
            <button class="profile-sort__btn${sortCreatedActive}" id="profile-sort-created" title="${t("profiles.sortCreated")}">${t("profiles.sortCreatedShort")}</button>
          </div>
        </div>
        <div class="profile-tree" id="profile-tree" role="listbox" aria-labelledby="profile-tree-label">${treeHtml}</div>
        <div class="profile-mgmt-btns">
          <button class="btn-secondary" id="new-profile-btn">${t("profiles.new")}</button>
          <button class="btn-secondary" id="edit-profile-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.edit")}</button>
          <button class="btn-secondary btn-danger" id="delete-profile-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.delete")}</button>
        </div>
        <button id="connect-btn" ${!hasSelection ? "disabled" : ""}>${t("profiles.connect")}</button>
      </div>
    `;
        this.container.querySelectorAll(".profile-group__header").forEach((h) => {
            h.addEventListener("click", () => {
                const key = h.dataset.group ?? "";
                if (this.collapsedGroups.has(key))
                    this.collapsedGroups.delete(key);
                else
                    this.collapsedGroups.add(key);
                this.render();
            });
        });
        this.container.querySelectorAll(".profile-row").forEach((row) => {
            row.addEventListener("click", () => {
                this.selectedId = row.dataset.id ?? null;
                this.updateRowSelection();
                this.updateButtonStates();
            });
            row.addEventListener("dblclick", () => {
                this.selectedId = row.dataset.id ?? null;
                if (this.selectedId && this.onConnectCallback && !this.isConnected && !this.isConnecting) {
                    this.onConnectCallback(this.selectedId);
                }
            });
        });
        document.getElementById("profile-sort-name")?.addEventListener("click", () => {
            if (this.sortMode !== "name") {
                this.sortMode = "name";
                void this.persistSortMode();
                this.render();
            }
        });
        document.getElementById("profile-sort-created")?.addEventListener("click", () => {
            if (this.sortMode !== "created") {
                this.sortMode = "created";
                void this.persistSortMode();
                this.render();
            }
        });
        document.getElementById("new-profile-btn")?.addEventListener("click", () => this.onNewCallback?.());
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
            if (this.selectedId && this.onConnectCallback)
                this.onConnectCallback(this.selectedId);
        });
    }
    /** Toggle the selected-row highlight without a full re-render. */
    updateRowSelection() {
        this.container.querySelectorAll(".profile-row").forEach((row) => {
            const sel = row.dataset.id === this.selectedId;
            row.classList.toggle("profile-row--selected", sel);
            row.setAttribute("aria-selected", String(sel));
        });
    }
}
