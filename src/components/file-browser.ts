import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "../api/index";
import { showConfirm, showPrompt } from "./dialog";
import type { FileEntry } from "../types";

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

  private onStatusMessage: ((msg: string, isError: boolean) => void) | null = null;
  private onDisconnectCallback: (() => void) | null = null;

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
          <button id="disconnect-btn" disabled>Disconnect</button>
          <button id="terminal-btn" disabled>Terminal</button>
          <button id="home-btn" disabled>Home</button>
          <button id="up-btn" disabled>Up</button>
          <button id="refresh-btn" disabled>Refresh</button>
        </div>
        <p>Connect to a profile to browse files.</p>
      </div>
    `;
  }

  /** Build clickable breadcrumb HTML for currentPath. */
  private renderBreadcrumbs(): string {
    const parts = this.currentPath.split("/").filter((p) => p.length > 0);

    if (parts.length === 0) {
      return `<nav class="file-browser__breadcrumbs"><span class="breadcrumb__current">/</span></nav>`;
    }

    const crumbs: string[] = [];
    crumbs.push(`<span class="breadcrumb__link" data-path="/">/</span>`);

    parts.forEach((segment, index) => {
      crumbs.push(`<span class="breadcrumb__sep">›</span>`);

      const isLast = index === parts.length - 1;
      if (isLast) {
        crumbs.push(`<span class="breadcrumb__current">${segment}</span>`);
      } else {
        const segPath = "/" + parts.slice(0, index + 1).join("/");
        crumbs.push(`<span class="breadcrumb__link" data-path="${segPath}">${segment}</span>`);
      }
    });

    return `<nav class="file-browser__breadcrumbs">${crumbs.join("")}</nav>`;
  }

  private render(): void {
    // Preserve scroll position before rebuilding the DOM.
    const scrollEl = this.container.querySelector<HTMLElement>(".file-browser");
    const savedScrollTop = scrollEl?.scrollTop ?? 0;

    const isAtRoot = this.currentPath === "/";
    const hasProfile = this.profileId !== null;
    const selCount = this.selectedNames.size;
    const singleEntry = this.selectedEntry; // non-null only when exactly 1 selected
    const hasAny = selCount > 0;
    const hasExactlyOne = selCount === 1;
    const hasFile = hasExactlyOne && singleEntry !== null && !singleEntry.is_dir;
    const hasDir = hasExactlyOne && singleEntry !== null && singleEntry.is_dir;

    // ".." row — also a drop target when dragging items
    const upRow = isAtRoot
      ? ""
      : `<tr class="file-entry file-entry--dir file-entry--up${this.dropTargetName === ".." ? " file-entry--drop-target" : ""}" data-name=".." data-isdir="true">
           <td colspan="2">.. (up)</td>
         </tr>`;

    const rows =
      this.entries.length === 0 && !this.inlineError
        ? '<tr><td colspan="2" class="empty-dir">Empty directory</td></tr>'
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
      ? `<div class="file-browser__selection-info">${selCount} items selected</div>`
      : selCount === 1
      ? `<div class="file-browser__selection-info">1 item selected</div>`
      : "";

    const inlineErrorHtml = this.inlineError
      ? `<div class="file-browser__inline-error">${escHtml(this.inlineError)}</div>`
      : "";

    // Download button label
    const downloadLabel = selCount > 1
      ? `Download (${selCount})`
      : hasDir
      ? "Download Folder"
      : "Download";
    const downloadDisabled = !hasAny || this.busy;

    this.container.innerHTML = `
      <div class="file-browser${this.isDragOver ? " file-browser--dragover" : ""}">
        <div class="file-browser__toolbar">
          <button id="disconnect-btn" ${!hasProfile || this.busy ? "disabled" : ""}>Disconnect</button>
          <button id="terminal-btn"   ${!hasProfile || this.busy ? "disabled" : ""}>Terminal</button>
          <button id="home-btn"       ${!hasProfile || this.busy ? "disabled" : ""}>Home</button>
          <button id="up-btn"         ${isAtRoot || this.busy ? "disabled" : ""}>Up</button>
          <button id="refresh-btn"    ${this.busy ? "disabled" : ""}>Refresh</button>
        </div>
        ${this.renderBreadcrumbs()}
        <div class="file-browser__path-row">
          <input id="path-input" type="text" class="file-browser__path-input"
            value="${escHtml(this.currentPath)}" spellcheck="false" autocomplete="off">
        </div>
        ${inlineErrorHtml}
        <table class="file-browser__table">
          <thead>
            <tr><th>Name</th><th>Size</th></tr>
          </thead>
          <tbody>${upRow}${rows}</tbody>
        </table>
        ${selectionInfo}
        <div class="file-browser__actions">
          <button id="upload-btn"        ${!hasProfile || this.busy ? "disabled" : ""}>Upload</button>
          <button id="upload-folder-btn" ${!hasProfile || this.busy ? "disabled" : ""}>Upload Folder</button>
          <button id="download-btn"      ${downloadDisabled ? "disabled" : ""}>${downloadLabel}</button>
          <button id="rename-btn"        ${!hasExactlyOne || this.busy ? "disabled" : ""}>Rename</button>
          <button id="move-btn"          ${!hasAny || this.busy ? "disabled" : ""}>Move to…</button>
          <button id="edit-btn"          ${!hasFile || this.busy ? "disabled" : ""}>Edit</button>
          <button id="delete-btn"        ${!hasAny  || this.busy ? "disabled" : ""}>Delete${selCount > 1 ? ` (${selCount})` : ""}</button>
          <button id="new-file-btn"      ${!hasProfile || this.busy ? "disabled" : ""}>＋ File</button>
          <button id="new-folder-btn"    ${!hasProfile || this.busy ? "disabled" : ""}>＋ Folder</button>
        </div>
      </div>
    `;

    // Restore scroll position after DOM rebuild.
    const newScrollEl = this.container.querySelector<HTMLElement>(".file-browser");
    if (newScrollEl && savedScrollTop > 0) {
      newScrollEl.scrollTop = savedScrollTop;
    }

    // ── Breadcrumb navigation ──────────────────────────────────────────────
    this.container.querySelectorAll<HTMLElement>(".breadcrumb__link").forEach((el) => {
      el.addEventListener("click", () => {
        const path = el.dataset.path;
        if (path) this.navigateTo(path);
      });
    });

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
      const msg = `Cannot open "${dirName}": ${String(err)}`;
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
      const msg = `Cannot navigate to "${targetPath}": ${String(err)}`;
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
          "The SSH key for this profile has permissions that OpenSSH may reject.\n\n" +
          "MurmurSSH can create a local runtime copy of the key with correct permissions " +
          "for this terminal session only. The original key file is not modified.\n\n" +
          "The copy will be deleted when you disconnect.",
          "Copy Key Locally for Terminal?"
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
      this.onStatusMessage?.(`Terminal launch failed: ${err}`, true);
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
    const label = total === 1 ? "file" : "files";

    this.setBusy(true);
    let uploaded = 0;
    let errors = 0;

    for (const localFilePath of localPaths) {
      const filename =
        localFilePath.replace(/\\/g, "/").split("/").pop() ?? localFilePath;
      const remotePath = joinPath(this.currentPath, filename);

      this.onStatusMessage?.(`Uploading… ${uploaded + 1} / ${total} ${label}`, false);

      try {
        await api.uploadFile(profileId, localFilePath, remotePath);
        uploaded++;
      } catch {
        errors++;
      }
    }

    if (errors === 0) {
      this.onStatusMessage?.(`Uploaded ${uploaded} ${uploaded === 1 ? "file" : "files"}`, false);
    } else {
      this.onStatusMessage?.(
        `Uploaded ${uploaded} ${uploaded === 1 ? "file" : "files"}, ${errors} failed`,
        true
      );
    }

    this.setBusy(false);
    await this.refresh();
  }

  private async uploadPathList(localPaths: string[]): Promise<void> {
    if (!this.profileId) return;
    const profileId = this.profileId;
    const total = localPaths.length;
    const label = total === 1 ? "item" : "items";

    this.setBusy(true);
    let uploaded = 0;
    let errors = 0;

    for (const localPath of localPaths) {
      const name =
        localPath.replace(/\\/g, "/").replace(/\/$/, "").split("/").pop() ??
        localPath;
      const remotePath = joinPath(this.currentPath, name);

      this.onStatusMessage?.(`Uploading… ${uploaded + 1} / ${total} ${label}`, false);

      try {
        await api.uploadPath(profileId, localPath, remotePath);
        uploaded++;
      } catch {
        errors++;
      }
    }

    if (errors === 0) {
      this.onStatusMessage?.(`Uploaded ${uploaded} ${uploaded === 1 ? "item" : "items"}`, false);
    } else {
      this.onStatusMessage?.(
        `Uploaded ${uploaded} ${uploaded === 1 ? "item" : "items"}, ${errors} failed`,
        true
      );
    }

    this.setBusy(false);
    await this.refresh();
  }

  private async handleUploadFolder(): Promise<void> {
    if (!this.profileId) return;

    const localFolderPath = await open({
      multiple: false,
      directory: true,
      title: "Select folder to upload",
      defaultPath: this.localPath ?? undefined,
    });

    if (!localFolderPath || typeof localFolderPath !== "string") return;

    const folderName =
      localFolderPath.replace(/\\/g, "/").replace(/\/$/, "").split("/").pop() ??
      "folder";
    const remotePath = joinPath(this.currentPath, folderName);

    try {
      this.setBusy(true);
      this.onStatusMessage?.(`Uploading folder ${folderName}…`, false);
      await api.uploadDirectory(this.profileId, localFolderPath, remotePath);
      this.onStatusMessage?.(`Uploaded folder ${folderName}`, false);
      await this.refresh();
    } catch (err) {
      this.onStatusMessage?.(`Folder upload failed: ${err}`, true);
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
      this.onStatusMessage?.(`Downloaded to ${savePath}`, false);
    } catch (err) {
      this.onStatusMessage?.(`Download failed: ${err}`, true);
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
      this.onStatusMessage?.(`Downloaded folder to ${localDestPath}`, false);
    } catch (err) {
      this.onStatusMessage?.(`Folder download failed: ${err}`, true);
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
    let done = 0;
    let errors = 0;

    for (const entry of entries) {
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
    }

    if (errors === 0) {
      this.onStatusMessage?.(`Downloaded ${done} items to ${destDir}`, false);
    } else {
      this.onStatusMessage?.(`Downloaded ${done} items, ${errors} failed`, true);
    }

    this.setBusy(false);
  }

  // ── Rename ────────────────────────────────────────────────────────────────

  private async handleRename(): Promise<void> {
    if (!this.profileId || this.selectedNames.size !== 1) return;
    const name = [...this.selectedNames][0];

    const newName = await showPrompt("Rename", "new name", name);
    if (!newName || newName === name) return;

    if (newName.includes("/")) {
      this.onStatusMessage?.(`Name cannot contain "/"`, true);
      return;
    }

    const fromPath = joinPath(this.currentPath, name);
    const toPath = joinPath(this.currentPath, newName);

    try {
      this.setBusy(true);
      await api.renameFile(this.profileId, fromPath, toPath);
      this.onStatusMessage?.(`Renamed to ${newName}`, false);
      this.clearSelection();
      await this.refresh();
    } catch (err) {
      this.onStatusMessage?.(`Rename failed: ${err}`, true);
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
      `Move ${label} to directory`,
      "e.g. /home/user/docs",
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

    for (const name of names) {
      const fromPath = joinPath(this.currentPath, name);
      const toPath = targetDir.replace(/\/?$/, "/") + name;
      // Skip if source and dest are identical
      if (fromPath === toPath) continue;
      try {
        await api.renameFile(this.profileId, fromPath, toPath);
        moved++;
      } catch {
        errors++;
      }
    }

    const itemLabel = moved === 1 ? "item" : "items";
    if (errors === 0) {
      this.onStatusMessage?.(`Moved ${moved} ${itemLabel}`, false);
    } else {
      this.onStatusMessage?.(`Moved ${moved} ${itemLabel}, ${errors} failed`, true);
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
      this.onStatusMessage?.(`Opened for editing`, false);
    } catch (err) {
      this.onStatusMessage?.(`Edit failed: ${err}`, true);
    } finally {
      this.setBusy(false);
    }
  }

  // ── New File / Folder ─────────────────────────────────────────────────────

  private async handleNewFile(): Promise<void> {
    if (!this.profileId) return;
    const name = await showPrompt("New File", "filename.txt");
    if (!name) return;

    const remotePath = joinPath(this.currentPath, name);
    try {
      this.setBusy(true);
      await api.uploadFileBytes(this.profileId, remotePath, []);
      this.onStatusMessage?.(`Created ${name}`, false);
      await this.refresh();
    } catch (err) {
      this.onStatusMessage?.(`Could not create file: ${err}`, true);
    } finally {
      this.setBusy(false);
    }
  }

  private async handleNewFolder(): Promise<void> {
    if (!this.profileId) return;
    const name = await showPrompt("New Folder", "folder-name");
    if (!name) return;

    const remotePath = joinPath(this.currentPath, name);
    try {
      this.setBusy(true);
      await api.createDirectory(this.profileId, remotePath);
      this.onStatusMessage?.(`Created folder ${name}`, false);
      await this.refresh();
    } catch (err) {
      this.onStatusMessage?.(`Could not create folder: ${err}`, true);
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
        `"${entry.name}" is a folder. This folder and all of its contents will be deleted recursively. Proceed?`,
        "Delete Folder"
      );
      if (!confirmed) return;

      try {
        this.setBusy(true);
        await api.deleteDirectory(this.profileId, remotePath);
        this.onStatusMessage?.(`Deleted folder ${entry.name}`, false);
        this.clearSelection();
        await this.refresh();
      } catch (err) {
        this.onStatusMessage?.(`Delete failed: ${err}`, true);
      } finally {
        this.setBusy(false);
      }
    } else {
      const confirmed = await showConfirm(
        `Delete "${entry.name}" on the remote server? This cannot be undone.`,
        "Delete File"
      );
      if (!confirmed) return;

      try {
        this.setBusy(true);
        await api.deleteFile(this.profileId, remotePath);
        this.onStatusMessage?.(`Deleted ${entry.name}`, false);
        this.clearSelection();
        await this.refresh();
      } catch (err) {
        this.onStatusMessage?.(`Delete failed: ${err}`, true);
      } finally {
        this.setBusy(false);
      }
    }
  }

  private async handleDeleteMulti(names: string[]): Promise<void> {
    if (!this.profileId) return;

    const confirmed = await showConfirm(
      `Delete ${names.length} items? Folders will be deleted recursively. This cannot be undone.`,
      `Delete ${names.length} Items`
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
      this.onStatusMessage?.(`Deleted ${deleted} items`, false);
    } else {
      this.onStatusMessage?.(`Deleted ${deleted} items, ${errors} failed`, true);
    }

    this.clearSelection();
    this.setBusy(false);
    await this.refresh();
  }
}
