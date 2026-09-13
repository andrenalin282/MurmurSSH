import * as api from "../api/index";
import { t } from "../i18n/index";
import { setDragSource, getDragSource, clearDragSource } from "../dnd-state";
import { clickSelect, moveCursor } from "./list-selection";
import { showPrompt, showConfirm } from "./dialog";
import { matchShortcut } from "../shortcuts";
import { getActivePanel } from "../panel-focus";
// ── Helpers ────────────────────────────────────────────────────────────────────
function escHtml(s) {
    return s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
function formatBytes(bytes) {
    if (bytes < 1024)
        return `${bytes} B`;
    if (bytes < 1024 * 1024)
        return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
function joinPath(dir, name) {
    return dir.replace(/\/?$/, "/") + name;
}
function parentPath(path) {
    if (path === "/")
        return "/";
    const parts = path.split("/").filter((p) => p.length > 0);
    parts.pop();
    return parts.length === 0 ? "/" : "/" + parts.join("/");
}
// ── Inline SVG icons (14×14, Lucide-style, currentColor) ──────────────────────
const ICONS = {
    up: `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/></svg>`,
    home: `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>`,
    refresh: `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/></svg>`,
};
// ── Component ──────────────────────────────────────────────────────────────────
export class LocalFileBrowser {
    constructor(containerId) {
        this.profileId = null;
        this.homePath = "";
        this.currentPath = "";
        this.entries = [];
        this.busy = false;
        this.isDragOver = false; // drop-from-remote indicator
        this.inlineError = null;
        // Drag-from-local state
        this.dragSourceNames = new Set();
        // Selection state (multi-select + keyboard cursor)
        this.selectedNames = new Set();
        this.anchorName = null;
        this.cursorName = null;
        // Callbacks wired from main.ts
        this.onDownloadCallback = null;
        this.onUploadCallback = null;
        this.onPathChange = null;
        // Editor command from the connected profile (optional)
        this.editorCommand = null;
        // Context menu
        this._contextMenu = null;
        this._hideContextMenuBound = (e) => this._hideContextMenu(e);
        // Type-ahead
        this.typeAheadBuffer = "";
        this.typeAheadTimer = null;
        const el = document.getElementById(containerId);
        if (!el)
            throw new Error(`Element #${containerId} not found`);
        this.container = el;
        this.renderEmpty();
        this.setupKeyboardShortcuts();
    }
    // ── Public API ───────────────────────────────────────────────────────────────
    /** Called when a profile connects. Loads the saved path or $HOME. */
    async setProfile(profileId) {
        this.profileId = profileId;
        try {
            this.homePath = await api.getHomeDir();
            this.currentPath = await api.getLocalBrowserPath(profileId);
        }
        catch {
            this.currentPath = await api.getHomeDir().catch(() => "/");
            this.homePath = this.currentPath;
        }
        await this.refresh();
    }
    /** Clear state on disconnect. */
    clear() {
        this.profileId = null;
        this.currentPath = "";
        this.homePath = "";
        this.entries = [];
        this.busy = false;
        this.isDragOver = false;
        this.inlineError = null;
        this.selectedNames = new Set();
        this.anchorName = null;
        this.cursorName = null;
        this.renderEmpty();
    }
    /** The current local directory. Used by main.ts to save path on disconnect. */
    getCurrentPath() {
        return this.currentPath;
    }
    /**
     * Register a callback that is invoked when the user drops remote files onto the local browser
     * (i.e. the local browser receives a "download here" drop).
     */
    onDownload(cb) {
        this.onDownloadCallback = cb;
    }
    /** Notify the local browser that a download is happening (disable drag target). */
    setBusy(value) {
        this.busy = value;
    }
    /** Register a callback fired whenever the user navigates to a new path. */
    onPathChanged(cb) {
        this.onPathChange = cb;
    }
    /** Register a callback invoked when the user chooses "Upload to remote" from the context menu. */
    onUpload(cb) {
        this.onUploadCallback = cb;
    }
    /** Set the editor command from the connected profile (may be null/empty). */
    setEditorCommand(cmd) {
        this.editorCommand = cmd || null;
    }
    // ── Rendering ────────────────────────────────────────────────────────────────
    renderEmpty() {
        this.container.innerHTML = `
      <div class="local-browser local-browser--empty">
        <div class="local-browser__toolbar">
          <button disabled title="${t("localBrowser.up")}">${ICONS.up}</button>
          <button disabled title="${t("localBrowser.home")}">${ICONS.home}</button>
          <button disabled title="${t("localBrowser.refresh")}">${ICONS.refresh}</button>
        </div>
        <p class="local-browser__prompt">${t("localBrowser.notConnected")}</p>
      </div>
    `;
    }
    async refresh() {
        if (!this.profileId || !this.currentPath)
            return;
        this.busy = true;
        try {
            this.entries = await api.listLocalDirectory(this.currentPath);
            this.inlineError = null;
            this.selectedNames = new Set();
            this.anchorName = null;
            this.cursorName = null;
        }
        catch (err) {
            this.inlineError = String(err);
        }
        finally {
            this.busy = false;
        }
        this.render();
    }
    render() {
        if (!this.profileId) {
            this.renderEmpty();
            return;
        }
        const isAtRoot = this.currentPath === "/";
        const hasEntries = this.entries.length > 0;
        const upRow = isAtRoot
            ? ""
            : `<tr class="lb-entry lb-entry--dir lb-entry--up" data-name=".." data-isdir="true" data-path="${escHtml(parentPath(this.currentPath))}">
           <td colspan="2">.. (up)</td>
         </tr>`;
        const rows = !hasEntries && !this.inlineError
            ? `<tr><td colspan="2" class="empty-dir">${t("localBrowser.emptyDir")}</td></tr>`
            : this.entries
                .map((entry) => {
                const fullPath = joinPath(this.currentPath, entry.name);
                const selected = this.selectedNames.has(entry.name) ? " lb-entry--selected" : "";
                return `<tr class="lb-entry${entry.is_dir ? " lb-entry--dir" : ""}${selected}" draggable="true" data-name="${escHtml(entry.name)}" data-isdir="${entry.is_dir}" data-path="${escHtml(fullPath)}">
              <td>${entry.is_dir ? "&#128193; " : ""}${escHtml(entry.name)}</td>
              <td>${entry.size != null && !entry.is_dir ? formatBytes(entry.size) : "—"}</td>
            </tr>`;
            })
                .join("");
        const inlineErrorHtml = this.inlineError
            ? `<div class="local-browser__inline-error">${escHtml(this.inlineError)}</div>`
            : "";
        this.container.innerHTML = `
      <div class="local-browser${this.isDragOver ? " local-browser--dragover" : ""}">
        <div class="local-browser__toolbar">
          <button id="lb-up-btn"      ${isAtRoot || this.busy ? "disabled" : ""} title="${t("localBrowser.up")}">${ICONS.up}</button>
          <button id="lb-home-btn"    ${this.busy ? "disabled" : ""} title="${t("localBrowser.home")}">${ICONS.home}</button>
          <button id="lb-refresh-btn" ${this.busy ? "disabled" : ""} title="${t("localBrowser.refresh")}">${ICONS.refresh}</button>
        </div>
        <div class="local-browser__path-row">
          <input id="lb-path-input" type="text" class="local-browser__path-input"
            value="${escHtml(this.currentPath)}" spellcheck="false" autocomplete="off">
        </div>
        ${inlineErrorHtml}
        <div class="local-browser__scroll">
          <table class="local-browser__table">
            <thead>
              <tr><th>${t("localBrowser.columnName")}</th><th>${t("localBrowser.columnSize")}</th></tr>
            </thead>
            <tbody>${upRow}${rows}</tbody>
          </table>
        </div>
        <div class="local-browser__drop-hint${this.isDragOver ? " local-browser__drop-hint--active" : ""}">
          ${t("localBrowser.dragToDownload")}
        </div>
      </div>
    `;
        this.wireEvents();
    }
    // ── Context menu ─────────────────────────────────────────────────────────────
    _showContextMenu(x, y, name, isDir) {
        this._hideContextMenu();
        const isFile = !isDir;
        const menu = document.createElement("div");
        menu.className = "lb-context-menu";
        menu.innerHTML = `
      ${isFile ? `<button data-action="open">${t("localBrowser.ctxOpen")}</button>` : ""}
      ${isFile ? `<button data-action="edit">${t("localBrowser.ctxEdit")}</button>` : ""}
      ${this.onUploadCallback ? `<button data-action="upload">${t("localBrowser.ctxUpload")}</button>` : ""}
      <button data-action="rename">${t("localBrowser.ctxRename")}</button>
      <button data-action="newFolder">${t("localBrowser.ctxNewFolder")}</button>
      <button data-action="delete">${t("localBrowser.ctxDelete")}</button>
    `;
        document.body.appendChild(menu);
        // Clamp to viewport
        const rect = menu.getBoundingClientRect();
        menu.style.left = `${Math.min(x, window.innerWidth - rect.width - 4)}px`;
        menu.style.top = `${Math.min(y, window.innerHeight - rect.height - 4)}px`;
        this._contextMenu = menu;
        menu.addEventListener("click", (e) => {
            const btn = e.target.closest("[data-action]");
            if (!btn)
                return;
            const action = btn.dataset.action;
            this._hideContextMenu();
            const path = joinPath(this.currentPath, name);
            if (action === "open")
                void this._ctxOpen(path, null, false);
            else if (action === "edit")
                void this._ctxOpen(path, this.editorCommand, true);
            else if (action === "upload")
                void this._ctxUpload(path, name);
            else if (action === "rename")
                void this._ctxRename(name);
            else if (action === "newFolder")
                void this.createFolder();
            else if (action === "delete") {
                this.applySelection({ selected: new Set([name]), anchor: name, cursor: name });
                void this.deleteSelected();
            }
        });
        setTimeout(() => {
            document.addEventListener("mousedown", this._hideContextMenuBound, { once: true });
        }, 0);
    }
    _hideContextMenu(e) {
        if (e && this._contextMenu?.contains(e.target))
            return;
        this._contextMenu?.remove();
        this._contextMenu = null;
    }
    async _ctxOpen(path, editor, useConfiguredEditor) {
        try {
            await api.openLocalFile(path, editor, useConfiguredEditor);
        }
        catch (err) {
            this.inlineError = t("localBrowser.openFailed", { error: String(err) });
            this.render();
        }
    }
    async _ctxUpload(path, name) {
        if (!this.onUploadCallback)
            return;
        await this.onUploadCallback([path], name);
    }
    async _ctxRename(oldName) {
        const newName = await showPrompt(t("localBrowser.renameTitle"), "", oldName);
        if (!newName || newName === oldName)
            return;
        if (newName.includes("/")) {
            this.inlineError = t("localBrowser.renameFailed", { error: "Name cannot contain \"/\"" });
            this.render();
            return;
        }
        const fromPath = joinPath(this.currentPath, oldName);
        const toPath = joinPath(this.currentPath, newName);
        try {
            await api.renameLocalFile(fromPath, toPath);
            await this.refresh();
        }
        catch (err) {
            this.inlineError = t("localBrowser.renameFailed", { error: String(err) });
            this.render();
        }
    }
    async createFolder() {
        const name = await showPrompt(t("localBrowser.newFolderTitle"), t("localBrowser.newFolderPlaceholder"));
        if (!name)
            return;
        if (name.includes("/")) {
            this.inlineError = t("localBrowser.nameContainsSlash");
            this.render();
            return;
        }
        try {
            await api.createLocalDir(joinPath(this.currentPath, name));
            await this.refresh();
        }
        catch (err) {
            this.inlineError = t("localBrowser.createFolderFailed", { error: String(err) });
            this.render();
        }
    }
    async deleteSelected() {
        const paths = this.getSelectedPaths();
        if (paths.length === 0)
            return;
        const label = paths.length === 1 ? paths[0] : t("localBrowser.itemsLabel", { count: paths.length });
        const ok = await showConfirm(t("localBrowser.deleteConfirmMsg", { label }), t("localBrowser.deleteConfirmTitle"));
        if (!ok)
            return;
        const failed = [];
        for (const p of paths) {
            try {
                await api.deleteLocalPath(p);
            }
            catch (err) {
                failed.push(`${p}: ${String(err)}`);
            }
        }
        await this.refresh();
        if (failed.length) {
            this.inlineError = t("localBrowser.deleteFailed", { error: failed.slice(0, 2).join("; ") });
            this.render();
        }
    }
    /** Register local-panel keyboard shortcuts (navigation, rename, new folder, delete, upload, type-ahead). */
    setupKeyboardShortcuts() {
        document.addEventListener("keydown", (e) => {
            if (document.querySelector(".modal-overlay"))
                return;
            if (getActivePanel() !== "local" || !this.profileId)
                return;
            const tag = document.activeElement?.tagName?.toLowerCase();
            if (tag === "input" || tag === "textarea" || tag === "select")
                return;
            // Enter on a focused toolbar button should trigger the button's own click, not the
            // panel's "open selected entry" shortcut (M2a).
            if (e.key === "Enter" && tag === "button")
                return;
            const id = matchShortcut(e, ["panels", "local"]);
            if (!id) {
                if (!e.ctrlKey && !e.altKey && !e.metaKey && e.key.length === 1 && /\S/.test(e.key)
                    && !this.busy && !matchShortcut(e, "global")) {
                    e.preventDefault();
                    this.typeAhead(e.key);
                }
                return;
            }
            if (this.busy)
                return;
            const names = this.entries.map((x) => x.name);
            const st = () => ({ selected: this.selectedNames, anchor: this.anchorName, cursor: this.cursorName });
            const page = Math.max(1, Math.floor((this.container.querySelector(".local-browser__scroll")?.clientHeight ?? 300) / 24) - 1);
            const selected = this.entries.filter((x) => this.selectedNames.has(x.name));
            // Returning `false` means "not handled": skip preventDefault so native
            // browser behavior (e.g. text copy, native select-all) still runs.
            const run = {
                cursorUp: () => this.applySelection(moveCursor(names, st(), -1, false)),
                cursorDown: () => this.applySelection(moveCursor(names, st(), 1, false)),
                extendUp: () => this.applySelection(moveCursor(names, st(), -1, true)),
                extendDown: () => this.applySelection(moveCursor(names, st(), 1, true)),
                first: () => this.applySelection(moveCursor(names, st(), "start", false)),
                last: () => this.applySelection(moveCursor(names, st(), "end", false)),
                pageUp: () => this.applySelection(moveCursor(names, st(), -page, false)),
                pageDown: () => this.applySelection(moveCursor(names, st(), page, false)),
                open: () => {
                    if (selected.length !== 1)
                        return;
                    const p = joinPath(this.currentPath, selected[0].name);
                    if (selected[0].is_dir)
                        void this.navigateTo(p);
                    else
                        void this._ctxOpen(p, this.editorCommand, true);
                },
                parent: () => { if (this.currentPath !== "/")
                    void this.navigateTo(parentPath(this.currentPath)); },
                focusPath: () => {
                    const input = this.container.querySelector("#lb-path-input");
                    input?.focus();
                    input?.select();
                },
                refresh: () => void this.refresh(),
                rename: () => { if (selected.length === 1)
                    void this._ctxRename(selected[0].name); },
                newFolder: () => void this.createFolder(),
                delete: () => { if (selected.length > 0)
                    void this.deleteSelected(); },
                selectAll: () => {
                    if ((window.getSelection()?.toString().length ?? 0) > 0)
                        return false;
                    this.applySelection({ selected: new Set(names), anchor: names[0] ?? null, cursor: this.cursorName });
                },
                clearSelection: () => {
                    if (this._contextMenu) {
                        this._hideContextMenu();
                        return;
                    }
                    this.applySelection({ selected: new Set(), anchor: null, cursor: null });
                },
                upload: () => {
                    const paths = this.getSelectedPaths();
                    if (paths.length === 0 || !this.onUploadCallback)
                        return;
                    const label = paths.length === 1 ? selected[0].name : t("localBrowser.itemsLabel", { count: paths.length });
                    void this.onUploadCallback(paths, label);
                },
            };
            const action = run[id];
            if (!action)
                return;
            if (action() === false)
                return;
            e.preventDefault();
        });
    }
    typeAhead(ch) {
        this.typeAheadBuffer += ch.toLowerCase();
        if (this.typeAheadTimer !== null)
            window.clearTimeout(this.typeAheadTimer);
        this.typeAheadTimer = window.setTimeout(() => { this.typeAheadBuffer = ""; this.typeAheadTimer = null; }, 800);
        const match = this.entries.find((x) => x.name.toLowerCase().startsWith(this.typeAheadBuffer));
        if (match)
            this.applySelection({ selected: new Set([match.name]), anchor: match.name, cursor: match.name });
    }
    wireEvents() {
        // ── Path input ────────────────────────────────────────────────────────────
        const pathInput = this.container.querySelector("#lb-path-input");
        pathInput?.addEventListener("keydown", (e) => {
            if (e.key === "Enter") {
                const p = pathInput.value.trim() || "/";
                void this.navigateTo(p);
            }
            else if (e.key === "Escape") {
                pathInput.value = this.currentPath;
                pathInput.blur();
            }
        });
        // ── Toolbar buttons ────────────────────────────────────────────────────────
        this.container.querySelector("#lb-up-btn")?.addEventListener("click", () => {
            if (this.currentPath !== "/")
                void this.navigateTo(parentPath(this.currentPath));
        });
        this.container.querySelector("#lb-home-btn")?.addEventListener("click", () => {
            void this.navigateTo(this.homePath || "/");
        });
        this.container.querySelector("#lb-refresh-btn")?.addEventListener("click", () => {
            void this.refresh();
        });
        // ── Table: click + dblclick ────────────────────────────────────────────────
        const tbody = this.container.querySelector("tbody");
        if (!tbody)
            return;
        tbody.addEventListener("click", (e) => {
            const row = e.target.closest("tr.lb-entry");
            if (!row) {
                this.selectedNames.clear();
                this.anchorName = null;
                this.render();
                return;
            }
            const name = row.dataset.name;
            if (!name || name === "..")
                return;
            const me = e;
            const st = clickSelect(this.entries.map((x) => x.name), { selected: this.selectedNames, anchor: this.anchorName, cursor: this.cursorName }, name, { ctrl: me.ctrlKey || me.metaKey, shift: me.shiftKey });
            this.applySelection(st);
        });
        tbody.addEventListener("dblclick", (e) => {
            const row = e.target.closest("tr.lb-entry");
            if (!row)
                return;
            const isDir = row.dataset.isdir === "true";
            const path = row.dataset.path;
            if (isDir && path)
                void this.navigateTo(path);
        });
        // ── Context menu (right-click) ─────────────────────────────────────────────
        tbody.addEventListener("contextmenu", (e) => {
            const row = e.target.closest("tr.lb-entry");
            if (!row)
                return;
            const name = row.dataset.name;
            if (!name || name === "..")
                return;
            e.preventDefault();
            const isDir = row.dataset.isdir === "true";
            this._showContextMenu(e.clientX, e.clientY, name, isDir);
        });
        // ── Drag source (local files → remote browser = upload) ────────────────────
        tbody.addEventListener("dragstart", (e) => {
            const row = e.target.closest("tr.lb-entry");
            const name = row?.dataset.name;
            if (!row || !name || name === "..") {
                e.preventDefault();
                return;
            }
            const paths = this.selectedNames.has(name)
                ? this.getSelectedPaths()
                : [joinPath(this.currentPath, name)];
            this.dragSourceNames = new Set(paths);
            setDragSource({ type: "local", paths });
            e.dataTransfer.effectAllowed = "copy";
            e.dataTransfer.setData("text/plain", "local-to-remote");
        });
        tbody.addEventListener("dragend", () => {
            this.dragSourceNames.clear();
            clearDragSource();
        });
        // ── Drop target (remote files → local browser = download) ───────────────────
        const localBrowserEl = this.container.querySelector(".local-browser");
        localBrowserEl?.addEventListener("dragover", (e) => {
            const src = getDragSource();
            if (!src || src.type !== "remote" || this.busy)
                return;
            e.preventDefault();
            e.stopPropagation(); // don't let Tauri's OS-drag handler see it
            e.dataTransfer.dropEffect = "copy";
            this.setDragOver(true);
            const dir = this.dropDirFromElement(e.target);
            this.setLocalDropTarget(dir === this.currentPath ? null : dir);
        });
        localBrowserEl?.addEventListener("dragleave", (e) => {
            // Only clear when leaving the entire component
            if (!localBrowserEl.contains(e.relatedTarget)) {
                this.setDragOver(false);
                this.setLocalDropTarget(null);
            }
        });
        localBrowserEl?.addEventListener("drop", async (e) => {
            e.preventDefault();
            e.stopPropagation();
            this.setDragOver(false);
            this.setLocalDropTarget(null);
            const src = getDragSource();
            if (!src || src.type !== "remote" || this.busy)
                return;
            clearDragSource();
            if (this.onDownloadCallback) {
                await this.onDownloadCallback(src.names, this.dropDirFromElement(e.target));
            }
        });
    }
    applySelection(st) {
        this.selectedNames = st.selected;
        this.anchorName = st.anchor;
        this.cursorName = st.cursor;
        this.container.querySelectorAll("tr.lb-entry").forEach((r) => {
            r.classList.toggle("lb-entry--selected", this.selectedNames.has(r.dataset.name ?? ""));
            r.classList.toggle("lb-entry--cursor", r.dataset.name === this.cursorName);
        });
        this.container.querySelector(`tr.lb-entry[data-name="${CSS.escape(this.cursorName ?? "")}"]`)
            ?.scrollIntoView({ block: "nearest" });
    }
    getSelectedPaths() {
        return this.entries.filter((e) => this.selectedNames.has(e.name)).map((e) => joinPath(this.currentPath, e.name));
    }
    dropDirFromElement(el) {
        const row = el?.closest("tr.lb-entry");
        if (!row || row.dataset.isdir !== "true" || !row.dataset.path)
            return this.currentPath;
        return row.dataset.path; // ".." row carries the parent path already
    }
    setLocalDropTarget(path) {
        this.container.querySelectorAll("tr.lb-entry").forEach((r) => {
            r.classList.toggle("lb-entry--drop-target", path !== null && r.dataset.path === path && r.dataset.isdir === "true");
        });
    }
    setDragOver(value) {
        if (this.isDragOver === value)
            return;
        this.isDragOver = value;
        this.container.querySelector(".local-browser")
            ?.classList.toggle("local-browser--dragover", value);
        this.container.querySelector(".local-browser__drop-hint")
            ?.classList.toggle("local-browser__drop-hint--active", value);
    }
    async navigateTo(path) {
        if (this.busy)
            return;
        this.busy = true;
        try {
            this.entries = await api.listLocalDirectory(path);
            this.currentPath = path;
            this.inlineError = null;
            this.selectedNames = new Set();
            this.anchorName = null;
            this.cursorName = null;
            this.onPathChange?.(path);
        }
        catch (err) {
            this.inlineError = t("localBrowser.cannotNavigate", {
                path,
                error: String(err),
            });
        }
        finally {
            this.busy = false;
        }
        this.render();
    }
}
