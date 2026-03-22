function escHtml(s) {
    return s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
/**
 * Show a simple in-app prompt dialog with a text input.
 * Returns the entered string (trimmed) or null if cancelled / empty.
 */
export function showPrompt(title, placeholder = "") {
    return new Promise((resolve) => {
        const overlay = document.createElement("div");
        overlay.className = "modal-overlay";
        overlay.innerHTML = `
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal__title">${escHtml(title)}</div>
        <div class="form-field" style="margin-bottom:0">
          <input id="modal-prompt-input" type="text" placeholder="${escHtml(placeholder)}" autocomplete="off">
        </div>
        <div class="modal__actions">
          <button class="btn-secondary" id="modal-cancel">Cancel</button>
          <button id="modal-confirm">OK</button>
        </div>
      </div>
    `;
        document.body.appendChild(overlay);
        const input = overlay.querySelector("#modal-prompt-input");
        // Focus the input after a short delay so the modal is rendered
        setTimeout(() => input.focus(), 10);
        const cleanup = (result) => {
            overlay.remove();
            resolve(result);
        };
        const submit = () => {
            const val = input.value.trim();
            cleanup(val || null);
        };
        input.addEventListener("keydown", (e) => {
            if (e.key === "Enter")
                submit();
            if (e.key === "Escape")
                cleanup(null);
        });
        overlay.querySelector("#modal-cancel")?.addEventListener("click", () => cleanup(null));
        overlay.querySelector("#modal-confirm")?.addEventListener("click", submit);
    });
}
/**
 * Show a simple in-app confirmation dialog.
 * Returns a promise that resolves to true (confirmed) or false (cancelled).
 */
export function showConfirm(message, title = "Confirm") {
    return new Promise((resolve) => {
        const overlay = document.createElement("div");
        overlay.className = "modal-overlay";
        overlay.innerHTML = `
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal__title">${escHtml(title)}</div>
        <div class="modal__body">${escHtml(message)}</div>
        <div class="modal__actions">
          <button class="btn-secondary" id="modal-cancel">Cancel</button>
          <button id="modal-confirm">Confirm</button>
        </div>
      </div>
    `;
        document.body.appendChild(overlay);
        const cleanup = (result) => {
            overlay.remove();
            resolve(result);
        };
        overlay.querySelector("#modal-cancel")?.addEventListener("click", () => cleanup(false));
        overlay.querySelector("#modal-confirm")?.addEventListener("click", () => cleanup(true));
    });
}
