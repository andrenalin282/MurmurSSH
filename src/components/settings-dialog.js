import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api/index";
import { t, getLocale, setLocale, getAvailableLocales } from "../i18n/index";
function escHtml(s) {
    return s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
export class SettingsDialog {
    constructor() {
        /**
         * Called after settings are saved.
         * Receives the newly saved Settings so callers can react immediately
         * (e.g. apply theme without a page reload).
         */
        this.onAppliedCallback = null;
    }
    onApplied(cb) {
        this.onAppliedCallback = cb;
    }
    async show() {
        const [settings, activePath] = await Promise.all([
            api.getSettings(),
            api.getProfilesPath(),
        ]);
        const isCustom = !!(settings.profiles_path && settings.profiles_path.trim());
        const customPath = settings.profiles_path ?? "";
        const currentTheme = settings.theme ?? "system";
        const currentPosition = settings.local_browser_position ?? "left";
        const currentLocale = getLocale();
        const availableLocales = getAvailableLocales();
        const localeOptions = availableLocales
            .map((l) => `<option value="${l.key}" ${l.key === currentLocale ? "selected" : ""}>${l.label}</option>`)
            .join("");
        const overlay = document.createElement("div");
        overlay.className = "modal-overlay";
        overlay.innerHTML = `
      <div class="modal modal--form" role="dialog" aria-modal="true">
        <div class="modal__title">${t("settings.title")}</div>

        <div class="form-field">
          <label>${t("settings.labelStoragePath")}</label>
          <div class="settings-path-display">${escHtml(activePath)}</div>
        </div>

        <div class="save-mode-options">
          <div class="save-mode-option">
            <label>
              <input type="radio" name="path-mode" value="default" ${isCustom ? "" : "checked"}>
              <span>${t("settings.optionDefault")} <span class="save-mode-tag save-mode-tag--safe">${t("settings.defaultPathTag")}</span></span>
            </label>
          </div>
          <div class="save-mode-option">
            <label>
              <input type="radio" name="path-mode" value="custom" ${isCustom ? "checked" : ""}>
              <span>${t("settings.optionCustom")}</span>
            </label>
          </div>
        </div>

        <div id="custom-path-row" class="form-field" style="${isCustom ? "" : "display:none"}">
          <label for="custom-path-input">${t("settings.labelCustomPath")}</label>
          <div class="form-field__row">
            <input id="custom-path-input" type="text" value="${escHtml(customPath)}" placeholder="${t("settings.placeholderCustomPath")}">
            <button type="button" id="browse-dir-btn">${t("common.browse")}</button>
          </div>
        </div>

        <div class="form-field">
          <label>${t("settings.labelTheme")}</label>
          <div class="save-mode-options" style="margin-bottom:0">
            <div class="save-mode-option">
              <label>
                <input type="radio" name="theme" value="system" ${currentTheme === "system" ? "checked" : ""}>
                <span>${t("settings.themeSystem")} <span class="save-mode-tag">${t("settings.themeSystemTag")}</span></span>
              </label>
            </div>
            <div class="save-mode-option">
              <label>
                <input type="radio" name="theme" value="dark" ${currentTheme === "dark" ? "checked" : ""}>
                <span>${t("settings.themeDark")}</span>
              </label>
            </div>
            <div class="save-mode-option">
              <label>
                <input type="radio" name="theme" value="light" ${currentTheme === "light" ? "checked" : ""}>
                <span>${t("settings.themeLight")}</span>
              </label>
            </div>
          </div>
        </div>

        <div class="form-field">
          <label>${t("settings.labelLocalBrowserPosition")}</label>
          <div class="save-mode-options" style="margin-bottom:0">
            <div class="save-mode-option">
              <label>
                <input type="radio" name="local-browser-position" value="left" ${currentPosition === "left" ? "checked" : ""}>
                <span>${t("settings.localBrowserLeft")}</span>
              </label>
            </div>
            <div class="save-mode-option">
              <label>
                <input type="radio" name="local-browser-position" value="right" ${currentPosition === "right" ? "checked" : ""}>
                <span>${t("settings.localBrowserRight")}</span>
              </label>
            </div>
          </div>
        </div>

        <div class="form-field">
          <label for="lang-select-settings">${t("settings.labelLanguage")}</label>
          <select id="lang-select-settings" style="width:100%">
            ${localeOptions}
          </select>
        </div>

        <div class="modal__actions">
          <button type="button" class="btn-secondary" id="settings-cancel">${t("common.cancel")}</button>
          <button type="button" id="settings-apply">${t("common.apply")}</button>
        </div>
      </div>
    `;
        document.body.appendChild(overlay);
        const pathInput = overlay.querySelector("#custom-path-input");
        const customRow = overlay.querySelector("#custom-path-row");
        // Show/hide custom path row based on radio
        overlay.querySelectorAll('input[name="path-mode"]').forEach((radio) => {
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
            const mode = overlay.querySelector('input[name="path-mode"]:checked')?.value;
            const newPath = mode === "custom" ? pathInput.value.trim() : null;
            // Validate custom path is not empty when custom mode is selected
            if (mode === "custom" && !newPath) {
                pathInput.focus();
                return;
            }
            const newTheme = (overlay.querySelector('input[name="theme"]:checked')?.value ?? "system");
            const newPosition = (overlay.querySelector('input[name="local-browser-position"]:checked')?.value ?? "left");
            const newLang = overlay.querySelector("#lang-select-settings")?.value ?? "en";
            const langChanged = newLang !== currentLocale;
            const updated = {
                ...settings,
                profiles_path: newPath ?? null,
                theme: newTheme,
                local_browser_position: newPosition,
            };
            try {
                await api.saveSettings(updated);
                if (langChanged) {
                    setLocale(newLang);
                    window.location.reload();
                    return;
                }
                overlay.remove();
                await this.onAppliedCallback?.(updated);
            }
            catch (err) {
                // Show inline error
                const actions = overlay.querySelector(".modal__actions");
                if (actions) {
                    let errEl = overlay.querySelector(".settings-error");
                    if (!errEl) {
                        errEl = document.createElement("div");
                        errEl.className = "form-error settings-error";
                        actions.before(errEl);
                    }
                    errEl.textContent = t("settings.errorSaveFailed", { error: String(err) });
                }
            }
        });
    }
}
