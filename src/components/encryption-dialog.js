import * as api from "../api/index";
import { t } from "../i18n/index";
import { showConfirm } from "./dialog";
function esc(s) {
    return s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
/**
 * Manage the credential encryption key: create (and encrypt existing secrets),
 * export the value, import a value from another machine, or remove the key
 * (decrypting all secrets back to plaintext). The raw key value is only shown
 * on explicit create/export.
 */
export async function showEncryptionDialog() {
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    document.body.appendChild(overlay);
    let revealed = null;
    let message = "";
    let error = "";
    async function render() {
        const status = await api.encryptionStatus().catch(() => ({ enabled: false, key_path: "" }));
        overlay.innerHTML = `
      <div class="modal modal--form" role="dialog" aria-modal="true">
        <div class="modal__title">${t("encryption.title")}</div>

        <div class="form-field">
          <div class="save-mode-tag ${status.enabled ? "save-mode-tag--safe" : ""}">
            ${status.enabled ? t("encryption.statusEnabled") : t("encryption.statusDisabled")}
          </div>
          <div class="form-field__hint">${t("encryption.keyPathLabel")} ${esc(status.key_path)}</div>
        </div>

        ${error ? `<div class="form-error">${esc(error)}</div>` : ""}
        ${message ? `<div class="form-field__hint" style="color:var(--accent)">${esc(message)}</div>` : ""}

        ${revealed
            ? `<div class="form-field">
                 <label>${t("encryption.keyValueLabel")}</label>
                 <div class="form-field__row">
                   <input id="enc-key-value" type="text" readonly value="${esc(revealed)}" spellcheck="false">
                   <button type="button" id="enc-copy" class="btn-secondary">${t("encryption.copy")}</button>
                 </div>
                 <div class="form-field__hint">${t("encryption.createWarning")}</div>
               </div>`
            : ""}

        ${status.enabled
            ? `<div class="form-field form-field__row" style="gap:8px">
                 <button type="button" id="enc-export" class="btn-secondary">${t("encryption.export")}</button>
                 <button type="button" id="enc-remove" class="btn-secondary">${t("encryption.remove")}</button>
               </div>`
            : `<div class="form-field">
                 <button type="button" id="enc-create">${t("encryption.create")}</button>
               </div>`}

        ${status.enabled
            ? ""
            : `<div class="form-field">
                 <label for="enc-import-input">${t("encryption.import")}</label>
                 <div class="form-field__row">
                   <input id="enc-import-input" type="text" placeholder="${t("encryption.importPlaceholder")}" spellcheck="false">
                   <button type="button" id="enc-import" class="btn-secondary">${t("encryption.importBtn")}</button>
                 </div>
               </div>`}

        <div class="modal__actions">
          <button type="button" class="btn-secondary" id="enc-close" data-modal-cancel>${t("common.close")}</button>
        </div>
      </div>`;
        wire();
    }
    function wire() {
        overlay.querySelector("#enc-close")?.addEventListener("click", () => overlay.remove());
        overlay.querySelector("#enc-copy")?.addEventListener("click", () => {
            const input = overlay.querySelector("#enc-key-value");
            if (!input)
                return;
            input.select();
            try {
                void navigator.clipboard.writeText(input.value);
            }
            catch {
                /* clipboard unavailable — user can copy manually from the selected field */
            }
        });
        overlay.querySelector("#enc-create")?.addEventListener("click", async () => {
            error = "";
            message = "";
            try {
                revealed = await api.createEncryptionKey();
                message = t("encryption.created");
            }
            catch (e) {
                error = String(e);
            }
            await render();
        });
        overlay.querySelector("#enc-export")?.addEventListener("click", async () => {
            error = "";
            message = "";
            try {
                revealed = await api.exportEncryptionKey();
            }
            catch (e) {
                error = String(e);
            }
            await render();
        });
        overlay.querySelector("#enc-import")?.addEventListener("click", async () => {
            const value = overlay.querySelector("#enc-import-input")?.value.trim() ?? "";
            if (!value)
                return;
            error = "";
            message = "";
            try {
                await api.importEncryptionKey(value);
                revealed = null;
                message = t("encryption.imported");
            }
            catch (e) {
                error = String(e);
            }
            await render();
        });
        overlay.querySelector("#enc-remove")?.addEventListener("click", async () => {
            const ok = await showConfirm(t("encryption.removeConfirm"), t("encryption.title"));
            if (!ok)
                return;
            error = "";
            message = "";
            try {
                await api.removeEncryptionKey();
                revealed = null;
                message = t("encryption.removed");
            }
            catch (e) {
                error = String(e);
            }
            await render();
        });
    }
    await render();
}
