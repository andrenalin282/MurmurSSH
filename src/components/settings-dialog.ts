import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api/index";
import type { Settings } from "../types";

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export class SettingsDialog {
  /**
   * Called after settings are saved.
   * Receives the newly saved Settings so callers can react immediately
   * (e.g. apply theme without a page reload).
   */
  private onAppliedCallback: ((settings: Settings) => Promise<void>) | null = null;

  onApplied(cb: (settings: Settings) => Promise<void>): void {
    this.onAppliedCallback = cb;
  }

  async show(): Promise<void> {
    const [settings, activePath] = await Promise.all([
      api.getSettings(),
      api.getProfilesPath(),
    ]);

    const isCustom = !!(settings.profiles_path && settings.profiles_path.trim());
    const customPath = settings.profiles_path ?? "";
    const currentTheme = settings.theme ?? "system";

    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    overlay.innerHTML = `
      <div class="modal modal--form" role="dialog" aria-modal="true">
        <div class="modal__title">Settings</div>

        <div class="form-field">
          <label>Profile Storage Location</label>
          <div class="settings-path-display">${escHtml(activePath)}</div>
        </div>

        <div class="save-mode-options">
          <div class="save-mode-option">
            <label>
              <input type="radio" name="path-mode" value="default" ${isCustom ? "" : "checked"}>
              <span>Default location <span class="save-mode-tag save-mode-tag--safe">~/.config/murmurssh/profiles/</span></span>
            </label>
          </div>
          <div class="save-mode-option">
            <label>
              <input type="radio" name="path-mode" value="custom" ${isCustom ? "checked" : ""}>
              <span>Custom directory</span>
            </label>
          </div>
        </div>

        <div id="custom-path-row" class="form-field" style="${isCustom ? "" : "display:none"}">
          <label for="custom-path-input">Custom path</label>
          <div class="form-field__row">
            <input id="custom-path-input" type="text" value="${escHtml(customPath)}" placeholder="/path/to/profiles">
            <button type="button" id="browse-dir-btn">Browse…</button>
          </div>
        </div>

        <div class="form-field">
          <label>Theme</label>
          <div class="save-mode-options" style="margin-bottom:0">
            <div class="save-mode-option">
              <label>
                <input type="radio" name="theme" value="system" ${currentTheme === "system" ? "checked" : ""}>
                <span>System <span class="save-mode-tag">follow OS preference</span></span>
              </label>
            </div>
            <div class="save-mode-option">
              <label>
                <input type="radio" name="theme" value="dark" ${currentTheme === "dark" ? "checked" : ""}>
                <span>Dark</span>
              </label>
            </div>
            <div class="save-mode-option">
              <label>
                <input type="radio" name="theme" value="light" ${currentTheme === "light" ? "checked" : ""}>
                <span>Light</span>
              </label>
            </div>
          </div>
        </div>

        <div class="modal__actions">
          <button type="button" class="btn-secondary" id="settings-cancel">Cancel</button>
          <button type="button" id="settings-apply">Apply</button>
        </div>
      </div>
    `;

    document.body.appendChild(overlay);

    const pathInput = overlay.querySelector<HTMLInputElement>("#custom-path-input")!;
    const customRow = overlay.querySelector<HTMLElement>("#custom-path-row")!;

    // Show/hide custom path row based on radio
    overlay.querySelectorAll<HTMLInputElement>('input[name="path-mode"]').forEach((radio) => {
      radio.addEventListener("change", () => {
        customRow.style.display = radio.value === "custom" && radio.checked ? "" : "none";
      });
    });

    // Browse for directory
    overlay.querySelector("#browse-dir-btn")?.addEventListener("click", async () => {
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === "string") {
        pathInput.value = selected;
      }
    });

    overlay.querySelector("#settings-cancel")?.addEventListener("click", () => {
      overlay.remove();
    });

    overlay.querySelector("#settings-apply")?.addEventListener("click", async () => {
      const mode = overlay.querySelector<HTMLInputElement>('input[name="path-mode"]:checked')?.value;
      const newPath = mode === "custom" ? pathInput.value.trim() : null;

      // Validate custom path is not empty when custom mode is selected
      if (mode === "custom" && !newPath) {
        pathInput.focus();
        return;
      }

      const newTheme = (
        overlay.querySelector<HTMLInputElement>('input[name="theme"]:checked')?.value ?? "system"
      ) as Settings["theme"];

      const updated: Settings = {
        ...settings,
        profiles_path: newPath ?? null,
        theme: newTheme,
      };

      try {
        await api.saveSettings(updated);
        overlay.remove();
        await this.onAppliedCallback?.(updated);
      } catch (err) {
        // Show inline error
        const actions = overlay.querySelector(".modal__actions");
        if (actions) {
          let errEl = overlay.querySelector<HTMLElement>(".settings-error");
          if (!errEl) {
            errEl = document.createElement("div");
            errEl.className = "form-error settings-error";
            actions.before(errEl);
          }
          errEl.textContent = `Failed to save settings: ${err}`;
        }
      }
    });
  }
}
