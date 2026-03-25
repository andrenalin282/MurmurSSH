import { t } from "../i18n/index";

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * Show a simple in-app prompt dialog with a text input.
 * Returns the entered string (trimmed) or null if cancelled / empty.
 * Pass `initialValue` to pre-fill the input (e.g. for rename dialogs).
 */
export function showPrompt(title: string, placeholder = "", initialValue = ""): Promise<string | null> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    overlay.innerHTML = `
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal__title">${escHtml(title)}</div>
        <div class="form-field" style="margin-bottom:0">
          <input id="modal-prompt-input" type="text" placeholder="${escHtml(placeholder)}" value="${escHtml(initialValue)}" autocomplete="off">
        </div>
        <div class="modal__actions">
          <button class="btn-secondary" id="modal-cancel">${t("dialogs.promptCancel")}</button>
          <button id="modal-confirm">${t("dialogs.promptOk")}</button>
        </div>
      </div>
    `;

    document.body.appendChild(overlay);

    const input = overlay.querySelector<HTMLInputElement>("#modal-prompt-input")!;
    // Focus the input after a short delay so the modal is rendered; select all if pre-filled
    setTimeout(() => { input.focus(); if (initialValue) input.select(); }, 10);

    const cleanup = (result: string | null) => {
      overlay.remove();
      resolve(result);
    };

    const submit = () => {
      const val = input.value.trim();
      cleanup(val || null);
    };

    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") submit();
      if (e.key === "Escape") cleanup(null);
    });

    overlay.querySelector("#modal-cancel")?.addEventListener("click", () => cleanup(null));
    overlay.querySelector("#modal-confirm")?.addEventListener("click", submit);
  });
}

export type OverwriteAction = "yes" | "no" | "cancel";

/**
 * Show the upload-overwrite dialog for a single file conflict.
 *
 * Returns the user's chosen action and whether "Apply to all" was checked:
 * - action "yes"    → overwrite this (and optionally all) files
 * - action "no"     → skip this (and optionally all) files
 * - action "cancel" → abort the entire upload batch
 * - applyToAll      → caller should remember `action` for remaining conflicts
 */
export function showOverwriteDialog(
  filename: string
): Promise<{ action: OverwriteAction; applyToAll: boolean }> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    overlay.innerHTML = `
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal__title">${t("dialogs.overwriteTitle")}</div>
        <div class="modal__body">
          <strong>${escHtml(filename)}</strong> ${t("dialogs.overwriteBodySuffix")}<br>${t("dialogs.overwriteQuestion")}
        </div>
        <div class="modal__check">
          <label>
            <input type="checkbox" id="overwrite-apply-all"> ${t("dialogs.overwriteApplyToAll")}
          </label>
        </div>
        <div class="modal__actions">
          <button class="btn-secondary" id="overwrite-cancel">${t("dialogs.overwriteCancel")}</button>
          <button class="btn-secondary" id="overwrite-no">${t("dialogs.overwriteNo")}</button>
          <button id="overwrite-yes">${t("dialogs.overwriteYes")}</button>
        </div>
      </div>
    `;
    document.body.appendChild(overlay);

    const checkbox = overlay.querySelector<HTMLInputElement>("#overwrite-apply-all")!;
    const cleanup = (action: OverwriteAction) => {
      overlay.remove();
      resolve({ action, applyToAll: checkbox.checked });
    };

    overlay.querySelector("#overwrite-cancel")?.addEventListener("click", () => cleanup("cancel"));
    overlay.querySelector("#overwrite-no")?.addEventListener("click", () => cleanup("no"));
    overlay.querySelector("#overwrite-yes")?.addEventListener("click", () => cleanup("yes"));

    // Default focus on Yes so Enter confirms
    setTimeout(() => overlay.querySelector<HTMLButtonElement>("#overwrite-yes")?.focus(), 10);
  });
}

/**
 * Show a simple in-app confirmation dialog.
 * Returns a promise that resolves to true (confirmed) or false (cancelled).
 */
export function showConfirm(message: string, title?: string): Promise<boolean> {
  const resolvedTitle = title ?? t("dialogs.confirmConfirm");
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    overlay.innerHTML = `
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal__title">${escHtml(resolvedTitle)}</div>
        <div class="modal__body">${escHtml(message)}</div>
        <div class="modal__actions">
          <button class="btn-secondary" id="modal-cancel">${t("dialogs.confirmCancel")}</button>
          <button id="modal-confirm">${t("dialogs.confirmConfirm")}</button>
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
