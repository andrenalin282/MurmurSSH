let active = "remote";
const listeners = [];
export function getActivePanel() {
    return active;
}
export function setActivePanel(p) {
    if (active === p)
        return;
    active = p;
    document.getElementById("file-browser")?.classList.toggle("panel--active", p === "remote");
    document.getElementById("local-file-browser")?.classList.toggle("panel--active", p === "local");
    listeners.forEach((cb) => cb(p));
}
export function onActivePanelChange(cb) {
    listeners.push(cb);
}
