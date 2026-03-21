function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * Show a simple in-app confirmation dialog.
 * Returns a promise that resolves to true (confirmed) or false (cancelled).
 */
export function showConfirm(message: string, title = "Confirm"): Promise<boolean> {
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

    const cleanup = (result: boolean) => {
      overlay.remove();
      resolve(result);
    };

    overlay.querySelector("#modal-cancel")?.addEventListener("click", () => cleanup(false));
    overlay.querySelector("#modal-confirm")?.addEventListener("click", () => cleanup(true));
  });
}
