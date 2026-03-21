import * as api from "../api/index";
import { showConfirm } from "./dialog";
import type { FileEntry } from "../types";

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
  private fileInput: HTMLInputElement;

  private profileId: string | null = null;
  private currentPath: string = "/";
  private entries: FileEntry[] = [];
  private selectedName: string | null = null;
  private busy: boolean = false;

  private onStatusMessage: ((msg: string, isError: boolean) => void) | null = null;

  constructor(containerId: string) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;

    // Persistent hidden file input — avoids recreating on each render
    this.fileInput = document.createElement("input");
    this.fileInput.type = "file";
    this.fileInput.style.display = "none";
    document.body.appendChild(this.fileInput);
    this.fileInput.addEventListener("change", () => this.handleUploadFileSelected());

    this.renderEmpty();
  }

  setProfile(profileId: string, defaultPath: string = "/"): void {
    this.profileId = profileId;
    this.currentPath = defaultPath;
    this.selectedName = null;
  }

  /** Provide a callback to surface status messages (download path, errors, etc.) */
  setStatusCallback(cb: (msg: string, isError: boolean) => void): void {
    this.onStatusMessage = cb;
  }

  async refresh(): Promise<void> {
    if (!this.profileId) return;
    this.setBusy(true);
    try {
      this.entries = await api.listDirectory(this.profileId, this.currentPath);
      this.selectedName = null;
      this.render();
    } catch (err) {
      this.renderError(String(err));
    } finally {
      this.setBusy(false);
    }
  }

  private setBusy(value: boolean): void {
    this.busy = value;
  }

  private get selectedEntry(): FileEntry | null {
    if (!this.selectedName) return null;
    return this.entries.find((e) => e.name === this.selectedName) ?? null;
  }

  private get selectedRemotePath(): string | null {
    if (!this.selectedName) return null;
    return joinPath(this.currentPath, this.selectedName);
  }

  private renderEmpty(): void {
    this.container.innerHTML = `
      <div class="file-browser file-browser--empty">
        <p>Connect to a profile to browse files.</p>
      </div>
    `;
  }

  private renderError(message: string): void {
    this.container.innerHTML = `
      <div class="file-browser file-browser--error">
        <p>${message}</p>
      </div>
    `;
  }

  private render(): void {
    const isAtRoot = this.currentPath === "/";

    const upRow = isAtRoot
      ? ""
      : `<tr class="file-entry file-entry--dir file-entry--up" data-name="..">
           <td colspan="2">.. (up)</td>
         </tr>`;

    const rows =
      this.entries.length === 0 && isAtRoot
        ? '<tr><td colspan="2" class="empty-dir">Empty directory</td></tr>'
        : this.entries
            .map(
              (entry) =>
                `<tr class="file-entry${entry.is_dir ? " file-entry--dir" : ""}${entry.name === this.selectedName ? " file-entry--selected" : ""}"
                    data-name="${entry.name}" data-isdir="${entry.is_dir}">
                   <td>${entry.is_dir ? "&#128193; " : ""}${entry.name}</td>
                   <td>${entry.size != null && !entry.is_dir ? formatBytes(entry.size) : "—"}</td>
                 </tr>`
            )
            .join("");

    const hasFile = this.selectedEntry !== null && !this.selectedEntry.is_dir;
    const hasAny = this.selectedEntry !== null;

    this.container.innerHTML = `
      <div class="file-browser">
        <div class="file-browser__toolbar">
          <span class="file-browser__path">${this.currentPath}</span>
          <button id="refresh-btn" ${this.busy ? "disabled" : ""}>Refresh</button>
        </div>
        <table class="file-browser__table">
          <thead>
            <tr><th>Name</th><th>Size</th></tr>
          </thead>
          <tbody>${upRow}${rows}</tbody>
        </table>
        <div class="file-browser__actions">
          <button id="upload-btn" ${this.busy ? "disabled" : ""}>Upload</button>
          <button id="download-btn" ${!hasFile || this.busy ? "disabled" : ""}>Download</button>
          <button id="edit-btn"     ${!hasFile || this.busy ? "disabled" : ""}>Edit</button>
          <button id="delete-btn"   ${!hasAny  || this.busy ? "disabled" : ""}>Delete</button>
        </div>
      </div>
    `;

    // Event delegation on the table body
    this.container.querySelector("tbody")?.addEventListener("click", (e) => {
      const row = (e.target as HTMLElement).closest("tr.file-entry") as HTMLElement | null;
      if (!row) return;
      const name = row.dataset.name;
      if (name === "..") {
        this.navigateUp();
      } else if (name) {
        this.selectedName = name;
        this.render();
      }
    });

    this.container.querySelector("tbody")?.addEventListener("dblclick", (e) => {
      const row = (e.target as HTMLElement).closest("tr.file-entry") as HTMLElement | null;
      if (!row) return;
      const name = row.dataset.name;
      const isDir = row.dataset.isdir === "true";
      if (name === "..") {
        this.navigateUp();
      } else if (name && isDir) {
        this.navigateInto(name);
      }
    });

    document
      .getElementById("refresh-btn")
      ?.addEventListener("click", () => this.refresh());

    document
      .getElementById("upload-btn")
      ?.addEventListener("click", () => this.fileInput.click());

    document
      .getElementById("download-btn")
      ?.addEventListener("click", () => this.handleDownload());

    document
      .getElementById("edit-btn")
      ?.addEventListener("click", () => this.handleEdit());

    document
      .getElementById("delete-btn")
      ?.addEventListener("click", () => this.handleDelete());
  }

  private navigateInto(dirName: string): void {
    this.currentPath = joinPath(this.currentPath, dirName);
    this.selectedName = null;
    this.refresh();
  }

  private navigateUp(): void {
    this.currentPath = parentPath(this.currentPath);
    this.selectedName = null;
    this.refresh();
  }

  // ── Action handlers ──────────────────────────────────────────────────────

  private async handleUploadFileSelected(): Promise<void> {
    const file = this.fileInput.files?.[0];
    if (!file || !this.profileId) return;

    // Reset so the same file can be re-uploaded
    this.fileInput.value = "";

    const remotePath = joinPath(this.currentPath, file.name);

    try {
      this.setBusy(true);
      const buffer = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buffer));
      await api.uploadFileBytes(this.profileId, remotePath, bytes);
      this.onStatusMessage?.(`Uploaded ${file.name}`, false);
      await this.refresh();
    } catch (err) {
      this.onStatusMessage?.(`Upload failed: ${err}`, true);
    } finally {
      this.setBusy(false);
    }
  }

  private async handleDownload(): Promise<void> {
    if (!this.profileId || !this.selectedRemotePath) return;
    try {
      this.setBusy(true);
      const savePath = await api.downloadFile(this.profileId, this.selectedRemotePath);
      this.onStatusMessage?.(`Downloaded to ${savePath}`, false);
    } catch (err) {
      this.onStatusMessage?.(`Download failed: ${err}`, true);
    } finally {
      this.setBusy(false);
    }
  }

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

  private async handleDelete(): Promise<void> {
    if (!this.profileId || !this.selectedRemotePath || !this.selectedEntry) return;

    const entry = this.selectedEntry;
    const confirmed = await showConfirm(
      `Delete "${entry.name}" on the remote server? This cannot be undone.`,
      "Delete File"
    );
    if (!confirmed) return;

    try {
      this.setBusy(true);
      if (entry.is_dir) {
        // Phase 2 scope: directory delete not implemented (requires recursive delete or empty dir)
        this.onStatusMessage?.(
          "Directory delete not supported yet. Remove contents first, then delete via SSH.",
          true
        );
        return;
      }
      await api.deleteFile(this.profileId, this.selectedRemotePath);
      this.onStatusMessage?.(`Deleted ${entry.name}`, false);
      this.selectedName = null;
      await this.refresh();
    } catch (err) {
      this.onStatusMessage?.(`Delete failed: ${err}`, true);
    } finally {
      this.setBusy(false);
    }
  }
}
