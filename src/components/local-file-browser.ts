import * as api from "../api/index";
import type { FileEntry } from "../types";
import { t } from "../i18n/index";
import { setDragSource, getDragSource, clearDragSource } from "../dnd-state";

// ── Helpers ────────────────────────────────────────────────────────────────────

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function joinPath(dir: string, name: string): string {
  return dir.replace(/\/?$/, "/") + name;
}

function parentPath(path: string): string {
  if (path === "/") return "/";
  const parts = path.split("/").filter((p) => p.length > 0);
  parts.pop();
  return parts.length === 0 ? "/" : "/" + parts.join("/");
}

// ── Inline SVG icons (14×14, Lucide-style, currentColor) ──────────────────────
const ICONS = {
  up:      `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/></svg>`,
  home:    `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>`,
  refresh: `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/></svg>`,
} as const;

// ── Component ──────────────────────────────────────────────────────────────────

export class LocalFileBrowser {
  private container: HTMLElement;

  private profileId: string | null = null;
  private homePath: string = "";
  private currentPath: string = "";
  private entries: FileEntry[] = [];
  private busy: boolean = false;
  private isDragOver: boolean = false; // drop-from-remote indicator
  private inlineError: string | null = null;

  // Drag-from-local state
  private dragSourceNames: Set<string> = new Set();

  // Callbacks wired from main.ts
  private onDownloadCallback: ((remoteNames: string[], localDestDir: string) => Promise<void>) | null = null;
  private onUploadCallback: ((localPaths: string[], name: string) => Promise<void>) | null = null;
  private onPathChange: ((path: string) => void) | null = null;

  // Editor command from the connected profile (optional)
  private editorCommand: string | null = null;

  // Context menu
  private _contextMenu: HTMLElement | null = null;
  private _hideContextMenuBound = (e: MouseEvent) => this._hideContextMenu(e);

