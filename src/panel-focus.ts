export type Panel = "local" | "remote";

let active: Panel = "remote";
const listeners: Array<(p: Panel) => void> = [];

export function getActivePanel(): Panel {
  return active;
}

export function setActivePanel(p: Panel): void {
  if (active === p) return;
  active = p;
  document.getElementById("file-browser")?.classList.toggle("panel--active", p === "remote");
  document.getElementById("local-file-browser")?.classList.toggle("panel--active", p === "local");
  listeners.forEach((cb) => cb(p));
}

export function onActivePanelChange(cb: (p: Panel) => void): void {
  listeners.push(cb);
}
