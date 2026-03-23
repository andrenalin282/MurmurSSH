import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "../api/index";
import { showConfirm, showPrompt } from "./dialog";
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
/** Join a remote directory path with a filename. Handles trailing slashes. */
function joinPath(dir, name) {
    return dir.replace(/\/?$/, "/") + name;
}
/** Navigate one level up from a remote path. */
function parentPath(path) {
    const parts = path.split("/").filter((p) => p.length > 0);
    parts.pop();
    return parts.length === 0 ? "/" : "/" + parts.join("/");
}
export class FileBrowser {
    constructor(containerId) {
        this.profileId = null;
        this.localPath = null;
        this.currentPath = "/";
        this.homePath = "/";
        this.entries = [];
        this.selectedName = null;
        this.busy = false;
        this.inlineError = null;
        this.isDragOver = false;
        this.onStatusMessage = null;
        this.onDisconnectCallback = null;
        const el = document.getElementById(containerId);
        if (!el)
            throw new Error(`Element #${containerId} not found`);
        this.container = el;
        this.renderEmpty();
        this.setupDragDrop();
    }
    /** Set up the Tauri window drag-and-drop listener (once, for the lifetime of the component). */
    setupDragDrop() {
        getCurrentWindow()
            .onDragDropEvent((event) => {
            const type = event.payload.type;
            if (type === "enter" || type === "over") {
                // Show visual indicator only when connected and not busy
                if (this.profileId && !this.busy) {
                    this.setDragOver(true);
                }
            }
            else if (type === "leave") {
                this.setDragOver(false);
            }
            else if (type === "drop") {
                this.setDragOver(false);
                if (this.profileId && !this.busy) {
                    // 'drop' payload always includes paths
                    const paths = event.payload.paths;
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
    setDragOver(value) {
        if (this.isDragOver === value)
            return;
        this.isDragOver = value;
        // Toggle class on the .file-browser element without a full re-render
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
        this.selectedName = null;
    }
    /** Provide a callback to surface status messages (download path, errors, etc.) */
    setStatusCallback(cb) {
        this.onStatusMessage = cb;
    }
    /** Provide a callback invoked when the user clicks Disconnect. */
    onDisconnect(callback) {
        this.onDisconnectCallback = callback;
    }
    async refresh() {
        if (!this.profileId)
            return;
        this.setBusy(true);
        try {
            this.entries = await api.listDirectory(this.profileId, this.currentPath);
            this.selectedName = null;
            this.inlineError = null;
            this.setBusy(false);
            this.render();
        }
        catch (err) {
            // Keep toolbar buttons active — connection is still live.
            // Show the error inline so Disconnect/Terminal/Refresh remain usable.
            this.inlineError = String(err);
            this.setBusy(false);
            this.render();
        }
    }
    setBusy(value) {
        this.busy = value;
    }
    get selectedEntry() {
        if (!this.selectedName)
            return null;
        return this.entries.find((e) => e.name === this.selectedName) ?? null;
    }
    get selectedRemotePath() {
        if (!this.selectedName)
            return null;
        return joinPath(this.currentPath, this.selectedName);
    }
    renderEmpty() {
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
    renderBreadcrumbs() {
        const parts = this.currentPath.split("/").filter((p) => p.length > 0);
        // Root crumb
        if (parts.length === 0) {
            // We are at root — root is the current (non-clickable) segment
            return `<nav class="file-browser__breadcrumbs"><span class="breadcrumb__current">/</span></nav>`;
        }
        const crumbs = [];
        // Root is always a clickable link (unless it's the last segment, handled above)
        crumbs.push(`<span class="breadcrumb__link" data-path="/">/</span>`);
        parts.forEach((segment, index) => {
            crumbs.push(`<span class="breadcrumb__sep">›</span>`);
            const isLast = index === parts.length - 1;
            if (isLast) {
                crumbs.push(`<span class="breadcrumb__current">${segment}</span>`);
            }
            else {
                const segPath = "/" + parts.slice(0, index + 1).join("/");
                crumbs.push(`<span class="breadcrumb__link" data-path="${segPath}">${segment}</span>`);
            }
        });
        return `<nav class="file-browser__breadcrumbs">${crumbs.join("")}</nav>`;
    }
    render() {
        // Preserve scroll position before rebuilding the DOM (prevents scroll-jump on row click).
        // The scrollable element is .file-browser (overflow-y: auto), not the table itself.
        const scrollEl = this.container.querySelector(".file-browser");
        const savedScrollTop = scrollEl?.scrollTop ?? 0;
        const isAtRoot = this.currentPath === "/";
        const hasProfile = this.profileId !== null;
        const upRow = isAtRoot
            ? ""
            : `<tr class="file-entry file-entry--dir file-entry--up" data-name="..">
           <td colspan="2">.. (up)</td>
         </tr>`;
        const rows = this.entries.length === 0 && !this.inlineError
            ? '<tr><td colspan="2" class="empty-dir">Empty directory</td></tr>'
            : this.entries.length === 0
                ? ""
                : this.entries
                    .map((entry) => `<tr class="file-entry${entry.is_dir ? " file-entry--dir" : ""}${entry.name === this.selectedName ? " file-entry--selected" : ""}"
                    data-name="${entry.name}" data-isdir="${entry.is_dir}">
                   <td>${entry.is_dir ? "&#128193; " : ""}${entry.name}</td>
                   <td>${entry.size != null && !entry.is_dir ? formatBytes(entry.size) : "—"}</td>
                 </tr>`)
                    .join("");
        const hasFile = this.selectedEntry !== null && !this.selectedEntry.is_dir;
        const hasDir = this.selectedEntry !== null && this.selectedEntry.is_dir;
        const hasAny = this.selectedEntry !== null;
        const inlineErrorHtml = this.inlineError
            ? `<div class="file-browser__inline-error">${escHtml(this.inlineError)}</div>`
            : "";
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
        <div class="file-browser__actions">
          <button id="upload-btn"        ${!hasProfile || this.busy ? "disabled" : ""}>Upload</button>
          <button id="upload-folder-btn" ${!hasProfile || this.busy ? "disabled" : ""}>Upload Folder</button>
          <button id="download-btn"      ${(!hasFile && !hasDir) || this.busy ? "disabled" : ""}>${hasDir ? "Download Folder" : "Download"}</button>
          <button id="edit-btn"       ${!hasFile || this.busy ? "disabled" : ""}>Edit</button>
          <button id="delete-btn"     ${!hasAny || this.busy ? "disabled" : ""}>Delete</button>
          <button id="new-file-btn"   ${!hasProfile || this.busy ? "disabled" : ""}>＋ File</button>
          <button id="new-folder-btn" ${!hasProfile || this.busy ? "disabled" : ""}>＋ Folder</button>
        </div>
      </div>
    `;
        // Restore scroll position after DOM rebuild (preserves position on single-click select)
        const newScrollEl = this.container.querySelector(".file-browser");
        if (newScrollEl && savedScrollTop > 0) {
            newScrollEl.scrollTop = savedScrollTop;
        }
        // Breadcrumb navigation
        this.container.querySelectorAll(".breadcrumb__link").forEach((el) => {
            el.addEventListener("click", () => {
                const path = el.dataset.path;
                if (path)
                    this.navigateTo(path);
            });
        });
        // Path input: Enter navigates, Escape resets
        const pathInput = this.container.querySelector("#path-input");
        pathInput?.addEventListener("keydown", (e) => {
            if (e.key === "Enter") {
                const p = pathInput.value.trim() || "/";
                this.navigateToPath(p);
            }
            else if (e.key === "Escape") {
                pathInput.value = this.currentPath;
                pathInput.blur();
            }
        });
        // Event delegation on the table body
        this.container.querySelector("tbody")?.addEventListener("click", (e) => {
            const row = e.target.closest("tr.file-entry");
            if (!row)
                return;
            const name = row.dataset.name;
            if (name === "..") {
                this.navigateUp();
            }
            else if (name) {
                this.selectedName = name;
                this.render();
            }
        });
        this.container.querySelector("tbody")?.addEventListener("dblclick", (e) => {
            const row = e.target.closest("tr.file-entry");
            if (!row)
                return;
            const name = row.dataset.name;
            const isDir = row.dataset.isdir === "true";
            if (name === "..") {
                this.navigateUp();
            }
            else if (name && isDir) {
                this.navigateInto(name);
            }
        });
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
    async navigateInto(dirName) {
        if (!this.profileId || this.busy)
            return;
        const targetPath = joinPath(this.currentPath, dirName);
        this.setBusy(true);
        try {
            const newEntries = await api.listDirectory(this.profileId, targetPath);
            // Only commit path if the listing succeeded
            this.currentPath = targetPath;
            this.entries = newEntries;
            this.selectedName = null;
            this.inlineError = null;
            this.setBusy(false);
            this.render();
        }
        catch (err) {
            const msg = `Cannot open "${dirName}": ${String(err)}`;
            this.onStatusMessage?.(msg, true);
            this.inlineError = msg;
            this.setBusy(false);
            this.render(); // re-render with error banner, same path
        }
    }
    navigateUp() {
        if (this.currentPath === "/")
            return;
        this.navigateTo(parentPath(this.currentPath));
    }
    navigateTo(path) {
        if (!this.profileId || this.busy)
            return;
        // Use navigateInto logic by temporarily treating it as a direct path navigate
        this.navigateToPath(path);
    }
    async navigateToPath(targetPath) {
        if (!this.profileId || this.busy)
            return;
        this.setBusy(true);
        try {
            const newEntries = await api.listDirectory(this.profileId, targetPath);
            this.currentPath = targetPath;
            this.entries = newEntries;
            this.selectedName = null;
            this.inlineError = null;
            this.setBusy(false);
            this.render();
        }
        catch (err) {
            const msg = `Cannot navigate to "${targetPath}": ${String(err)}`;
            this.onStatusMessage?.(msg, true);
            this.inlineError = msg;
            this.setBusy(false);
            this.render(); // re-render with error banner, same path
        }
    }
    async handleTerminal() {
        if (!this.profileId)
            return;
        // Check whether the SSH key needs a local runtime copy for terminal compatibility.
        // OpenSSH rejects keys with group/other permission bits or on non-local filesystems.
        let useRuntimeCopy = false;
        try {
            const needsCopy = await api.checkKeyNeedsCopy(this.profileId);
            if (needsCopy) {
                const accepted = await showConfirm("The SSH key for this profile has permissions that OpenSSH may reject.\n\n" +
                    "MurmurSSH can create a local runtime copy of the key with correct permissions " +
                    "for this terminal session only. The original key file is not modified.\n\n" +
                    "The copy will be deleted when you disconnect.", "Copy Key Locally for Terminal?");
                if (!accepted) {
                    // User declined — try the launch anyway with the original key
                }
                else {
                    await api.copyKeyForRuntime(this.profileId);
                    useRuntimeCopy = true;
                }
            }
        }
        catch {
            // Non-fatal — if the check fails, proceed with normal launch
        }
        try {
            await api.launchSsh(this.profileId, useRuntimeCopy);
        }
        catch (err) {
            this.onStatusMessage?.(`Terminal launch failed: ${err}`, true);
        }
    }
    handleDisconnect() {
        this.profileId = null;
        this.localPath = null;
        this.currentPath = "/";
        this.homePath = "/";
        this.entries = [];
        this.selectedName = null;
        this.busy = false;
        this.inlineError = null;
        this.isDragOver = false;
        this.onDisconnectCallback?.();
        this.renderEmpty();
    }
    // ── Action handlers ──────────────────────────────────────────────────────
    async handleUpload() {
        if (!this.profileId)
            return;
        // Open file picker with multi-select; start in local_path if configured
        const result = await open({
            multiple: true,
            directory: false,
            defaultPath: this.localPath ?? undefined,
        });
        if (!result)
            return;
        // Dialog returns string[] when multiple:true, but guard defensively
        const paths = Array.isArray(result) ? result : [result];
        if (paths.length === 0)
            return;
        await this.uploadFileList(paths);
    }
    /** Upload a list of known-file paths into the current remote directory with a counter. */
    async uploadFileList(localPaths) {
        if (!this.profileId)
            return;
        const profileId = this.profileId;
        const total = localPaths.length;
        const label = total === 1 ? "file" : "files";
        this.setBusy(true);
        let uploaded = 0;
        let errors = 0;
        for (const localFilePath of localPaths) {
            const filename = localFilePath.replace(/\\/g, "/").split("/").pop() ?? localFilePath;
            const remotePath = joinPath(this.currentPath, filename);
            this.onStatusMessage?.(`Uploading… ${uploaded + 1} / ${total} ${label}`, false);
            try {
                await api.uploadFile(profileId, localFilePath, remotePath);
                uploaded++;
            }
            catch {
                errors++;
            }
        }
        if (errors === 0) {
            this.onStatusMessage?.(`Uploaded ${uploaded} ${uploaded === 1 ? "file" : "files"}`, false);
        }
        else {
            this.onStatusMessage?.(`Uploaded ${uploaded} ${uploaded === 1 ? "file" : "files"}, ${errors} failed`, true);
        }
        this.setBusy(false);
        await this.refresh();
    }
    /**
     * Upload a list of local paths (files or directories) into the current remote directory.
     * Used by drag-and-drop; the backend auto-detects file vs directory for each path.
     */
    async uploadPathList(localPaths) {
        if (!this.profileId)
            return;
        const profileId = this.profileId;
        const total = localPaths.length;
        const label = total === 1 ? "item" : "items";
        this.setBusy(true);
        let uploaded = 0;
        let errors = 0;
        for (const localPath of localPaths) {
            const name = localPath.replace(/\\/g, "/").replace(/\/$/, "").split("/").pop() ??
                localPath;
            const remotePath = joinPath(this.currentPath, name);
            this.onStatusMessage?.(`Uploading… ${uploaded + 1} / ${total} ${label}`, false);
            try {
                await api.uploadPath(profileId, localPath, remotePath);
                uploaded++;
            }
            catch {
                errors++;
            }
        }
        if (errors === 0) {
            this.onStatusMessage?.(`Uploaded ${uploaded} ${uploaded === 1 ? "item" : "items"}`, false);
        }
        else {
            this.onStatusMessage?.(`Uploaded ${uploaded} ${uploaded === 1 ? "item" : "items"}, ${errors} failed`, true);
        }
        this.setBusy(false);
        await this.refresh();
    }
    async handleUploadFolder() {
        if (!this.profileId)
            return;
        // Open folder picker; start in local_path if set so user lands in the right place
        const localFolderPath = await open({
            multiple: false,
            directory: true,
            title: "Select folder to upload",
            defaultPath: this.localPath ?? undefined,
        });
        if (!localFolderPath || typeof localFolderPath !== "string")
            return;
        // Extract folder name from the local path (strip trailing slash, then take last segment)
        const folderName = localFolderPath.replace(/\\/g, "/").replace(/\/$/, "").split("/").pop() ??
            "folder";
        const remotePath = joinPath(this.currentPath, folderName);
        try {
            this.setBusy(true);
            this.onStatusMessage?.(`Uploading folder ${folderName}…`, false);
            await api.uploadDirectory(this.profileId, localFolderPath, remotePath);
            this.onStatusMessage?.(`Uploaded folder ${folderName}`, false);
            await this.refresh();
        }
        catch (err) {
            this.onStatusMessage?.(`Folder upload failed: ${err}`, true);
        }
        finally {
            this.setBusy(false);
        }
    }
    async handleDownload() {
        if (!this.profileId || !this.selectedRemotePath || !this.selectedName || !this.selectedEntry)
            return;
        if (this.selectedEntry.is_dir) {
            await this.handleDownloadFolder();
        }
        else {
            await this.handleDownloadFile();
        }
    }
    async handleDownloadFile() {
        if (!this.profileId || !this.selectedRemotePath || !this.selectedName)
            return;
        let savePath;
        if (this.localPath) {
            // Local path set — save directly without a dialog
            savePath = this.localPath.replace(/\/?$/, "/") + this.selectedName;
        }
        else {
            // No local path — ask the user where to save
            const chosen = await save({
                defaultPath: this.selectedName,
                title: "Save file",
            });
            if (!chosen)
                return; // user cancelled
            savePath = chosen;
        }
        try {
            this.setBusy(true);
            await api.downloadFileTo(this.profileId, this.selectedRemotePath, savePath);
            this.onStatusMessage?.(`Downloaded to ${savePath}`, false);
        }
        catch (err) {
            this.onStatusMessage?.(`Download failed: ${err}`, true);
        }
        finally {
            this.setBusy(false);
        }
    }
    async handleDownloadFolder() {
        if (!this.profileId || !this.selectedRemotePath || !this.selectedName)
            return;
        let localDestPath;
        if (this.localPath) {
            // Local path set — download into that directory
            localDestPath = this.localPath.replace(/\/?$/, "/") + this.selectedName;
        }
        else {
            // No local path — ask the user to pick a destination folder
            const chosen = await open({
                multiple: false,
                directory: true,
                title: "Select destination folder",
            });
            if (!chosen || typeof chosen !== "string")
                return; // user cancelled
            // Save into a subfolder named after the remote directory
            localDestPath = chosen.replace(/\/?$/, "/") + this.selectedName;
        }
        try {
            this.setBusy(true);
            await api.downloadDirectory(this.profileId, this.selectedRemotePath, localDestPath);
            this.onStatusMessage?.(`Downloaded folder to ${localDestPath}`, false);
        }
        catch (err) {
            this.onStatusMessage?.(`Folder download failed: ${err}`, true);
        }
        finally {
            this.setBusy(false);
        }
    }
    async handleEdit() {
        if (!this.profileId || !this.selectedRemotePath)
            return;
        try {
            this.setBusy(true);
            await api.openForEdit(this.profileId, this.selectedRemotePath);
            this.onStatusMessage?.(`Opened for editing`, false);
        }
        catch (err) {
            this.onStatusMessage?.(`Edit failed: ${err}`, true);
        }
        finally {
            this.setBusy(false);
        }
    }
    async handleNewFile() {
        if (!this.profileId)
            return;
        const name = await showPrompt("New File", "filename.txt");
        if (!name)
            return;
        const remotePath = joinPath(this.currentPath, name);
        try {
            this.setBusy(true);
            await api.uploadFileBytes(this.profileId, remotePath, []);
            this.onStatusMessage?.(`Created ${name}`, false);
            await this.refresh();
        }
        catch (err) {
            this.onStatusMessage?.(`Could not create file: ${err}`, true);
        }
        finally {
            this.setBusy(false);
        }
    }
    async handleNewFolder() {
        if (!this.profileId)
            return;
        const name = await showPrompt("New Folder", "folder-name");
        if (!name)
            return;
        const remotePath = joinPath(this.currentPath, name);
        try {
            this.setBusy(true);
            await api.createDirectory(this.profileId, remotePath);
            this.onStatusMessage?.(`Created folder ${name}`, false);
            await this.refresh();
        }
        catch (err) {
            this.onStatusMessage?.(`Could not create folder: ${err}`, true);
        }
        finally {
            this.setBusy(false);
        }
    }
    async handleDelete() {
        if (!this.profileId || !this.selectedRemotePath || !this.selectedEntry)
            return;
        const entry = this.selectedEntry;
        const remotePath = this.selectedRemotePath;
        if (entry.is_dir) {
            const confirmed = await showConfirm(`"${entry.name}" is a folder. This folder and all of its contents will be deleted recursively. Proceed?`, "Delete Folder");
            if (!confirmed)
                return;
            try {
                this.setBusy(true);
                await api.deleteDirectory(this.profileId, remotePath);
                this.onStatusMessage?.(`Deleted folder ${entry.name}`, false);
                this.selectedName = null;
                await this.refresh();
            }
            catch (err) {
                this.onStatusMessage?.(`Delete failed: ${err}`, true);
            }
            finally {
                this.setBusy(false);
            }
        }
        else {
            const confirmed = await showConfirm(`Delete "${entry.name}" on the remote server? This cannot be undone.`, "Delete File");
            if (!confirmed)
                return;
            try {
                this.setBusy(true);
                await api.deleteFile(this.profileId, remotePath);
                this.onStatusMessage?.(`Deleted ${entry.name}`, false);
                this.selectedName = null;
                await this.refresh();
            }
            catch (err) {
                this.onStatusMessage?.(`Delete failed: ${err}`, true);
            }
            finally {
                this.setBusy(false);
            }
        }
    }
}