  constructor(containerId: string) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;
    this.renderEmpty();
  }

  // ── Public API ───────────────────────────────────────────────────────────────

  /** Called when a profile connects. Loads the saved path or $HOME. */
  async setProfile(profileId: string): Promise<void> {
    this.profileId = profileId;
    try {
      this.homePath = await api.getHomeDir();
      this.currentPath = await api.getLocalBrowserPath(profileId);
    } catch {
      this.currentPath = await api.getHomeDir().catch(() => "/");
      this.homePath = this.currentPath;
    }
    await this.refresh();
  }

  /** Clear state on disconnect. */
  clear(): void {
    this.profileId = null;
    this.currentPath = "";
    this.homePath = "";
    this.entries = [];
    this.busy = false;
    this.isDragOver = false;
    this.inlineError = null;
    this.renderEmpty();
  }

  /** The current local directory. Used by main.ts to save path on disconnect. */
  getCurrentPath(): string {
    return this.currentPath;
  }

  /**
   * Register a callback that is invoked when the user drops remote files onto the local browser
   * (i.e. the local browser receives a "download here" drop).
   */
  onDownload(cb: (remoteNames: string[], localDestDir: string) => Promise<void>): void {
    this.onDownloadCallback = cb;
  }

  /** Notify the local browser that a download is happening (disable drag target). */
  setBusy(value: boolean): void {
    this.busy = value;
  }

  /** Register a callback fired whenever the user navigates to a new path. */
  onPathChanged(cb: (path: string) => void): void {
    this.onPathChange = cb;
  }

  /** Register a callback invoked when the user chooses "Upload to remote" from the context menu. */
  onUpload(cb: (localPaths: string[], name: string) => Promise<void>): void {
    this.onUploadCallback = cb;
  }

  /** Set the editor command from the connected profile (may be null/empty). */
  setEditorCommand(cmd: string | null): void {
    this.editorCommand = cmd || null;
  }

  // ── Rendering ────────────────────────────────────────────────────────────────

  private renderEmpty(): void {
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

  async refresh(): Promise<void> {
    if (!this.profileId || !this.currentPath) return;
    this.busy = true;
    try {
      this.entries = await api.listLocalDirectory(this.currentPath);
      this.inlineError = null;
    } catch (err) {
      this.inlineError = String(err);
    } finally {
      this.busy = false;
    }
    this.render();
  }

  private render(): void {
    if (!this.profileId) { this.renderEmpty(); return; }

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
            return `<tr class="lb-entry${entry.is_dir ? " lb-entry--dir" : ""}" draggable="${!entry.is_dir ? 'true' : 'false'}" data-name="${escHtml(entry.name)}" data-isdir="${entry.is_dir}" data-path="${escHtml(fullPath)}">
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

  private _showContextMenu(x: number, y: number, name: string, isDir: boolean): void {
    this._hideContextMenu();
    const isFile = !isDir;
    const menu = document.createElement("div");
    menu.className = "lb-context-menu";
    menu.innerHTML = `
      ${isFile ? `<button data-action="open">${t("localBrowser.ctxOpen")}</button>` : ""}
      ${isFile ? `<button data-action="edit">${t("localBrowser.ctxEdit")}</button>` : ""}
      ${this.onUploadCallback ? `<button data-action="upload">${t("localBrowser.ctxUpload")}</button>` : ""}
      <button data-action="rename">${t("localBrowser.ctxRename")}</button>
    `;
    document.body.appendChild(menu);
    // Clamp to viewport
    const rect = menu.getBoundingClientRect();
    menu.style.left = `${Math.min(x, window.innerWidth  - rect.width  - 4)}px`;
    menu.style.top  = `${Math.min(y, window.innerHeight - rect.height - 4)}px`;
    this._contextMenu = menu;

    menu.addEventListener("click", (e) => {
      const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-action]");
      if (!btn) return;
      const action = btn.dataset.action;
      this._hideContextMenu();
      const path = joinPath(this.currentPath, name);
      if      (action === "open")   void this._ctxOpen(path, null);
      else if (action === "edit")   void this._ctxOpen(path, this.editorCommand);
      else if (action === "upload") void this._ctxUpload(path, name);
      else if (action === "rename") void this._ctxRename(name);
    });

    setTimeout(() => {
      document.addEventListener("mousedown", this._hideContextMenuBound, { once: true });
    }, 0);
  }

  private _hideContextMenu(e?: MouseEvent): void {
    if (e && this._contextMenu?.contains(e.target as Node)) return;
    this._contextMenu?.remove();
    this._contextMenu = null;
  }

  private async _ctxOpen(path: string, editor: string | null): Promise<void> {
    try {
      await api.openLocalFile(path, editor);
    } catch (err) {
      this.inlineError = t("localBrowser.openFailed", { error: String(err) });
      this.render();
    }
  }

  private async _ctxUpload(path: string, name: string): Promise<void> {
    if (!this.onUploadCallback) return;
    await this.onUploadCallback([path], name);
  }

  private async _ctxRename(oldName: string): Promise<void> {
    const newName = window.prompt(t("localBrowser.renameTitle"), oldName);
    if (!newName || newName === oldName) return;
    if (newName.includes("/")) {
      this.inlineError = t("localBrowser.renameFailed", { error: "Name cannot contain \"/\"" });
      this.render();
      return;
    }
    const fromPath = joinPath(this.currentPath, oldName);
    const toPath   = joinPath(this.currentPath, newName);
    try {
      await api.renameLocalFile(fromPath, toPath);
      await this.refresh();
    } catch (err) {
      this.inlineError = t("localBrowser.renameFailed", { error: String(err) });
      this.render();
    }
  }

  private wireEvents(): void {
    // ── Path input ────────────────────────────────────────────────────────────
    const pathInput = this.container.querySelector<HTMLInputElement>("#lb-path-input");
    pathInput?.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        const p = pathInput.value.trim() || "/";
        void this.navigateTo(p);
      } else if (e.key === "Escape") {
        pathInput.value = this.currentPath;
        pathInput.blur();
      }
    });

    // ── Toolbar buttons ────────────────────────────────────────────────────────
    this.container.querySelector("#lb-up-btn")?.addEventListener("click", () => {
      if (this.currentPath !== "/") void this.navigateTo(parentPath(this.currentPath));
    });
    this.container.querySelector("#lb-home-btn")?.addEventListener("click", () => {
      void this.navigateTo(this.homePath || "/");
    });
    this.container.querySelector("#lb-refresh-btn")?.addEventListener("click", () => {
      void this.refresh();
    });

    // ── Table: click + dblclick ────────────────────────────────────────────────
    const tbody = this.container.querySelector("tbody");
    if (!tbody) return;

    tbody.addEventListener("dblclick", (e) => {
      const row = (e.target as HTMLElement).closest<HTMLElement>("tr.lb-entry");
      if (!row) return;
      const isDir = row.dataset.isdir === "true";
      const path = row.dataset.path;
      if (isDir && path) void this.navigateTo(path);
    });

    // ── Context menu (right-click) ─────────────────────────────────────────────
    tbody.addEventListener("contextmenu", (e) => {
      const row = (e.target as HTMLElement).closest<HTMLElement>("tr.lb-entry");
      if (!row) return;
      const name = row.dataset.name;
      if (!name || name === "..") return;
      e.preventDefault();
      const isDir = row.dataset.isdir === "true";
      this._showContextMenu(e.clientX, e.clientY, name, isDir);
    });

    // ── Drag source (local files → remote browser = upload) ────────────────────
    tbody.addEventListener("dragstart", (e) => {
      const row = (e.target as HTMLElement).closest<HTMLElement>("tr.lb-entry");
      // Only drag regular files, not folders or ".." (can be extended later)
      if (!row || row.dataset.isdir === "true" || row.dataset.name === "..") {
        e.preventDefault();
        return;
      }
      const path = row.dataset.path!;
      this.dragSourceNames.add(path);
      setDragSource({ type: "local", paths: [path] });
      e.dataTransfer!.effectAllowed = "copy";
      e.dataTransfer!.setData("text/plain", "local-to-remote");
    });

    tbody.addEventListener("dragend", () => {
      this.dragSourceNames.clear();
      clearDragSource();
    });

    // ── Drop target (remote files → local browser = download) ─────────────────
    const localBrowserEl = this.container.querySelector<HTMLElement>(".local-browser");

    localBrowserEl?.addEventListener("dragover", (e) => {
      const src = getDragSource();
      if (!src || src.type !== "remote" || this.busy) return;
      e.preventDefault();
      e.dataTransfer!.dropEffect = "copy";
      this.setDragOver(true);
    });

    localBrowserEl?.addEventListener("dragleave", (e) => {
      // Only clear when leaving the entire component
      if (!localBrowserEl.contains(e.relatedTarget as Node | null)) {
        this.setDragOver(false);
      }
    });

    localBrowserEl?.addEventListener("drop", async (e) => {
      e.preventDefault();
      this.setDragOver(false);
      const src = getDragSource();
      if (!src || src.type !== "remote" || this.busy) return;
      clearDragSource();
      if (this.onDownloadCallback) {
        await this.onDownloadCallback(src.names, this.currentPath);
      }
    });
  }

  private setDragOver(value: boolean): void {
    if (this.isDragOver === value) return;
    this.isDragOver = value;
    this.container.querySelector<HTMLElement>(".local-browser")
      ?.classList.toggle("local-browser--dragover", value);
    this.container.querySelector<HTMLElement>(".local-browser__drop-hint")
      ?.classList.toggle("local-browser__drop-hint--active", value);
  }

  private async navigateTo(path: string): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try {
      this.entries = await api.listLocalDirectory(path);
      this.currentPath = path;
      this.inlineError = null;
      this.onPathChange?.(path);
    } catch (err) {
      this.inlineError = t("localBrowser.cannotNavigate", {
        path,
        error: String(err),
      });
    } finally {
      this.busy = false;
    }
    this.render();
  }
}
