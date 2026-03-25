import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "../api/index";
import { showConfirm, showPrompt, showOverwriteDialog } from "./dialog";
import type { OverwriteAction } from "./dialog";
import type { FileEntry } from "../types";
import { t } from "../i18n/index";

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

// ── Inline SVG icons (Lucide-style, 14×14, currentColor) ──────────────────
const ICONS = {
  disconnect:   `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`,
  terminal:     `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>`,
  home:         `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>`,
  up:           `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/></svg>`,
  refresh:      `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/></svg>`,
  upload:       `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 16 12 12 8 16"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.39 18.39A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.3"/></svg>`,
  uploadFolder: `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2z"/><polyline points="12 14 12 18"/><polyline points="10 16 12 14 14 16"/></svg>`,
  download:     `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="8 17 12 21 16 17"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.88 18.09A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.3"/></svg>`,
  rename:       `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>`,
  moveTo:       `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 14 20 9 15 4"/><path d="M4 20v-7a4 4 0 0 1 4-4h12"/></svg>`,
  edit:         `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>`,
  delete:       `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>`,
  newFile:      `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="12" y1="18" x2="12" y2="12"/><line x1="9" y1="15" x2="15" y2="15"/></svg>`,
  newFolder:    `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><line x1="12" y1="11" x2="12" y2="17"/><line x1="9" y1="14" x2="15" y2="14"/></svg>`,
  openFolder:   `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>`,
} as const;

type CtxMenuItem =
  | { icon: string; label: string; action: () => void; danger?: boolean }
  | { separator: true };

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Join a remote directory path with a filename. Handles trailing slashes. */
function joinPath(dir: string, name: string): string {
  return dir.replace(/\/?$/, "/") + name;
}

/** Navigate one level up from a remote path. */
function parentPath(path: string): string {
  const parts = path.split("/").filter((p) => p.length > 0);
  parts.pop();
  return parts.length === 0 ? "/" : "/" + parts.join("/");
}

export class FileBrowser {
  private container: HTMLElement;

  private profileId: string | null = null;
  private localPath: string | null = null;
  private currentPath: string = "/";
  private homePath: string = "/";
  private entries: FileEntry[] = [];

  // Multi-selection state
  private selectedNames: Set<string> = new Set();
  private anchorName: string | null = null; // for Shift+click range

  // Drag-and-drop (internal move) state
  private dragSourceNames: Set<string> = new Set();
  private dropTargetName: string | null = null;
  private isDraggingInternal: boolean = false;

  private busy: boolean = false;
  private inlineError: string | null = null;
  private isDragOver: boolean = false; // Tauri OS→app drag indicator

  // Transfer progress (upload / download)
  private transferProgress: {
    label: string;   // "Uploading" | "Downloading"
    current: number;
    total: number;
    cancelled: boolean;
  } | null = null;

  private onStatusMessage: ((msg: string, isError: boolean) => void) | null = null;
  private onDisconnectCallback: (() => void) | null = null;
  private uploadApplyToAllDecision: OverwriteAction | null = null;

