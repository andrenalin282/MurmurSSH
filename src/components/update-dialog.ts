import * as api from "../api/index";
import type { UpdateCheckResult } from "../types";
import { t } from "../i18n/index";

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Modal shown when a newer GitHub Release is available. */
export function showUpdateAvailableDialog(result: UpdateCheckResult): void {
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
        <button type="button" class="btn-secondary" id="update-later" data-modal-cancel>${t("dialogs.updateLater")}</button>
        <button type="button" id="update-open" data-modal-primary>${t("dialogs.updateOpenReleases")}</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  setTimeout(() => overlay.querySelector<HTMLButtonElement>("#update-open")?.focus(), 10);
  const close = () => overlay.remove();
  overlay.querySelector("#update-later")?.addEventListener("click", close);
  overlay.querySelector("#update-open")?.addEventListener("click", () => {
    api.openUrl(result.release_url).catch(() => {});
    close();
  });
}
