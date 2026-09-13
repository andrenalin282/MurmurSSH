import { t } from "./i18n/index";

export type ShortcutScope = "global" | "panels" | "remote" | "local" | "dialogs";

export interface Shortcut {
  /** i18n key suffix: description is t(`shortcuts.${id}`) */
  id: string;
  /** Alternatives, e.g. ["Backspace", "Alt+ArrowUp"]. Modifiers: Ctrl, Shift, Alt. */
  keys: string[];
  scope: ShortcutScope;
  /** Listed in help but matched elsewhere (type-ahead, dialog Enter/Esc). */
  displayOnly?: boolean;
}

export const SHORTCUTS: Shortcut[] = [
  { id: "help", keys: ["F1", "?"], scope: "global" },
  { id: "switchPanel", keys: ["Tab"], scope: "global" },

  { id: "cursorUp", keys: ["ArrowUp"], scope: "panels" },
  { id: "cursorDown", keys: ["ArrowDown"], scope: "panels" },
  { id: "extendUp", keys: ["Shift+ArrowUp"], scope: "panels" },
  { id: "extendDown", keys: ["Shift+ArrowDown"], scope: "panels" },
  { id: "first", keys: ["Home"], scope: "panels" },
  { id: "last", keys: ["End"], scope: "panels" },
  { id: "pageUp", keys: ["PageUp"], scope: "panels" },
  { id: "pageDown", keys: ["PageDown"], scope: "panels" },
  { id: "open", keys: ["Enter"], scope: "panels" },
  { id: "parent", keys: ["Backspace", "Alt+ArrowUp"], scope: "panels" },
  { id: "focusPath", keys: ["Ctrl+L"], scope: "panels" },
  { id: "refresh", keys: ["F5"], scope: "panels" },
  { id: "rename", keys: ["F2"], scope: "panels" },
  { id: "newFolder", keys: ["F7", "Ctrl+Shift+N"], scope: "panels" },
  { id: "delete", keys: ["Delete"], scope: "panels" },
  { id: "selectAll", keys: ["Ctrl+A"], scope: "panels" },
  { id: "clearSelection", keys: ["Escape"], scope: "panels" },
  { id: "typeAhead", keys: ["a–z, 0–9"], scope: "panels", displayOnly: true },
  { id: "pathEnter", keys: ["Enter"], scope: "panels", displayOnly: true },
  { id: "pathEscape", keys: ["Escape"], scope: "panels", displayOnly: true },

  { id: "newFile", keys: ["Ctrl+N"], scope: "remote" },
  { id: "moveTo", keys: ["F6"], scope: "remote" },
  { id: "copyTo", keys: ["Ctrl+Shift+C"], scope: "remote" },
  { id: "clipCopy", keys: ["Ctrl+C"], scope: "remote" },
  { id: "clipCut", keys: ["Ctrl+X"], scope: "remote" },
  { id: "clipPaste", keys: ["Ctrl+V"], scope: "remote" },
  { id: "download", keys: ["Ctrl+D"], scope: "remote" },
  { id: "terminal", keys: ["F11"], scope: "remote" },

  { id: "upload", keys: ["Ctrl+U"], scope: "local" },

  { id: "dialogConfirm", keys: ["Enter"], scope: "dialogs", displayOnly: true },
  { id: "dialogCancel", keys: ["Escape"], scope: "dialogs", displayOnly: true },
];

function parse(spec: string): { key: string; ctrl: boolean; shift: boolean; alt: boolean } {
  const parts = spec.split("+");
  const key = parts.pop()!;
  return { key, ctrl: parts.includes("Ctrl"), shift: parts.includes("Shift"), alt: parts.includes("Alt") };
}

function keyMatches(e: KeyboardEvent, spec: string): boolean {
  const p = parse(spec);
  const isSymbol = p.key.length === 1 && !/[a-z0-9]/i.test(p.key);
  const evKey = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  const specKey = p.key.length === 1 ? p.key.toLowerCase() : p.key;
  if (evKey !== specKey) return false;
  if ((e.ctrlKey || e.metaKey) !== p.ctrl) return false;
  if (e.altKey !== p.alt) return false;
  // Symbols like "?" need Shift on most layouts — ignore Shift for them.
  if (!isSymbol && e.shiftKey !== p.shift) return false;
  return true;
}

/** Id of the first non-display-only shortcut in `scope` matching `e`, else null. */
export function matchShortcut(e: KeyboardEvent, scope: ShortcutScope | ShortcutScope[]): string | null {
  const scopes = Array.isArray(scope) ? scope : [scope];
  for (const s of SHORTCUTS) {
    if (s.displayOnly || !scopes.includes(s.scope)) continue;
    if (s.keys.some((k) => keyMatches(e, k))) return s.id;
  }
  return null;
}

function keyLabel(spec: string): string {
  return spec
    .split("+")
    .map((k) => (t(`shortcuts.key_${k}`) !== `shortcuts.key_${k}` ? t(`shortcuts.key_${k}`) : k))
    .map((k) => `<kbd>${k}</kbd>`)
    .join("+");
}

/** Help-dialog table generated from SHORTCUTS — the only place shortcuts are documented. */
export function shortcutHelpHtml(): string {
  const order: ShortcutScope[] = ["global", "panels", "remote", "local", "dialogs"];
  return order
    .map((scope) => {
      const rows = SHORTCUTS.filter((s) => s.scope === scope)
        .map((s) => `<tr><td class="help-keys">${s.keys.map(keyLabel).join(" / ")}</td><td>${t(`shortcuts.${s.id}`)}</td></tr>`)
        .join("");
      return `<p><strong>${t(`shortcuts.scope_${scope}`)}</strong></p><table class="help-shortcuts">${rows}</table>`;
    })
    .join("");
}
