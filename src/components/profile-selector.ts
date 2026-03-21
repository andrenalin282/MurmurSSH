import * as api from "../api/index";
import type { Profile, Settings } from "../types";

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export class ProfileSelector {
  private container: HTMLElement;
  private profiles: Profile[] = [];
  private selectedId: string | null = null;
  private onConnectCallback: ((profileId: string) => void) | null = null;
  private onNewCallback: (() => void) | null = null;
  private onEditCallback: ((profile: Profile) => void) | null = null;
  private onDeleteCallback: ((profileId: string) => Promise<void>) | null = null;

  constructor(containerId: string) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;
  }

  async init(): Promise<string | null> {
    this.profiles = await api.listProfiles();
    const settings: Settings = await api.getSettings();
    this.selectedId = settings.last_used_profile_id;

    if (this.selectedId === null && this.profiles.length > 0) {
      this.selectedId = this.profiles[0].id;
    }

    this.render();
    return this.selectedId;
  }

  /** Reload profiles from backend and re-render. Preserves current selection if still valid. */
  async reload(selectId?: string): Promise<void> {
    this.profiles = await api.listProfiles();
    if (selectId !== undefined) {
      this.selectedId = selectId;
    }
    // If current selection no longer exists, fall back to first profile
    if (this.selectedId && !this.profiles.find((p) => p.id === this.selectedId)) {
      this.selectedId = this.profiles[0]?.id ?? null;
    }
    this.render();
  }

  onConnect(callback: (profileId: string) => void): void {
    this.onConnectCallback = callback;
  }

  onNew(callback: () => void): void {
    this.onNewCallback = callback;
  }

  onEdit(callback: (profile: Profile) => void): void {
    this.onEditCallback = callback;
  }

  onDelete(callback: (profileId: string) => Promise<void>): void {
    this.onDeleteCallback = callback;
  }

  getSelectedProfile(): Profile | null {
    return this.profiles.find((p) => p.id === this.selectedId) ?? null;
  }

  private render(): void {
    const hasProfiles = this.profiles.length > 0;
    const hasSelection = this.selectedId !== null && hasProfiles;

    const options = hasProfiles
      ? this.profiles
          .map(
            (p) =>
              `<option value="${escapeHtml(p.id)}" ${p.id === this.selectedId ? "selected" : ""}>${escapeHtml(p.name)}</option>`
          )
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
      this.selectedId = (e.target as HTMLSelectElement).value || null;
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
