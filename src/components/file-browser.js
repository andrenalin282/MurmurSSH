import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "../api/index";
import { showConfirm, showPrompt, showOverwriteDialog } from "./dialog";
import { t } from "../i18n/index";

function escHtml(s) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function joinPath(dir, name) {
  return dir.replace(/\/?$/, "/") + name;
}

function parentPath(path) {
  const parts = path.split("/").filter((p) => p.length > 0);
  parts.pop();
  return parts.length === 0 ? "/" : "/" + parts.join("/");
}

export class FileBrowser {
  constructor(containerId) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;
    this.profileId = null;
    this.localPath = null;
    this.currentPath = "/";
    this.homePath = "/";
    this.entries = [];
    this.selectedNames = new Set();
    this.anchorName = null;
    this.dragSourceNames = new Set();
    this.dropTargetName = null;
    this.isDraggingInternal = false;
    this.busy = false;
    this.inlineError = null;
    this.isDragOver = false;
    this.transferProgress = null;
    this.onStatusMessage = null;
    this.onDisconnectCallback = null;
    this.uploadApplyToAllDecision = null;
    this.renderEmpty();
    this.setupDragDrop();
  }

  setupDragDrop() {
    getCurrentWindow()
      .onDragDropEvent((event) => {
        const type = event.payload.type;
        if (type === "enter" || type === "over") {
          if (this.profileId && !this.busy && !this.isDraggingInternal) {
            this.setDragOver(true);
          }
        } else if (type === "leave") {
          this.setDragOver(false);
        } else if (type === "drop") {
          this.setDragOver(false);
          if (this.profileId && !this.busy && !this.isDraggingInternal) {
            const paths = event.payload.paths;
            if (paths.length > 0) {
              void this.uploadPathList(paths);
            }
          }
        }
      })
      .then(() => {})
      .catch(() => {});
  }

  setDragOver(value) {
    if (this.isDragOver === value) return;
    this.isDragOver = value;
    const el = this.container.querySelector(".file-browser");
    if (el) {
      el.classList.toggle("file-browser--dragover", value);
    }
  }

  setProfile(profileId, defaultPath = "/", localPath = null) {
    this.profileId = profileId;
    this.localPath = localPath;
    this.currentPath = defaultPath;
    this.homePath = defaultPath;
    this.clearSelection();
  }

  setStatusCallback(cb) {
    this.onStatusMessage = cb;
  }

  onDisconnect(callback) {
    this.onDisconnectCallback = callback;
  }

  async refresh() {
    if (!this.profileId) return;
    this.setBusy(true);
    try {
      this.entries = await api.listDirectory(this.profileId, this.currentPath);
      this.clearSelection();
      this.inlineError = null;
      this.setBusy(false);
      this.render();
    } catch (err) {
      this.inlineError = String(err);
      this.setBusy(false);
      this.render();
    }
  }

  clearSelection() {
    this.selectedNames.clear();
    this.anchorName = null;
  }

  setBusy(value) {
    this.busy = value;
  }

  get selectedEntry() {
    if (this.selectedNames.size !== 1) return null;
    const name = [...this.selectedNames][0];
    return this.entries.find((e) => e.name === name) ?? null;
  }

  get selectedEntries() {
    return this.entries.filter((e) => this.selectedNames.has(e.name));
  }

  get selectedRemotePath() {
    const entry = this.selectedEntry;
    if (!entry) return null;
    return joinPath(this.currentPath, entry.name);
  }

  renderEmpty() {
    this.container.innerHTML = `
      <div class="file-browser file-browser--empty">
        <div class="file-browser__toolbar">
          <button id="disconnect-btn" disabled>🚫 ${t("fileBrowser.disconnect")}</button>
          <button id="terminal-btn" disabled>💻 ${t("fileBrowser.terminal")}</button>
          <button id="home-btn" disabled>🏠 ${t("fileBrowser.home")}</button>
          <button id="up-btn" disabled>⬆️ ${t("fileBrowser.up")}</button>
          <button id="refresh-btn" disabled>🔄 ${t("fileBrowser.refresh")}</button>
        </div>
        <p>${t("fileBrowser.connectPrompt")}</p>
      </div>
    `;
  }

  render() {
    const scrollEl = this.container.querySelector(".file-browser__scroll");
    const savedScrollTop = scrollEl?.scrollTop ?? 0;

    const isAtRoot = this.currentPath === "/";
    const hasProfile = this.profileId !== null;
    const selCount = this.selectedNames.size;
    const singleEntry = this.selectedEntry;
    const hasAny = selCount > 0;
    const hasExactlyOne = selCount === 1;
    const hasFile = hasExactlyOne && singleEntry !== null && !singleEntry.is_dir;
    const hasDir = hasExactlyOne && singleEntry !== null && singleEntry.is_dir;

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
            .map((entry) => {
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
            })
            .join("");

    const selectionInfo = selCount > 1
      ? `<div class="file-browser__selection-info">${t("fileBrowser.itemsSelected", { count: selCount })}</div>`
      : selCount === 1
      ? `<div class="file-browser__selection-info">${t("fileBrowser.oneItemSelected")}</div>`
      : "";

    const inlineErrorHtml = this.inlineError
      ? `<div class="file-browser__inline-error">${escHtml(this.inlineError)}</div>`
      : "";

    const downloadLabel = selCount > 1
      ? t("fileBrowser.downloadMultiple", { count: selCount })
      : hasDir
      ? t("fileBrowser.downloadFolder")
      : t("fileBrowser.download");
    const downloadDisabled = !hasAny || this.busy;

    const deleteLabel = selCount > 1
      ? t("fileBrowser.deleteMultiple", { count: selCount })
      : t("fileBrowser.delete");

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
          <button id="disconnect-btn" ${!hasProfile || this.busy ? "disabled" : ""}>${t("fileBrowser.disconnect")}</button>
          <button id="terminal-btn"   ${!hasProfile || this.busy ? "disabled" : ""}>${t("fileBrowser.terminal")}</button>
          <button id="home-btn"       ${!hasProfile || this.busy ? "disabled" : ""}>${t("fileBrowser.home")}</button>
          <button id="up-btn"         ${isAtRoot || this.busy ? "disabled" : ""}>${t("fileBrowser.up")}</button>
          <button id="refresh-btn"    ${this.busy ? "disabled" : ""}>${t("fileBrowser.refresh")}</button>
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
          <button id="upload-btn"        ${!hasProfile || this.busy ? "disabled" : ""}>${t("fileBrowser.upload")}</button>
          <button id="upload-folder-btn" ${!hasProfile || this.busy ? "disabled" : ""}>${t("fileBrowser.uploadFolder")}</button>
          <button id="download-btn"      ${downloadDisabled ? "disabled" : ""}>${downloadLabel}</button>
          <button id="rename-btn"        ${!hasExactlyOne || this.busy ? "disabled" : ""}>${t("fileBrowser.rename")}</button>
          <button id="move-btn"          ${!hasAny || this.busy ? "disabled" : ""}>${t("fileBrowser.moveTo")}</button>
          <button id="edit-btn"          ${!hasFile || this.busy ? "disabled" : ""}>${t("fileBrowser.edit")}</button>
          <button id="delete-btn"        ${!hasAny  || this.busy ? "disabled" : ""}>${deleteLabel}</button>
          <button id="new-file-btn"      ${!hasProfile || this.busy ? "disabled" : ""}>${t("fileBrowser.newFile")}</button>
          <button id="new-folder-btn"    ${!hasProfile || this.busy ? "disabled" : ""}>${t("fileBrowser.newFolder")}</button>
        </div>
        ${transferProgressHtml}
      </div>
    `;

    const newScrollEl = this.container.querySelector(".file-browser__scroll");
    if (newScrollEl && savedScrollTop > 0) {
      newScrollEl.scrollTop = savedScrollTop;
    }

    const pathInput = this.container.querySelector("#path-input");
    pathInput?.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        const p = pathInput.value.trim() || "/";
        this.navigateToPath(p);
      } else if (e.key === "Escape") {
        pathInput.value = this.currentPath;
        pathInput.blur();
      }
    });

    const tbody = this.container.querySelector("tbody");
    if (tbody) {
      tbody.addEventListener("click", (e) => {
        const row = e.target.closest("tr.file-entry");
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

        if (e.ctrlKey || e.metaKey) {
          if (this.selectedNames.has(name)) {
            this.selectedNames.delete(name);
          } else {
            this.selectedNames.add(name);
            this.anchorName = name;
          }
        } else if (e.shiftKey && this.anchorName) {
          const names = this.entries.map((e) => e.name);
          const anchorIdx = names.indexOf(this.anchorName);
          const curIdx = names.indexOf(name);
          if (anchorIdx >= 0 && curIdx >= 0) {
            const from = Math.min(anchorIdx, curIdx);
            const to = Math.max(anchorIdx, curIdx);
            for (let i = from; i <= to; i++) this.selectedNames.add(names[i]);
          }
        } else {
          this.selectedNames.clear();
          this.selectedNames.add(name);
          this.anchorName = name;
        }
        this.render();
      });

      tbody.addEventListener("dblclick", (e) => {
        const row = e.target.closest("tr.file-entry");
        if (!row) return;
        const name = row.dataset.name;
        const isDir = row.dataset.isdir === "true";
        if (name === "..") {
          this.navigateUp();
        } else if (name && isDir) {
          this.navigateInto(name);
        }
      });

      tbody.addEventListener("dragstart", (e) => {
        const row = e.target.closest("tr.file-entry");
        if (!row || row.dataset.name === "..") {
          e.preventDefault();
          return;
        }
        const name = row.dataset.name;
        if (this.selectedNames.has(name)) {
          this.dragSourceNames = new Set(this.selectedNames);
        } else {
          this.dragSourceNames = new Set([name]);
        }
        this.isDraggingInternal = true;
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", "internal-move");
      });

      tbody.addEventListener("dragover", (e) => {
        if (!this.isDraggingInternal) return;
        const row = e.target.closest("tr.file-entry");
        if (!row) {
          this.setDropTarget(null);
          return;
        }
        const name = row.dataset.name;
        const isDir = row.dataset.isdir === "true";

        if (name === ".." && !isAtRoot) {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
          this.setDropTarget("..");
          return;
        }

        if (isDir && name && !this.dragSourceNames.has(name)) {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
          this.setDropTarget(name);
        } else {
          this.setDropTarget(null);
        }
      });

      tbody.addEventListener("dragleave", (e) => {
        if (!tbody.contains(e.relatedTarget)) {
          this.setDropTarget(null);
        }
      });

      tbody.addEventListener("drop", (e) => {
        e.preventDefault();
        if (!this.isDraggingInternal) return;
        const row = e.target.closest("tr.file-entry");
        if (!row) return;
        const targetName = row.dataset.name;
        const isDir = row.dataset.isdir === "true";
        if (targetName !== ".." && (!isDir || this.dragSourceNames.has(targetName))) return;
        void this.handleMove(targetName);
      });

      tbody.addEventListener("dragend", () => {
        this.isDraggingInternal = false;
        this.dragSourceNames = new Set();
        this.setDropTarget(null);
      });
    }

    document.getElementById("disconnect-btn")?.addEventListener("click", () => this.handleDisconnect());
    document.getElementById("terminal-btn")?.addEventListener("click", () => this.handleTerminal());
    document.getElementById("home-btn")?.addEventListener("click", () => this.navigateTo(this.homePath));
    document.getElementById("up-btn")?.addEventListener("click", () => this.navigateUp());
    document.getElementById("refresh-btn")?.addEventListener("click", () => this.refresh());
    document.getElementById("upload-btn")?.addEventListener("click", () => this.handleUpload());
    document.getElementById("upload-folder-btn")?.addEventListener("click", () => this.handleUploadFolder());
    document.getElementById("download-btn")?.addEventListener("click", () => this.handleDownload());
    document.getElementById("rename-btn")?.addEventListener("click", () => this.handleRename());
    document.getElementById("move-btn")?.addEventListener("click", () => this.handleMoveTo());
    document.getElementById("edit-btn")?.addEventListener("click", () => this.handleEdit());
    document.getElementById("delete-btn")?.addEventListener("click", () => this.handleDelete());
    document.getElementById("new-file-btn")?.addEventListener("click", () => this.handleNewFile());
    document.getElementById("new-folder-btn")?.addEventListener("click", () => this.handleNewFolder());

    document.getElementById("transfer-cancel-btn")?.addEventListener("click", () => {
      if (!this.transferProgress) return;
      this.transferProgress.cancelled = true;
      const btn = document.getElementById("transfer-cancel-btn");
      if (btn) { btn.disabled = true; btn.textContent = t("common.cancelling"); }
      const statusEl = document.getElementById("transfer-status");
      if (statusEl) statusEl.textContent = t("common.cancelling");
    });
  }

  startTransfer(label, total) {
    this.transferProgress = { label, current: 0, total, cancelled: false };
    this.render();
  }

  updateTransfer(current) {
    if (!this.transferProgress) return;
    this.transferProgress.current = current;
    const el = document.getElementById("transfer-status");
    if (el) el.textContent = `${this.transferProgress.label}: ${current} / ${this.transferProgress.total} files`;
  }

  endTransfer() {
    this.transferProgress = null;
    this.uploadApplyToAllDecision = null;
  }

  normalizeRemoteError(err) {
    const msg = String(err);
    if (/file exists|already exists/i.test(msg)) return "target already exists";
    if (/no such file|not found/i.test(msg)) return "source or target path not found";
    if (/permission denied|access denied/i.test(msg)) return "permission denied";
    return msg;
  }

  async resolveOverwrite(remotePath, label) {
    if (!this.profileId) return false;
    try {
      const exists = await api.remoteFileExists(this.profileId, remotePath);
      if (!exists) return true;

      let action;
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
      return true;
    }
  }

  setDropTarget(name) {
    if (this.dropTargetName === name) return;
    this.dropTargetName = name;
    this.container.querySelectorAll("tr.file-entry").forEach((row) => {
      row.classList.toggle("file-entry--drop-target", row.dataset.name === name);
    });
  }

  async navigateInto(dirName) {
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

  navigateUp() {
    if (this.currentPath === "/") return;
    this.navigateTo(parentPath(this.currentPath));
  }

  navigateTo(path) {
    if (!this.profileId || this.busy) return;
    this.navigateToPath(path);
  }

  async navigateToPath(targetPath) {
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

  async handleTerminal() {
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

  handleDisconnect() {
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

  async handleUpload() {
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

  async uploadFileList(localPaths) {
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
      if (this.transferProgress?.cancelled) { aborted = true; break; }

      const filename = localFilePath.replace(/\\/g, "/").split("/").pop() ?? localFilePath;
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

      try {
        await api.uploadFile(profileId, localFilePath, remotePath);
        uploaded++;
      } catch {
        errors++;
      }

      this.updateTransfer(uploaded + skipped + errors);
    }

    this.endTransfer();

    if (aborted) {
      const parts = [];
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
      const parts = [];
      if (uploaded > 0) parts.push(t("fileBrowser.uploadedCount", { count: uploaded }));
      if (skipped > 0) parts.push(t("fileBrowser.skippedCount", { count: skipped }));
      if (errors > 0) parts.push(t("fileBrowser.failedCount", { count: errors }));
      this.onStatusMessage?.(parts.join(", ") || t("fileBrowser.nothingToUpload"), errors > 0);
    }

    this.setBusy(false);
    await this.refresh();
  }

  async uploadPathList(localPaths) {
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

      const name = localPath.replace(/\\/g, "/").replace(/\/$/, "").split("/").pop() ?? localPath;
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

    const parts = [];
    if (uploaded > 0) parts.push(t("fileBrowser.uploadedCount", { count: uploaded }));
    if (skipped > 0) parts.push(t("fileBrowser.skippedCount", { count: skipped }));
    if (errors > 0) parts.push(t("fileBrowser.failedCount", { count: errors }));

    if (aborted) {
      const summary = parts.join(", ");
      this.onStatusMessage?.(
        summary
          ? t("fileBrowser.uploadCancelled", { summary })
          : t("fileBrowser.uploadCancelledNoFiles"),
        false
      );
    } else {
      this.onStatusMessage?.(parts.join(", ") || t("fileBrowser.nothingToUpload"), errors > 0);
    }

    this.setBusy(false);
    await this.refresh();
  }

  async handleUploadFolder() {
    if (!this.profileId) return;

    const localFolderPath = await open({
      multiple: false,
      directory: true,
      title: t("fileBrowser.selectFolderUpload"),
      defaultPath: this.localPath ?? undefined,
    });

    if (!localFolderPath || typeof localFolderPath !== "string") return;

    const folderName = localFolderPath.replace(/\\/g, "/").replace(/\/$/, "").split("/").pop() ?? "folder";
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

  async handleDownload() {
    if (!this.profileId || this.selectedNames.size === 0) return;

    if (this.selectedNames.size > 1) {
      await this.handleDownloadMulti();
      return;
    }

    if (!this.selectedEntry) return;
    if (this.selectedEntry.is_dir) {
      await this.handleDownloadFolder();
    } else {
      await this.handleDownloadFile();
    }
  }

  async handleDownloadFile() {
    if (!this.profileId || !this.selectedRemotePath || !this.selectedEntry) return;

    let savePath;

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

  async handleDownloadFolder() {
    if (!this.profileId || !this.selectedRemotePath || !this.selectedEntry) return;

    let localDestPath;

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

  async handleDownloadMulti() {
    if (!this.profileId || this.selectedNames.size === 0) return;

    let destDir;
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

  async handleRename() {
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

  async handleMoveTo() {
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

  async handleMove(targetDirName) {
    if (!this.profileId || this.dragSourceNames.size === 0) return;

    const targetDir = targetDirName === ".."
      ? parentPath(this.currentPath)
      : joinPath(this.currentPath, targetDirName);

    const names = [...this.dragSourceNames];
    this.dragSourceNames = new Set();
    this.setDropTarget(null);

    await this.moveNamesToDir(names, targetDir);
  }

  async moveNamesToDir(names, targetDir) {
    if (!this.profileId) return;

    this.setBusy(true);
    let moved = 0;
    let errors = 0;
    const failedItems = [];

    for (const name of names) {
      const fromPath = joinPath(this.currentPath, name);
      const toPath = targetDir.replace(/\/?$/, "/") + name;
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

  async handleEdit() {
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

  async handleNewFile() {
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

  async handleNewFolder() {
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

  async handleDelete() {
    if (!this.profileId || this.selectedNames.size === 0) return;

    const names = [...this.selectedNames];

    if (names.length > 1) {
      await this.handleDeleteMulti(names);
      return;
    }

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

  async handleDeleteMulti(names) {
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
}
