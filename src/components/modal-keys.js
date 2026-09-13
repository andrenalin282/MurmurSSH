/**
 * Central keyboard handling for in-app modals.
 *
 * The topmost `.modal-overlay` (last in DOM order) receives:
 *  - Escape → click its `[data-modal-cancel]` button
 *  - Enter  → click its `[data-modal-primary]` button, unless focus is in a textarea/select
 *             or on a button (a focused button keeps native Enter behaviour)
 * Handled events are stopped so file-browser shortcuts never see them.
 */
let installed = false;
export function installModalKeyHandler() {
    if (installed)
        return;
    installed = true;
    document.addEventListener("keydown", (e) => {
        if (e.key !== "Escape" && e.key !== "Enter")
            return;
        const overlays = document.querySelectorAll(".modal-overlay");
        if (overlays.length === 0)
            return;
        const top = overlays[overlays.length - 1];
        if (e.key === "Escape") {
            const cancel = top.querySelector("[data-modal-cancel]");
            if (cancel && !cancel.disabled) {
                e.preventDefault();
                e.stopPropagation();
                cancel.click();
            }
            return;
        }
        // Enter
        const active = document.activeElement;
        const tag = active?.tagName?.toLowerCase();
        if (tag === "textarea" || tag === "select" || tag === "button")
            return;
        if (e.isComposing)
            return;
        const primary = top.querySelector("[data-modal-primary]");
        if (primary && !primary.disabled) {
            e.preventDefault();
            e.stopPropagation();
            primary.click();
        }
    }, true);
}
