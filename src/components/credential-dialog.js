import { t } from "../i18n/index";
function escHtml(s) {
    return s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
/**
 * Show a password prompt with a save-mode choice.
 *
 * Used for password authentication only. Returns the entered password and
 * how the user wants it stored, or null if cancelled.
 * The caller is responsible for calling saveCredential() when saveMode != "never".
 */
export function showPasswordPrompt(username, host) {
    return new Promise((resolve) => {
        const overlay = document.createElement("div");
        overlay.className = "modal-overlay";
        overlay.innerHTML = `
      <div class="modal modal--form" role="dialog" aria-modal="true">
        <div class="modal__title">${t("credentials.passwordTitle")}</div>
        <div class="modal__body">${t("credentials.passwordBody", { username: escHtml(username), host: escHtml(host) })}</div>
        <form id="cred-form">
          <div class="form-field">
            <label for="cred-input">${t("credentials.passwordLabel")}</label>
            <input id="cred-input" type="password" autocomplete="off" autofocus>
          </div>
          <div class="save-mode-options">
            <div class="save-mode-option">
              <label>
                <input type="radio" name="save-mode" value="never" checked>
                <span>${t("credentials.saveModeNever")} <span class="save-mode-tag save-mode-tag--safe">${t("credentials.saveModeNeverTag")}</span></span>
              </label>
            </div>
            <div class="save-mode-option">
              <label>
                <input type="radio" name="save-mode" value="local_machine">
                <span>${t("credentials.saveModeLocal")} <span class="save-mode-tag">${t("credentials.saveModeLocalTag")}</span></span>
              </label>
            </div>
            <div class="save-mode-option">
              <label>
                <input type="radio" name="save-mode" value="portable_profile">
                <span>${t("credentials.saveModePortable")} <span class="save-mode-tag save-mode-tag--warn">${t("credentials.saveModePortableTag")}</span></span>
              </label>
            </div>
            <div class="save-mode-warning" id="save-mode-warning" style="display:none">
              ${t("credentials.portableWarning")}
            </div>
          </div>
          <div class="modal__actions">
            <button type="button" class="btn-secondary" id="cred-cancel">${t("common.cancel")}</button>
            <button type="submit">${t("credentials.connect")}</button>
          </div>
        </form>
      </div>
    `;
        document.body.appendChild(overlay);
        const cleanup = (result) => {
            overlay.remove();
            resolve(result);
        };
        setTimeout(() => {
            overlay.querySelector("#cred-input")?.focus();
        }, 0);
        overlay.querySelectorAll('input[name="save-mode"]').forEach((radio) => {
            radio.addEventListener("change", () => {
                const warning = overlay.querySelector("#save-mode-warning");
                if (warning) {
                    warning.style.display = radio.value === "portable_profile" && radio.checked ? "" : "none";
                }
            });
        });
        overlay.querySelector("#cred-cancel")?.addEventListener("click", () => cleanup(null));
        overlay.querySelector("#cred-form")?.addEventListener("submit", (e) => {
            e.preventDefault();
            const secret = overlay.querySelector("#cred-input")?.value ?? "";
            if (!secret)
                return;
            const saveMode = (overlay.querySelector('input[name="save-mode"]:checked')?.value ?? "never");
            cleanup({ secret, saveMode });
        });
    });
}
/**
 * Prompt the user for an SSH key passphrase (masked input).
 *
 * Returns the entered passphrase string, or null if cancelled.
 *
 * IMPORTANT: Passphrases are runtime-only. There are no save options here.
 * The passphrase is used only for the current connection and is discarded immediately.
 * It is never written to disk, the profile JSON, or any persistent storage.
 */
export function showPassphrasePrompt(keyPath) {
    return new Promise((resolve) => {
        const overlay = document.createElement("div");
        overlay.className = "modal-overlay";
        overlay.innerHTML = `
      <div class="modal modal--form" role="dialog" aria-modal="true">
        <div class="modal__title">${t("credentials.passphraseTitle")}</div>
        <div class="modal__body">${t("credentials.passphraseBody", { keyPath: escHtml(keyPath) })}</div>
        <form id="pp-form">
          <div class="form-field">
            <label for="pp-input">${t("credentials.passphraseLabel")}</label>
            <input id="pp-input" type="password" autocomplete="off" autofocus>
          </div>
          <div class="modal__body" style="font-size:12px; color:var(--fg-subtle); margin-top:0; margin-bottom:8px;">
            ${t("credentials.passphraseNote")}
          </div>
          <div class="modal__actions">
            <button type="button" class="btn-secondary" id="pp-cancel">${t("common.cancel")}</button>
            <button type="submit">${t("credentials.unlock")}</button>
          </div>
        </form>
      </div>
    `;
        document.body.appendChild(overlay);
        const cleanup = (result) => {
            overlay.remove();
            resolve(result);
        };
        setTimeout(() => {
            overlay.querySelector("#pp-input")?.focus();
        }, 0);
        overlay.querySelector("#pp-cancel")?.addEventListener("click", () => cleanup(null));
        overlay.querySelector("#pp-form")?.addEventListener("submit", (e) => {
            e.preventDefault();
            const val = overlay.querySelector("#pp-input")?.value ?? "";
            cleanup(val || null);
        });
    });
}
/**
 * Show a host key verification dialog with three explicit options.
 *
 * "Accept once" trusts the key for this session in memory only — the user will
 * be prompted again next time the app starts.
 *
 * "Accept and save" writes the key to known_hosts and the host is trusted from
 * now on without prompting.
 */
export function showHostKeyDialog(host, fingerprint) {
    return new Promise((resolve) => {
        const overlay = document.createElement("div");
        overlay.className = "modal-overlay";
        overlay.innerHTML = `
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal__title">${t("credentials.hostKeyTitle")}</div>
        <div class="modal__body">
          <p>${t("credentials.hostKeyBody1", { host: escHtml(host) })}</p>
          <p>${t("credentials.hostKeyBody2")}</p>
          <code class="host-fingerprint">${escHtml(fingerprint)}</code>
          <p>${t("credentials.hostKeyBody3")}</p>
        </div>
        <div class="modal__actions modal__actions--hostkey">
          <button class="btn-secondary" id="hk-cancel">${t("credentials.hostKeyCancel")}</button>
          <button class="btn-secondary" id="hk-once">${t("credentials.hostKeyOnce")}</button>
          <button id="hk-save">${t("credentials.hostKeySave")}</button>
        </div>
      </div>
    `;
        document.body.appendChild(overlay);
        const cleanup = (result) => {
            overlay.remove();
            resolve(result);
        };
        overlay.querySelector("#hk-cancel")?.addEventListener("click", () => cleanup("cancel"));
        overlay.querySelector("#hk-once")?.addEventListener("click", () => cleanup("accept_once"));
        overlay.querySelector("#hk-save")?.addEventListener("click", () => cleanup("accept_save"));
    });
}
