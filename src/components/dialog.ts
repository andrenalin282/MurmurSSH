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
 * Show the change-permissions (chmod) dialog for a single entry.
 * Presents an rwx checkbox grid (owner/group/other) kept in sync with an
 * octal text field. Returns the selected mode (integer, 0..=0o777) or null
 * if cancelled.
 *
 * @param name      Display name of the target entry (for the title).
 * @param initial   Current mode bits (only the low 9 bits are used); defaults to 0o644.
 */
export function showPermissionsDialog(
  name: string,
  initial: number | null
): Promise<number | null> {
  return new Promise((resolve) => {
    const start = ((initial ?? 0o644) & 0o777);

    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    const classes: Array<["owner" | "group" | "other", number]> = [
      ["owner", 6],
      ["group", 3],
      ["other", 0],
    ];
    const perms: Array<["r" | "w" | "x", number]> = [
      ["r", 4],
      ["w", 2],
      ["x", 1],
    ];

    const rowsHtml = classes
      .map(
        ([cls]) => `
        <tr>
          <td class="perm-grid__label">${t(`dialogs.perm_${cls}`)}</td>
          ${perms
            .map(
              ([p]) =>
                `<td><input type="checkbox" class="perm-cb" data-cls="${cls}" data-perm="${p}"></td>`
            )
            .join("")}
        </tr>`
      )
      .join("");

    overlay.innerHTML = `
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal__title">${t("dialogs.permTitle")}: ${escHtml(name)}</div>
        <table class="perm-grid">
          <thead>
            <tr>
              <th></th>
              <th>${t("dialogs.perm_read")}</th>
              <th>${t("dialogs.perm_write")}</th>
              <th>${t("dialogs.perm_execute")}</th>
            </tr>
          </thead>
          <tbody>${rowsHtml}</tbody>
        </table>
        <div class="form-field perm-grid__octal">
          <label>${t("dialogs.permOctal")}
            <input id="perm-octal" type="text" inputmode="numeric" maxlength="4" autocomplete="off" value="${start
              .toString(8)
              .padStart(3, "0")}">
          </label>
        </div>
        <div class="modal__actions">
          <button class="btn-secondary" id="perm-cancel">${t("dialogs.promptCancel")}</button>
          <button id="perm-apply">${t("dialogs.permApply")}</button>
        </div>
      </div>
    `;

    document.body.appendChild(overlay);

    const octalInput = overlay.querySelector<HTMLInputElement>("#perm-octal")!;
    const cbs = Array.from(overlay.querySelectorAll<HTMLInputElement>(".perm-cb"));

    const clsShift: Record<string, number> = { owner: 6, group: 3, other: 0 };
    const permBit: Record<string, number> = { r: 4, w: 2, x: 1 };

    const modeToGrid = (mode: number) => {
      for (const cb of cbs) {
        const shift = clsShift[cb.dataset.cls!];
        const bit = permBit[cb.dataset.perm!];
        cb.checked = (((mode >> shift) & 7) & bit) !== 0;
      }
    };

    const gridToMode = (): number => {
      let mode = 0;
      for (const cb of cbs) {
        if (cb.checked) {
          mode |= permBit[cb.dataset.perm!] << clsShift[cb.dataset.cls!];
        }
      }
      return mode;
    };

    modeToGrid(start);

    for (const cb of cbs) {
      cb.addEventListener("change", () => {
        octalInput.value = gridToMode().toString(8).padStart(3, "0");
      });
    }

    octalInput.addEventListener("input", () => {
      const raw = octalInput.value.trim();
      if (!/^[0-7]{1,4}$/.test(raw)) return;
      const parsed = parseInt(raw, 8) & 0o777;
      modeToGrid(parsed);
    });

    const cleanup = (result: number | null) => {
      overlay.remove();
      resolve(result);
    };

    overlay.querySelector("#perm-cancel")?.addEventListener("click", () => cleanup(null));
    overlay.querySelector("#perm-apply")?.addEventListener("click", () => {
      cleanup(gridToMode());
    });

    setTimeout(() => overlay.querySelector<HTMLButtonElement>("#perm-apply")?.focus(), 10);
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
