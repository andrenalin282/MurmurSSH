import type { ConnectionStatus } from "../types";

const STATUS_LABELS: Record<ConnectionStatus, string> = {
  disconnected: "Not connected",
  connecting: "Connecting…",
  connected: "Connected",
  error: "Error",
};

export class StatusBar {
  private container: HTMLElement;

  constructor(containerId: string) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;
    this.set("disconnected");
  }

  set(status: ConnectionStatus, message?: string): void {
    const label = message ?? STATUS_LABELS[status];
    this.container.innerHTML = `
      <div class="status-bar status-${status}">
        <span class="status-indicator"></span>
        <span class="status-label">${label}</span>
      </div>
    `;
  }
}
