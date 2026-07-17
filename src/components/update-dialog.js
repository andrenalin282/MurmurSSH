import * as api from "../api/index";
import { t } from "../i18n/index";
function escHtml(s) {
    return s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
/** Modal shown when a newer GitHub Release is available. */
export function showUpdateAvailableDialog(result) {
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    overlay.innerHTML = `
    <div class="modal" role="dialog" aria-modal="true">
      <div class="modal__title">${t("dialogs.updateAvailableTitle")}</div>
      <div class="modal__body">${t("dialogs.updateAvailableBody", {
        latest: escHtml(result.latest_version),
        current: escHtml(result.current_version),
    })}</div>
      <div class="modal__actions">
        <button type="button" class="btn-secondary" id="update-later">${t("dialogs.updateLater")}</button>
        <button type="button" id="update-open">${t("dialogs.updateOpenReleases")}</button>
      </div>
    </div>`;
    document.body.appendChild(overlay);
    const close = () => overlay.remove();
    overlay.querySelector("#update-later")?.addEventListener("click", close);
    overlay.querySelector("#update-open")?.addEventListener("click", () => {
        api.openUrl(result.release_url).catch(() => { });
        close();
    });
}