  constructor(containerId: string) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;
    this.renderEmpty();
    this.setupDragDrop();
  }

  /** Set up the Tauri window drag-and-drop listener (once, for the lifetime of the component). */
  private setupDragDrop(): void {
    getCurrentWindow()
      .onDragDropEvent((event) => {
        const type = event.payload.type;
        if (type === "enter" || type === "over") {
          // Only show visual indicator for external (OS) drags, not internal row drags
          if (this.profileId && !this.busy && !this.isDraggingInternal) {
            this.setDragOver(true);
          }
        } else if (type === "leave") {
          this.setDragOver(false);
        } else if (type === "drop") {
          this.setDragOver(false);
          if (this.profileId && !this.busy && !this.isDraggingInternal) {
            // 'drop' payload always includes paths
            const paths = (event.payload as { type: "drop"; paths: string[] }).paths;
            if (paths.length > 0) {
              void this.uploadPathList(paths);
            }
          }
        }
      })
      .then(() => {
        // Listener registered — not stored because FileBrowser lives for the app lifetime
      })
      .catch(() => {
        // Non-fatal — drag & drop will simply not be available
      });
  }

  private setDragOver(value: boolean): void {
    if (this.isDragOver === value) return;
    this.isDragOver = value;
    const el = this.container.querySelector<HTMLElement>(".file-browser");
    if (el) {
      el.classList.toggle("file-browser--dragover", value);
    }
  }

  setProfile(profileId: string, defaultPath: string = "/", localPath: string | null = null): void {
    this.profileId = profileId;
    this.localPath = localPath;
    this.currentPath = defaultPath;
    this.homePath = defaultPath;
    this.clearSelection();
  }

  /** Provide a callback to surface status messages (download path, errors, etc.) */
  setStatusCallback(cb: (msg: string, isError: boolean) => void): void {
    this.onStatusMessage = cb;
  }

  /** Provide a callback invoked when the user clicks Disconnect. */
  onDisconnect(callback: () => void): void {
    this.onDisconnectCallback = callback;
  }

  async refresh(): Promise<void> {
    if (!this.profileId) return;
    this.setBusy(true);
    try {
      this.entries = await api.listDirectory(this.profileId, this.currentPath);
      this.clearSelection();
      this.inlineError = null;
      this.setBusy(false);
      this.render();
    } catch (err) {
      // Keep toolbar buttons active — connection is still live.
      this.inlineError = String(err);
      this.setBusy(false);
      this.render();
    }
  }

  private clearSelection(): void {
    this.selectedNames.clear();
    this.anchorName = null;
  }

  private setBusy(value: boolean): void {
    this.busy = value;
  }

  /** Returns the single selected FileEntry, or null when 0 or >1 are selected. */
  private get selectedEntry(): FileEntry | null {
    if (this.selectedNames.size !== 1) return null;
    const name = [...this.selectedNames][0];
    return this.entries.find((e) => e.name === name) ?? null;
  }

  /** Returns all selected FileEntry objects. */
  private get selectedEntries(): FileEntry[] {
    return this.entries.filter((e) => this.selectedNames.has(e.name));
  }

  private get selectedRemotePath(): string | null {
    const entry = this.selectedEntry;
    if (!entry) return null;
    return joinPath(this.currentPath, entry.name);
  }

  private renderEmpty(): void {
    this.container.innerHTML = `
      <div class="file-browser file-browser--empty">
        <div class="file-browser__toolbar">
          <button id="disconnect-btn" disabled title="${t("fileBrowser.disconnect")}">${ICONS.disconnect} ${t("fileBrowser.disconnect")}</button>
          <button id="terminal-btn"   disabled title="${t("fileBrowser.terminal")}">${ICONS.terminal}</button>
          <button id="home-btn"       disabled title="${t("fileBrowser.home")}">${ICONS.home}</button>
          <button id="up-btn"         disabled title="${t("fileBrowser.up")}">${ICONS.up}</button>
          <button id="refresh-btn"    disabled title="${t("fileBrowser.refresh")}">${ICONS.refresh}</button>
        </div>
        <p>${t("fileBrowser.connectPrompt")}</p>
      </div>
    `;
  }

  private render(): void {
    // Preserve scroll position before rebuilding the DOM (list region only).
    const scrollEl = this.container.querySelector<HTMLElement>(".file-browser__scroll");
    const savedScrollTop = scrollEl?.scrollTop ?? 0;

    const isAtRoot = this.currentPath === "/";
    const hasProfile = this.profileId !== null;
    const selCount = this.selectedNames.size;
    const singleEntry = this.selectedEntry; // non-null only when exactly 1 selected
    const hasAny = selCount > 0;
    const hasExactlyOne = selCount === 1;
    const hasFile = hasExactlyOne && singleEntry !== null && !singleEntry.is_dir;

    // ".." row — also a drop target when dragging items
    const upRow = isAtRoot
      ? ""
      : `<tr class="file-entry file-entry--dir file-entry--up${this.dropTargetName === ".." ? " file-entry--drop-target" : ""}" data-name=".." data-isdir="true">
           <td colspan="2">${t("fileBrowser.upRow")}</td>
         </tr>`;

    const rows =
      this.entries.length === 0 && !this.inlineError
        ? `<tr><td colspan="2" class="empty-dir">${t("fileBrowser.emptyDir")}</td></tr>`
        : this.entries.length === 0
        ? ""
        : this.entries
            .map(
              (entry) => {
                const isSelected = this.selectedNames.has(entry.name);
                const isDropTarget = this.dropTargetName === entry.name;
                let cls = "file-entry";
                if (entry.is_dir) cls += " file-entry--dir";
                if (isSelected) cls += " file-entry--selected";
                if (isDropTarget) cls += " file-entry--drop-target";
                return `<tr class="${cls}" data-name="${escHtml(entry.name)}" data-isdir="${entry.is_dir}" draggable="true">
                   <td>${entry.is_dir ? "&#128193; " : ""}${escHtml(entry.name)}</td>
                   <td>${entry.size != null && !entry.is_dir ? formatBytes(entry.size) : "—"}</td>
                 </tr>`;
              }
            )
            .join("");

    // Selection count info line
    const selectionInfo = selCount > 1
      ? `<div class="file-browser__selection-info">${t("fileBrowser.itemsSelected", { count: selCount })}</div>`
      : selCount === 1
      ? `<div class="file-browser__selection-info">${t("fileBrowser.oneItemSelected")}</div>`
      : "";

    const inlineErrorHtml = this.inlineError
      ? `<div class="file-browser__inline-error">${escHtml(this.inlineError)}</div>`
      : "";

    const downloadDisabled = !hasAny || this.busy;

    // Transfer progress bar
    const tp = this.transferProgress;
    const transferProgressHtml = tp
      ? `<div class="transfer-progress" id="transfer-progress">
           <span class="transfer-progress__status" id="transfer-status">${escHtml(tp.label)}: ${tp.current} / ${tp.total} files</span>
           <button class="transfer-progress__cancel" id="transfer-cancel-btn">${t("fileBrowser.transferCancel")}</button>
         </div>`
      : "";

    this.container.innerHTML = `
      <div class="file-browser${this.isDragOver ? " file-browser--dragover" : ""}">
        <div class="file-browser__toolbar">
          <button id="disconnect-btn" ${!hasProfile || this.busy ? "disabled" : ""} title="${t("fileBrowser.disconnect")}">${ICONS.disconnect} ${t("fileBrowser.disconnect")}</button>
          <button id="terminal-btn"   ${!hasProfile || this.busy ? "disabled" : ""} title="${t("fileBrowser.terminal")}">${ICONS.terminal}</button>
          <button id="home-btn"       ${!hasProfile || this.busy ? "disabled" : ""} title="${t("fileBrowser.home")}">${ICONS.home}</button>
          <button id="up-btn"         ${isAtRoot || this.busy ? "disabled" : ""} title="${t("fileBrowser.up")}">${ICONS.up}</button>
          <button id="refresh-btn"    ${this.busy ? "disabled" : ""} title="${t("fileBrowser.refresh")}">${ICONS.refresh}</button>
        </div>
        <div class="file-browser__path-row">
          <input id="path-input" type="text" class="file-browser__path-input"
            value="${escHtml(this.currentPath)}" spellcheck="false" autocomplete="off">
        </div>
        ${inlineErrorHtml}
        <div class="file-browser__scroll">
        <table class="file-browser__table">
          <thead>
            <tr><th>${t("fileBrowser.columnName")}</th><th>${t("fileBrowser.columnSize")}</th></tr>
          </thead>
          <tbody>${upRow}${rows}</tbody>
        </table>
        </div>
        ${selectionInfo}
        <div class="file-browser__actions">
          <button id="upload-btn"        ${!hasProfile || this.busy ? "disabled" : ""} title="${t("fileBrowser.upload")}">${ICONS.upload}</button>
          <button id="upload-folder-btn" ${!hasProfile || this.busy ? "disabled" : ""} title="${t("fileBrowser.uploadFolder")}">${ICONS.uploadFolder}</button>
          <button id="download-btn"      ${downloadDisabled ? "disabled" : ""} title="${t("fileBrowser.download")}">${ICONS.download}</button>
          <button id="rename-btn"        ${!hasExactlyOne || this.busy ? "disabled" : ""} title="${t("fileBrowser.rename")}">${ICONS.rename}</button>
          <button id="move-btn"          ${!hasAny || this.busy ? "disabled" : ""} title="${t("fileBrowser.moveTo")}">${ICONS.moveTo}</button>
          <button id="edit-btn"          ${!hasFile || this.busy ? "disabled" : ""} title="${t("fileBrowser.edit")}">${ICONS.edit}</button>
          <button id="delete-btn"        ${!hasAny  || this.busy ? "disabled" : ""} title="${t("fileBrowser.delete")}">${ICONS.delete}</button>
          <button id="new-file-btn"      ${!hasProfile || this.busy ? "disabled" : ""} title="${t("fileBrowser.newFile")}">${ICONS.newFile}</button>
          <button id="new-folder-btn"    ${!hasProfile || this.busy ? "disabled" : ""} title="${t("fileBrowser.newFolder")}">${ICONS.newFolder}</button>
        </div>
        ${transferProgressHtml}
      </div>
    `;

    // Restore scroll position after DOM rebuild.
    const newScrollEl = this.container.querySelector<HTMLElement>(".file-browser__scroll");
    if (newScrollEl && savedScrollTop > 0) {
      newScrollEl.scrollTop = savedScrollTop;
    }

    // ── Path input ─────────────────────────────────────────────────────────
    const pathInput = this.container.querySelector<HTMLInputElement>("#path-input");
    pathInput?.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        const p = pathInput.value.trim() || "/";
        this.navigateToPath(p);
      } else if (e.key === "Escape") {
        pathInput.value = this.currentPath;
        pathInput.blur();
      }
    });

    // ── Table click (multi-select) + dblclick (navigate) ──────────────────
    const tbody = this.container.querySelector("tbody");
    if (tbody) {
      // Click on empty area below rows → deselect all
      tbody.addEventListener("click", (e) => {
        const row = (e.target as HTMLElement).closest<HTMLElement>("tr.file-entry");
        if (!row) {
          this.clearSelection();
          this.render();
          return;
        }
        const name = row.dataset.name;
        if (!name) return;

        if (name === "..") {
          this.navigateUp();
          return;
        }

        const me = e as MouseEvent;
        if (me.ctrlKey || me.metaKey) {
          // Toggle individual entry
          if (this.selectedNames.has(name)) {
            this.selectedNames.delete(name);
          } else {
            this.selectedNames.add(name);
            this.anchorName = name;
          }
        } else if (me.shiftKey && this.anchorName) {
          // Range select from anchor to current
          const names = this.entries.map((e) => e.name);
          const anchorIdx = names.indexOf(this.anchorName);
          const curIdx = names.indexOf(name);
          if (anchorIdx >= 0 && curIdx >= 0) {
            const from = Math.min(anchorIdx, curIdx);
            const to = Math.max(anchorIdx, curIdx);
            for (let i = from; i <= to; i++) this.selectedNames.add(names[i]);
          }
        } else {
          // Plain click → single select
          this.selectedNames.clear();
          this.selectedNames.add(name);
          this.anchorName = name;
        }
        this.render();
      });

      tbody.addEventListener("dblclick", (e) => {
        const row = (e.target as HTMLElement).closest<HTMLElement>("tr.file-entry");
        if (!row) return;
        const name = row.dataset.name;
        const isDir = row.dataset.isdir === "true";
        if (name === "..") {
          this.navigateUp();
        } else if (name && isDir) {
          this.navigateInto(name);
        }
      });

      // ── Internal drag-and-drop (move) ────────────────────────────────────
      tbody.addEventListener("dragstart", (e) => {
        const row = (e.target as HTMLElement).closest<HTMLElement>("tr.file-entry");
        if (!row || row.dataset.name === "..") {
          e.preventDefault();
          return;
        }
        const name = row.dataset.name!;
        // If dragging a non-selected item, drag only that item (don't alter selection to avoid re-render mid-drag)
        if (this.selectedNames.has(name)) {
          this.dragSourceNames = new Set(this.selectedNames);
        } else {
          this.dragSourceNames = new Set([name]);
        }
        this.isDraggingInternal = true;
        e.dataTransfer!.effectAllowed = "move";
        e.dataTransfer!.setData("text/plain", "internal-move");
      });

      tbody.addEventListener("dragover", (e) => {
        if (!this.isDraggingInternal) return;
        const row = (e.target as HTMLElement).closest<HTMLElement>("tr.file-entry");
        if (!row) {
          this.setDropTarget(null);
          return;
        }
        const name = row.dataset.name!;
        const isDir = row.dataset.isdir === "true";

        // ".." row: valid drop target when not at root
        if (name === ".." && !isAtRoot) {
          e.preventDefault();
          e.dataTransfer!.dropEffect = "move";
          this.setDropTarget("..");
          return;
        }

        // Directory rows: valid target only if not in the dragged set
        if (isDir && name && !this.dragSourceNames.has(name)) {
          e.preventDefault();
          e.dataTransfer!.dropEffect = "move";
          this.setDropTarget(name);
        } else {
          this.setDropTarget(null);
        }
      });

      tbody.addEventListener("dragleave", (e) => {
        // Only clear when the drag leaves the tbody entirely (not just moving between cells)
        if (!tbody.contains(e.relatedTarget as Node | null)) {
          this.setDropTarget(null);
        }
      });

      tbody.addEventListener("drop", (e) => {
        e.preventDefault();
        if (!this.isDraggingInternal) return;
        const row = (e.target as HTMLElement).closest<HTMLElement>("tr.file-entry");
        if (!row) return;
        const targetName = row.dataset.name!;
        const isDir = row.dataset.isdir === "true";
        // Accept ".." as special target; reject non-directories and self-drops
        if (targetName !== ".." && (!isDir || this.dragSourceNames.has(targetName))) return;
        void this.handleMove(targetName);
      });

      tbody.addEventListener("dragend", () => {
        this.isDraggingInternal = false;
        this.dragSourceNames = new Set();
        this.setDropTarget(null);
      });
    }

    // ── Toolbar buttons ────────────────────────────────────────────────────
    document
      .getElementById("disconnect-btn")
      ?.addEventListener("click", () => this.handleDisconnect());

    document
      .getElementById("terminal-btn")
      ?.addEventListener("click", () => this.handleTerminal());

    document
      .getElementById("home-btn")
      ?.addEventListener("click", () => this.navigateTo(this.homePath));

    document
      .getElementById("up-btn")
      ?.addEventListener("click", () => this.navigateUp());

    document
      .getElementById("refresh-btn")
      ?.addEventListener("click", () => this.refresh());

    // ── Action buttons ─────────────────────────────────────────────────────
    document
      .getElementById("upload-btn")
      ?.addEventListener("click", () => this.handleUpload());

    document
      .getElementById("upload-folder-btn")
      ?.addEventListener("click", () => this.handleUploadFolder());

    document
      .getElementById("download-btn")
      ?.addEventListener("click", () => this.handleDownload());

    document
      .getElementById("rename-btn")
      ?.addEventListener("click", () => this.handleRename());

    document
      .getElementById("move-btn")
      ?.addEventListener("click", () => this.handleMoveTo());

    document
      .getElementById("edit-btn")
      ?.addEventListener("click", () => this.handleEdit());

    document
      .getElementById("delete-btn")
      ?.addEventListener("click", () => this.handleDelete());

    document
      .getElementById("new-file-btn")
      ?.addEventListener("click", () => this.handleNewFile());

    document
      .getElementById("new-folder-btn")
      ?.addEventListener("click", () => this.handleNewFolder());

    // ── Transfer-progress cancel ───────────────────────────────────────────
    document
      .getElementById("transfer-cancel-btn")
      ?.addEventListener("click", () => {
        if (!this.transferProgress) return;
        this.transferProgress.cancelled = true;
        const btn = document.getElementById("transfer-cancel-btn") as HTMLButtonElement | null;
        if (btn) { btn.disabled = true; btn.textContent = t("common.cancelling"); }
        const statusEl = document.getElementById("transfer-status");
        if (statusEl) statusEl.textContent = t("common.cancelling");
      });

    // ── Context menu (right-click) ─────────────────────────────────────────
    const tbodyCtx = this.container.querySelector("tbody");
    if (tbodyCtx) {
      tbodyCtx.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        if (!this.profileId || this.busy) return;
        const row = (e.target as HTMLElement).closest<HTMLElement>("tr.file-entry");
        if (row && row.dataset.name && row.dataset.name !== "..") {
          const name = row.dataset.name;
          const isDir = row.dataset.isdir === "true";
          // Select clicked entry (lightweight, no re-render)
          this.selectedNames.clear();
          this.selectedNames.add(name);
          this.anchorName = name;
          this.container.querySelectorAll<HTMLElement>("tr.file-entry").forEach((r) => {
            r.classList.toggle("file-entry--selected", r.dataset.name === name);
          });
          this.showContextMenu(e.clientX, e.clientY, isDir
            ? this.buildFolderContextItems(name)
            : this.buildFileContextItems(name)
          );
        } else if (!row) {
          this.showContextMenu(e.clientX, e.clientY, this.buildEmptyContextItems());
        }
      });
    }

    // Right-click on the scroll area (empty space below rows)
    const scrollCtx = this.container.querySelector<HTMLElement>(".file-browser__scroll");
    if (scrollCtx) {
      scrollCtx.addEventListener("contextmenu", (e) => {
        const row = (e.target as HTMLElement).closest<HTMLElement>("tr.file-entry");
        if (!row && this.profileId && !this.busy) {
          e.preventDefault();
          this.showContextMenu(e.clientX, e.clientY, this.buildEmptyContextItems());
        }
      });
    }
  }

  // ── Transfer-progress helpers ──────────────────────────────────────────────

  /**
   * Start a tracked transfer operation. Calls render() to insert the progress bar.
   * Must be called with busy=true already set.
   */
  private startTransfer(label: string, total: number): void {
    this.transferProgress = { label, current: 0, total, cancelled: false };
    this.render();
  }

  /**
   * Update the in-progress counter via direct DOM manipulation.
   * Must NOT call render() — that would interrupt the transfer loop visually.
   */
  private updateTransfer(current: number): void {
    if (!this.transferProgress) return;
    this.transferProgress.current = current;
    const el = document.getElementById("transfer-status");
    if (el) el.textContent = `${this.transferProgress.label}: ${current} / ${this.transferProgress.total} files`;
  }

  /**
   * Mark the transfer as done and remove the progress bar state.
   * render() will be called by the following refresh(), removing the element.
   */
  private endTransfer(): void {
    this.transferProgress = null;
    this.uploadApplyToAllDecision = null;
  }

  private normalizeRemoteError(err: unknown): string {
    const msg = String(err);
    if (/file exists|already exists/i.test(msg)) return "target already exists";
    if (/no such file|not found/i.test(msg)) return "source or target path not found";
    if (/permission denied|access denied/i.test(msg)) return "permission denied";
    return msg;
  }

  /**
   * Ask overwrite/skip/cancel for a remote destination path.
   * Returns true if upload may proceed, false if item should be skipped.
   * Throws "UPLOAD_CANCELLED" when the whole batch should stop.
   */
  private async resolveOverwrite(remotePath: string, label: string): Promise<boolean> {
    if (!this.profileId) return false;
    try {
      const exists = await api.remoteFileExists(this.profileId, remotePath);
      if (!exists) return true;

      let action: OverwriteAction;
      if (this.uploadApplyToAllDecision) {
        action = this.uploadApplyToAllDecision;
      } else {
        const result = await showOverwriteDialog(label);
        action = result.action;
        if (result.applyToAll && action !== "cancel") {
          this.uploadApplyToAllDecision = action;
        }
      }

      if (action === "cancel") throw new Error("UPLOAD_CANCELLED");
      if (action === "no") return false;
      return true;
    } catch (err) {
      if (String(err) === "Error: UPLOAD_CANCELLED") throw err;
      // Conflict check failed -> keep behavior resilient and attempt upload.
      return true;
    }
  }

  /**
   * Update the drop-target highlight on rows without a full re-render.
   * This must not trigger render() as that would cancel an active drag.
   */
  private setDropTarget(name: string | null): void {
    if (this.dropTargetName === name) return;
    this.dropTargetName = name;
    this.container.querySelectorAll<HTMLElement>("tr.file-entry").forEach((row) => {
      row.classList.toggle("file-entry--drop-target", row.dataset.name === name);
    });
  }

  // ── Navigation ────────────────────────────────────────────────────────────

  private async navigateInto(dirName: string): Promise<void> {
    if (!this.profileId || this.busy) return;
    const targetPath = joinPath(this.currentPath, dirName);
    this.setBusy(true);
    try {
      const newEntries = await api.listDirectory(this.profileId, targetPath);
      this.currentPath = targetPath;
      this.entries = newEntries;
      this.clearSelection();
      this.inlineError = null;
      this.setBusy(false);
      this.render();
    } catch (err) {
      const msg = t("fileBrowser.cannotOpen", { name: dirName, error: String(err) });
      this.onStatusMessage?.(msg, true);
      this.inlineError = msg;
      this.setBusy(false);
      this.render();
    }
  }

  private navigateUp(): void {
    if (this.currentPath === "/") return;
    this.navigateTo(parentPath(this.currentPath));
  }

  private navigateTo(path: string): void {
    if (!this.profileId || this.busy) return;
    this.navigateToPath(path);
  }

  private async navigateToPath(targetPath: string): Promise<void> {
    if (!this.profileId || this.busy) return;
    this.setBusy(true);
    try {
      const newEntries = await api.listDirectory(this.profileId, targetPath);
      this.currentPath = targetPath;
      this.entries = newEntries;
      this.clearSelection();
      this.inlineError = null;
      this.setBusy(false);
      this.render();
    } catch (err) {
      const msg = t("fileBrowser.cannotNavigate", { path: targetPath, error: String(err) });
      this.onStatusMessage?.(msg, true);
      this.inlineError = msg;
      this.setBusy(false);
      this.render();
    }
  }

  // ── Terminal ──────────────────────────────────────────────────────────────

  private async handleTerminal(): Promise<void> {
    if (!this.profileId) return;

    let useRuntimeCopy = false;
    try {
      const needsCopy = await api.checkKeyNeedsCopy(this.profileId);
      if (needsCopy) {
        const accepted = await showConfirm(
          t("fileBrowser.copyKeyMsg"),
          t("fileBrowser.copyKeyTitle")
        );
        if (accepted) {
          await api.copyKeyForRuntime(this.profileId);
          useRuntimeCopy = true;
        }
      }
    } catch {
      // Non-fatal
    }

    try {
      await api.launchSsh(this.profileId, useRuntimeCopy);
    } catch (err) {
      this.onStatusMessage?.(t("fileBrowser.terminalFailed", { error: String(err) }), true);
    }
  }

  // ── Disconnect ────────────────────────────────────────────────────────────

  private handleDisconnect(): void {
    this.profileId = null;
    this.localPath = null;
    this.currentPath = "/";
    this.homePath = "/";
    this.entries = [];
    this.clearSelection();
    this.busy = false;
    this.inlineError = null;
    this.isDragOver = false;
    this.isDraggingInternal = false;
    this.dragSourceNames = new Set();
    this.dropTargetName = null;
    this.transferProgress = null;
    this.onDisconnectCallback?.();
    this.renderEmpty();
  }

  // ── Upload ────────────────────────────────────────────────────────────────

  private async handleUpload(): Promise<void> {
    if (!this.profileId) return;

    const result = await open({
      multiple: true,
      directory: false,
      defaultPath: this.localPath ?? undefined,
    });

    if (!result) return;
    const paths = Array.isArray(result) ? result : [result];
    if (paths.length === 0) return;

    await this.uploadFileList(paths);
  }

  private async uploadFileList(localPaths: string[]): Promise<void> {
    if (!this.profileId) return;
    const profileId = this.profileId;
    const total = localPaths.length;

    this.setBusy(true);
    this.startTransfer(t("fileBrowser.transferUploading"), total);

    let uploaded = 0;
    let skipped = 0;
    let errors = 0;
    let aborted = false;

    for (const localFilePath of localPaths) {
      // Check for user-requested cancel
      if (this.transferProgress?.cancelled) { aborted = true; break; }

      const filename =
        localFilePath.replace(/\\/g, "/").split("/").pop() ?? localFilePath;
      const remotePath = joinPath(this.currentPath, filename);

      try {
        const proceed = await this.resolveOverwrite(remotePath, filename);
        if (!proceed) {
          skipped++;
          this.updateTransfer(uploaded + skipped + errors);
          continue;
        }
      } catch (err) {
        if (String(err) === "Error: UPLOAD_CANCELLED") {
          aborted = true;
          break;
        }
      }

      // ── Upload ───────────────────────────────────────────────────────────
      try {
        await api.uploadFile(profileId, localFilePath, remotePath);
        uploaded++;
      } catch {
        errors++;
      }

      this.updateTransfer(uploaded + skipped + errors);
    }

    this.endTransfer();

    // Summary status message
    if (aborted) {
      const parts: string[] = [];
      if (uploaded > 0) parts.push(t("fileBrowser.uploadedCount", { count: uploaded }));
      if (skipped > 0) parts.push(t("fileBrowser.skippedCount", { count: skipped }));
      const summary = parts.join(", ");
      this.onStatusMessage?.(
        summary
          ? t("fileBrowser.uploadCancelled", { summary })
          : t("fileBrowser.uploadCancelledNoFiles"),
        false
      );
    } else {
      const parts: string[] = [];
      if (uploaded > 0) parts.push(t("fileBrowser.uploadedCount", { count: uploaded }));
      if (skipped > 0) parts.push(t("fileBrowser.skippedCount", { count: skipped }));
      if (errors > 0) parts.push(t("fileBrowser.failedCount", { count: errors }));
      this.onStatusMessage?.(parts.join(", ") || t("fileBrowser.nothingToUpload"), errors > 0);
    }

    this.setBusy(false);
    await this.refresh();
  }

  private async uploadPathList(localPaths: string[]): Promise<void> {
    if (!this.profileId) return;
    const profileId = this.profileId;
    const total = localPaths.length;

    this.setBusy(true);
    this.startTransfer(t("fileBrowser.transferUploading"), total);

    let uploaded = 0;
    let skipped = 0;
    let errors = 0;
    let aborted = false;

    for (const localPath of localPaths) {
      if (this.transferProgress?.cancelled) break;

      const name =
        localPath.replace(/\\/g, "/").replace(/\/$/, "").split("/").pop() ??
        localPath;
      const remotePath = joinPath(this.currentPath, name);

      try {
        const proceed = await this.resolveOverwrite(remotePath, name);
        if (!proceed) {
          skipped++;
          this.updateTransfer(uploaded + skipped + errors);
          continue;
        }
      } catch (err) {
        if (String(err) === "Error: UPLOAD_CANCELLED") {
          aborted = true;
          break;
        }
      }

      try {
        await api.uploadPath(profileId, localPath, remotePath);
        uploaded++;
      } catch {
        errors++;
      }

      this.updateTransfer(uploaded + skipped + errors);
    }

    this.endTransfer();

    if (aborted) {
      const parts: string[] = [];
      if (uploaded > 0) parts.push(t("fileBrowser.uploadedCount", { count: uploaded }));
      if (skipped > 0) parts.push(t("fileBrowser.skippedCount", { count: skipped }));
      if (errors > 0) parts.push(t("fileBrowser.failedCount", { count: errors }));
      const summary = parts.join(", ");
      this.onStatusMessage?.(
        summary
          ? t("fileBrowser.uploadCancelled", { summary })
          : t("fileBrowser.uploadCancelledNoFiles"),
        false
      );
    } else if (errors === 0) {
      const parts: string[] = [];
      if (uploaded > 0) parts.push(t("fileBrowser.uploadedCount", { count: uploaded }));
      if (skipped > 0) parts.push(t("fileBrowser.skippedCount", { count: skipped }));
      this.onStatusMessage?.(parts.join(", ") || t("fileBrowser.nothingToUpload"), false);
    } else {
      const parts: string[] = [];
      if (uploaded > 0) parts.push(t("fileBrowser.uploadedCount", { count: uploaded }));
      if (skipped > 0) parts.push(t("fileBrowser.skippedCount", { count: skipped }));
      if (errors > 0) parts.push(t("fileBrowser.failedCount", { count: errors }));
      this.onStatusMessage?.(parts.join(", ") || t("fileBrowser.nothingToUpload"), true);
    }

    this.setBusy(false);
    await this.refresh();
  }

  private async handleUploadFolder(): Promise<void> {
    if (!this.profileId) return;

    const localFolderPath = await open({
      multiple: false,
      directory: true,
      title: t("fileBrowser.selectFolderUpload"),
      defaultPath: this.localPath ?? undefined,
    });

    if (!localFolderPath || typeof localFolderPath !== "string") return;

    const folderName =
      localFolderPath.replace(/\\/g, "/").replace(/\/$/, "").split("/").pop() ??
      "folder";
    const remotePath = joinPath(this.currentPath, folderName);

    try {
      const proceed = await this.resolveOverwrite(remotePath, folderName);
      if (!proceed) {
        this.onStatusMessage?.(t("fileBrowser.uploadSkipped"), false);
        return;
      }
      this.setBusy(true);
      this.onStatusMessage?.(t("fileBrowser.uploadingFolder", { name: folderName }), false);
      await api.uploadDirectory(this.profileId, localFolderPath, remotePath);
      this.onStatusMessage?.(t("fileBrowser.uploadedFolder", { name: folderName }), false);
      await this.refresh();
    } catch (err) {
      if (String(err) === "Error: UPLOAD_CANCELLED") {
        this.onStatusMessage?.(t("fileBrowser.uploadCancelledSimple"), false);
        return;
      }
      this.onStatusMessage?.(t("fileBrowser.folderUploadFailed", { error: String(err) }), true);
    } finally {
      this.setBusy(false);
    }
  }

  // ── Download ──────────────────────────────────────────────────────────────

  private async handleDownload(): Promise<void> {
    if (!this.profileId || this.selectedNames.size === 0) return;

    if (this.selectedNames.size > 1) {
      await this.handleDownloadMulti();
      return;
    }

    // Single selection
    if (!this.selectedEntry) return;
    if (this.selectedEntry.is_dir) {
      await this.handleDownloadFolder();
    } else {
      await this.handleDownloadFile();
    }
  }

  private async handleDownloadFile(): Promise<void> {
    if (!this.profileId || !this.selectedRemotePath || !this.selectedEntry) return;

    let savePath: string;

    if (this.localPath) {
      savePath = this.localPath.replace(/\/?$/, "/") + this.selectedEntry.name;
    } else {
      const chosen = await save({
        defaultPath: this.selectedEntry.name,
        title: "Save file",
      });
      if (!chosen) return;
      savePath = chosen;
    }

    try {
      this.setBusy(true);
      await api.downloadFileTo(this.profileId, this.selectedRemotePath, savePath);
      this.onStatusMessage?.(t("fileBrowser.downloadedTo", { path: savePath }), false);
    } catch (err) {
      this.onStatusMessage?.(t("fileBrowser.downloadFailed", { error: String(err) }), true);
    } finally {
      this.setBusy(false);
    }
  }

  private async handleDownloadFolder(): Promise<void> {
    if (!this.profileId || !this.selectedRemotePath || !this.selectedEntry) return;

    let localDestPath: string;

    if (this.localPath) {
      localDestPath = this.localPath.replace(/\/?$/, "/") + this.selectedEntry.name;
    } else {
      const chosen = await open({
        multiple: false,
        directory: true,
        title: "Select destination folder",
      });
      if (!chosen || typeof chosen !== "string") return;
      localDestPath = chosen.replace(/\/?$/, "/") + this.selectedEntry.name;
    }

    try {
      this.setBusy(true);
      await api.downloadDirectory(this.profileId, this.selectedRemotePath, localDestPath);
      this.onStatusMessage?.(t("fileBrowser.downloadedFolderTo", { path: localDestPath }), false);
    } catch (err) {
      this.onStatusMessage?.(t("fileBrowser.folderDownloadFailed", { error: String(err) }), true);
    } finally {
      this.setBusy(false);
    }
  }

  private async handleDownloadMulti(): Promise<void> {
    if (!this.profileId || this.selectedNames.size === 0) return;

    // Determine destination directory
    let destDir: string;
    if (this.localPath) {
      destDir = this.localPath;
    } else {
      const chosen = await open({
        multiple: false,
        directory: true,
        title: "Select destination folder for download",
      });
      if (!chosen || typeof chosen !== "string") return;
      destDir = chosen;
    }

    const entries = this.selectedEntries;
    this.setBusy(true);
    this.startTransfer(t("fileBrowser.transferDownloading"), entries.length);

    let done = 0;
    let errors = 0;
    let aborted = false;

    for (const entry of entries) {
      if (this.transferProgress?.cancelled) {
        aborted = true;
        break;
      }

      const remotePath = joinPath(this.currentPath, entry.name);
      const localDest = destDir.replace(/\/?$/, "/") + entry.name;
      try {
        if (entry.is_dir) {
          await api.downloadDirectory(this.profileId, remotePath, localDest);
        } else {
          await api.downloadFileTo(this.profileId, remotePath, localDest);
        }
        done++;
      } catch {
        errors++;
      }

      this.updateTransfer(done + errors);
    }

    this.endTransfer();

    if (aborted) {
      const doneStr = t("fileBrowser.downloadedCount", { count: done });
      const failedStr = errors > 0 ? `, ${t("fileBrowser.failedCount", { count: errors })}` : "";
      this.onStatusMessage?.(
        t("fileBrowser.downloadCancelledStatus", { done: doneStr, failed: failedStr }),
        false
      );
    } else if (errors === 0) {
      this.onStatusMessage?.(
        t("fileBrowser.downloadComplete", { done: t("fileBrowser.downloadedCount", { count: done }), dest: destDir }),
        false
      );
    } else {
      this.onStatusMessage?.(
        t("fileBrowser.downloadCompleteErrors", {
          done: t("fileBrowser.downloadedCount", { count: done }),
          failed: t("fileBrowser.failedCount", { count: errors }),
        }),
        true
      );
    }

    this.setBusy(false);
    await this.refresh();
  }

  // ── Rename ────────────────────────────────────────────────────────────────

  private async handleRename(): Promise<void> {
    if (!this.profileId || this.selectedNames.size !== 1) return;
    const name = [...this.selectedNames][0];

    const newName = await showPrompt(t("fileBrowser.renameTitle"), t("fileBrowser.renamePlaceholder"), name);
    if (!newName || newName === name) return;

    if (newName.includes("/")) {
      this.onStatusMessage?.(t("fileBrowser.nameContainsSlash"), true);
      return;
    }

    const fromPath = joinPath(this.currentPath, name);
    const toPath = joinPath(this.currentPath, newName);

    try {
      this.setBusy(true);
      await api.renameFile(this.profileId, fromPath, toPath);
      this.onStatusMessage?.(t("fileBrowser.renamedTo", { name: newName }), false);
      this.clearSelection();
      await this.refresh();
    } catch (err) {
      this.onStatusMessage?.(t("fileBrowser.renameFailed", { error: this.normalizeRemoteError(err) }), true);
    } finally {
      this.setBusy(false);
    }
  }

  // ── Move to… (prompted path) ──────────────────────────────────────────────

  private async handleMoveTo(): Promise<void> {
    if (!this.profileId || this.selectedNames.size === 0) return;

    const names = [...this.selectedNames];
    const label = names.length === 1 ? `"${names[0]}"` : `${names.length} items`;

    const targetDir = await showPrompt(
      t("fileBrowser.moveToTitle", { label }),
      t("fileBrowser.moveToPlaceholder"),
      this.currentPath
    );
    if (!targetDir) return;

    await this.moveNamesToDir(names, targetDir.replace(/\/?$/, ""));
  }

  // ── Move (drag-and-drop) ──────────────────────────────────────────────────

  private async handleMove(targetDirName: string): Promise<void> {
    if (!this.profileId || this.dragSourceNames.size === 0) return;

    const targetDir = targetDirName === ".."
      ? parentPath(this.currentPath)
      : joinPath(this.currentPath, targetDirName);

    const names = [...this.dragSourceNames];
    this.dragSourceNames = new Set();
    this.setDropTarget(null);

    await this.moveNamesToDir(names, targetDir);
  }

  /** Move a list of names (from the current directory) into targetDir. */
  private async moveNamesToDir(names: string[], targetDir: string): Promise<void> {
    if (!this.profileId) return;

    this.setBusy(true);
    let moved = 0;
    let errors = 0;
    const failedItems: string[] = [];

    for (const name of names) {
      const fromPath = joinPath(this.currentPath, name);
      const toPath = targetDir.replace(/\/?$/, "/") + name;
      // Skip if source and dest are identical
      if (fromPath === toPath) continue;
      try {
        await api.renameFile(this.profileId, fromPath, toPath);
        moved++;
      } catch (err) {
        errors++;
        failedItems.push(`${name}: ${this.normalizeRemoteError(err)}`);
      }
    }

    const itemLabel = moved === 1 ? t("fileBrowser.itemSingular") : t("fileBrowser.itemPlural");
    if (errors === 0) {
      this.onStatusMessage?.(t("fileBrowser.movedItems", { count: moved, itemLabel }), false);
    } else {
      const detail = failedItems.slice(0, 2).join("; ");
      const detailStr = detail ? ` (${detail}${failedItems.length > 2 ? "; …" : ""})` : "";
      this.onStatusMessage?.(
        t("fileBrowser.movedItemsErrors", { count: moved, itemLabel, errors, detail: detailStr }),
        true
      );
    }

    this.clearSelection();
    this.setBusy(false);
    await this.refresh();
  }

  // ── Edit ──────────────────────────────────────────────────────────────────

  private async handleEdit(): Promise<void> {
    if (!this.profileId || !this.selectedRemotePath) return;
    try {
      this.setBusy(true);
      await api.openForEdit(this.profileId, this.selectedRemotePath);
      this.onStatusMessage?.(t("fileBrowser.editOpened"), false);
    } catch (err) {
      this.onStatusMessage?.(t("fileBrowser.editFailed", { error: String(err) }), true);
    } finally {
      this.setBusy(false);
    }
  }

  // ── New File / Folder ─────────────────────────────────────────────────────

  private async handleNewFile(): Promise<void> {
    if (!this.profileId) return;
    const name = await showPrompt(t("fileBrowser.newFileTitle"), t("fileBrowser.newFilePlaceholder"));
    if (!name) return;

    const remotePath = joinPath(this.currentPath, name);
    try {
      this.setBusy(true);
      await api.uploadFileBytes(this.profileId, remotePath, []);
      this.onStatusMessage?.(t("fileBrowser.createdFile", { name }), false);
      await this.refresh();
    } catch (err) {
      this.onStatusMessage?.(t("fileBrowser.createFileFailed", { error: String(err) }), true);
    } finally {
      this.setBusy(false);
    }
  }

  private async handleNewFolder(): Promise<void> {
    if (!this.profileId) return;
    const name = await showPrompt(t("fileBrowser.newFolderTitle"), t("fileBrowser.newFolderPlaceholder"));
    if (!name) return;

    const remotePath = joinPath(this.currentPath, name);
    try {
      this.setBusy(true);
      await api.createDirectory(this.profileId, remotePath);
      this.onStatusMessage?.(t("fileBrowser.createdFolder", { name }), false);
      await this.refresh();
    } catch (err) {
      this.onStatusMessage?.(t("fileBrowser.createFolderFailed", { error: String(err) }), true);
    } finally {
      this.setBusy(false);
    }
  }

  // ── Delete ────────────────────────────────────────────────────────────────

  private async handleDelete(): Promise<void> {
    if (!this.profileId || this.selectedNames.size === 0) return;

    const names = [...this.selectedNames];

    if (names.length > 1) {
      await this.handleDeleteMulti(names);
      return;
    }

    // Single entry delete (original behavior)
    const entry = this.selectedEntry;
    const remotePath = this.selectedRemotePath;
    if (!entry || !remotePath) return;

    if (entry.is_dir) {
      const confirmed = await showConfirm(
        t("fileBrowser.deleteFolderMsg", { name: entry.name }),
        t("fileBrowser.deleteFolderTitle")
      );
      if (!confirmed) return;

      try {
        this.setBusy(true);
        await api.deleteDirectory(this.profileId, remotePath);
        this.onStatusMessage?.(t("fileBrowser.deletedFolder", { name: entry.name }), false);
        this.clearSelection();
        await this.refresh();
      } catch (err) {
        this.onStatusMessage?.(t("fileBrowser.deleteFailed", { error: String(err) }), true);
      } finally {
        this.setBusy(false);
      }
    } else {
      const confirmed = await showConfirm(
        t("fileBrowser.deleteFileMsg", { name: entry.name }),
        t("fileBrowser.deleteFileTitle")
      );
      if (!confirmed) return;

      try {
        this.setBusy(true);
        await api.deleteFile(this.profileId, remotePath);
        this.onStatusMessage?.(t("fileBrowser.deletedFile", { name: entry.name }), false);
        this.clearSelection();
        await this.refresh();
      } catch (err) {
        this.onStatusMessage?.(t("fileBrowser.deleteFailed", { error: String(err) }), true);
      } finally {
        this.setBusy(false);
      }
    }
  }

  private async handleDeleteMulti(names: string[]): Promise<void> {
    if (!this.profileId) return;

    const confirmed = await showConfirm(
      t("fileBrowser.deleteMultiMsg", { count: names.length }),
      t("fileBrowser.deleteMultiTitle", { count: names.length })
    );
    if (!confirmed) return;

    this.setBusy(true);
    let deleted = 0;
    let errors = 0;

    for (const name of names) {
      const entry = this.entries.find((e) => e.name === name);
      if (!entry) continue;
      const remotePath = joinPath(this.currentPath, name);
      try {
        if (entry.is_dir) {
          await api.deleteDirectory(this.profileId, remotePath);
        } else {
          await api.deleteFile(this.profileId, remotePath);
        }
        deleted++;
      } catch {
        errors++;
      }
    }

    if (errors === 0) {
      this.onStatusMessage?.(t("fileBrowser.deletedItems", { count: deleted }), false);
    } else {
      this.onStatusMessage?.(t("fileBrowser.deletedItemsErrors", { count: deleted, errors }), true);
    }

    this.clearSelection();
    this.setBusy(false);
    await this.refresh();
  }

  // ── Context menu ───────────────────────────────────────────────────────────

  private buildFileContextItems(_name: string): CtxMenuItem[] {
    return [
      { icon: ICONS.download, label: t("fileBrowser.download"), action: () => this.handleDownload() },
      { icon: ICONS.edit,     label: t("fileBrowser.edit"),     action: () => this.handleEdit() },
      { separator: true },
      { icon: ICONS.rename,   label: t("fileBrowser.rename"),   action: () => this.handleRename() },
      { icon: ICONS.moveTo,   label: t("fileBrowser.moveTo"),   action: () => this.handleMoveTo() },
      { separator: true },
      { icon: ICONS.delete,   label: t("fileBrowser.delete"),   action: () => this.handleDelete(), danger: true },
    ];
  }

  private buildFolderContextItems(name: string): CtxMenuItem[] {
    return [
      { icon: ICONS.openFolder, label: t("fileBrowser.openFolder"), action: () => this.navigateInto(name) },
      { icon: ICONS.download,   label: t("fileBrowser.download"),   action: () => this.handleDownload() },
      { separator: true },
      { icon: ICONS.rename,     label: t("fileBrowser.rename"),     action: () => this.handleRename() },
      { icon: ICONS.moveTo,     label: t("fileBrowser.moveTo"),     action: () => this.handleMoveTo() },
      { separator: true },
      { icon: ICONS.delete,     label: t("fileBrowser.delete"),     action: () => this.handleDelete(), danger: true },
    ];
  }

  private buildEmptyContextItems(): CtxMenuItem[] {
    return [
      { icon: ICONS.upload,       label: t("fileBrowser.upload"),       action: () => this.handleUpload() },
      { icon: ICONS.uploadFolder, label: t("fileBrowser.uploadFolder"), action: () => this.handleUploadFolder() },
      { separator: true },
      { icon: ICONS.newFile,      label: t("fileBrowser.newFile"),      action: () => this.handleNewFile() },
      { icon: ICONS.newFolder,    label: t("fileBrowser.newFolder"),    action: () => this.handleNewFolder() },
      { separator: true },
      { icon: ICONS.refresh,      label: t("fileBrowser.refresh"),      action: () => this.refresh() },
    ];
  }

  private showContextMenu(x: number, y: number, items: CtxMenuItem[]): void {
    this.closeContextMenu();

    const menu = document.createElement("div");
    menu.className = "ctx-menu";
    menu.id = "ctx-menu";
    menu.style.left = `${x}px`;
    menu.style.top = `${y}px`;

    for (const item of items) {
      if ("separator" in item) {
        const sep = document.createElement("div");
        sep.className = "ctx-menu__sep";
        menu.appendChild(sep);
      } else {
        const btn = document.createElement("button");
        btn.className = `ctx-menu__item${item.danger ? " ctx-menu__item--danger" : ""}`;
        btn.innerHTML = `<span class="ctx-menu__icon">${item.icon}</span><span>${escHtml(item.label)}</span>`;
        btn.addEventListener("click", () => {
          this.closeContextMenu();
          item.action();
        });
        menu.appendChild(btn);
      }
    }

    document.body.appendChild(menu);

    // Adjust so menu stays within viewport
    const rect = menu.getBoundingClientRect();
    if (rect.right > window.innerWidth) menu.style.left = `${x - rect.width}px`;
    if (rect.bottom > window.innerHeight) menu.style.top = `${y - rect.height}px`;

    // Close on any outside click or another right-click
    const closeHandler = (ev: MouseEvent) => {
      if (!menu.contains(ev.target as Node)) {
        this.closeContextMenu();
        document.removeEventListener("click", closeHandler, true);
        document.removeEventListener("contextmenu", closeHandler, true);
      }
    };
    setTimeout(() => {
      document.addEventListener("click", closeHandler, true);
      document.addEventListener("contextmenu", closeHandler, true);
    }, 0);
  }

  private closeContextMenu(): void {
    document.getElementById("ctx-menu")?.remove();
  }
}
