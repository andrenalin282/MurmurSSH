import type { ConnectionStatus } from "../types";
import { t } from "../i18n/index";

export class StatusBar {
  private container: HTMLElement;

  constructor(containerId: string) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;
    this.set("disconnected");
  }

  set(status: ConnectionStatus, message?: string): void {
    const statusLabels: Record<ConnectionStatus, string> = {
      disconnected: t("connection.notConnected"),
      connecting: t("connection.connecting"),
      connected: t("connection.connected"),
      error: t("connection.error"),
    };
    const label = message ?? statusLabels[status];
    this.container.innerHTML = `
      <div class="status-bar status-${status}">
        <span class="status-indicator"></span>
        <span class="status-label">${label}</span>
      </div>
    `;
  }
}
