import { listen } from "@tauri-apps/api/event";
import * as api from "../api/index";
import type { TransferJobView } from "../types";
import { t } from "../i18n";

function escHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function fmtPct(j: TransferJobView): number {
  if (j.state === "done") return 100;
  if (j.bytesTotal > 0) return Math.min(100, (j.bytesDone / j.bytesTotal) * 100);
  return j.state === "active" ? 100 : 0; // indeterminate -> full bar styled separately
}

/**
 * Bottom queue panel. Holds a map of jobs keyed by id, updated from the
 * `transfer-update` Tauri event, and re-renders on each change. Hidden when
 * there are no jobs.
 *
 * Rendering strategy:
 *   - Full render (innerHTML rebuild + listener re-attach) only when STRUCTURE
 *     changes: the set of job ids, any job's state, or empty↔non-empty toggle.
 *   - Progress-only updates (same ids, same states, only bytesDone/bytesTotal/
 *     filename changed) use updateRowInPlace() which mutates existing DOM nodes
 *     without touching listeners — eliminates flicker and dropped clicks at
 *     high event rates (~80 events/s with 8 concurrent jobs).
 */
export class TransferQueuePanel {
  private container: HTMLElement;
  private jobs = new Map<number, TransferJobView>();
  /** Called when any job reaches a terminal state, with the finished job. */
  private onJobFinished: ((job: TransferJobView) => void) | null = null;
  /** Structure signature of the last full render: sorted "id:state" pairs. */
  private lastStructureSig = "";

  constructor(containerId: string) {
    const el = document.getElementById(containerId);
    if (!el) throw new Error(`Element #${containerId} not found`);
    this.container = el;
    this.render();
    void this.subscribe();
  }

  setOnJobFinished(cb: (job: TransferJobView) => void): void {
    this.onJobFinished = cb;
  }

  /** Build a structure signature from sorted id:state pairs. */
  private structureSig(): string {
    return [...this.jobs.values()]
      .sort((a, b) => a.id - b.id)
      .map((j) => `${j.id}:${j.state}`)
      .join(",");
  }

  private async subscribe(): Promise<void> {
    // Seed from any jobs already in the backend (e.g. after a reload).
    try {
      for (const j of await api.listTransfers()) this.jobs.set(j.id, j);
      this.render();
    } catch {
      /* ignore */
    }
    await listen<TransferJobView>("transfer-update", (e) => {
      const job = e.payload;
      this.jobs.set(job.id, job);

      const sig = this.structureSig();
      if (sig !== this.lastStructureSig) {
        // Structural change (new/removed job or state transition) — full render.
        this.render();
      } else {
        // Progress-only change — update existing row without touching listeners.
        this.updateRowInPlace(job);
      }

      if (job.state === "done" || job.state === "failed" || job.state === "cancelled") {
        this.onJobFinished?.(job);
      }
    });
  }

  private activeOrQueued(): boolean {
    for (const j of this.jobs.values()) {
      if (j.state === "queued" || j.state === "active") return true;
    }
    return false;
  }

  /**
   * Update a single row's progress elements in place without rebuilding DOM.
   * Falls back to full render if the row is missing (e.g. first render race).
   */
  private updateRowInPlace(job: TransferJobView): void {
    const row = this.container.querySelector<HTMLElement>(`.tq-row[data-job="${job.id}"]`);
    if (!row) {
      // Row not found — fall back to full render to stay consistent.
      this.render();
      return;
    }

    // Update filename.
    const nameEl = row.querySelector<HTMLElement>(".tq-name");
    if (nameEl) {
      nameEl.textContent = job.filename;
      nameEl.setAttribute("title", job.dst);
    }

    // Update state label.
    const stateEl = row.querySelector<HTMLElement>(".tq-state");
    if (stateEl) {
      stateEl.textContent = t(`transferQueue.state_${job.state}`);
    }

    // Update progress bar.
    const fill = row.querySelector<HTMLElement>(".tq-fill");
    if (fill) {
      const pct = fmtPct(job);
      const indeterminate = job.state === "active" && job.bytesTotal === 0;
      fill.style.width = `${pct.toFixed(1)}%`;
      fill.classList.toggle("tq-fill--indeterminate", indeterminate);
    }
  }

  private render(): void {
    if (this.jobs.size === 0) {
      this.container.innerHTML = "";
      this.container.classList.add("hidden");
      this.lastStructureSig = "";
      return;
    }
    this.container.classList.remove("hidden");

    const rows = [...this.jobs.values()]
      .sort((a, b) => a.id - b.id)
      .map((j) => {
        const pct = fmtPct(j);
        const indeterminate = j.state === "active" && j.bytesTotal === 0;
        const stateLabel = t(`transferQueue.state_${j.state}`);
        const cancellable = j.state === "queued" || j.state === "active";
        const arrow = j.kind === "upload" || j.kind === "uploadDir" ? "↑" : "↓";
        return `
          <div class="tq-row tq-row--${j.state}" data-job="${j.id}">
            <span class="tq-arrow">${arrow}</span>
            <span class="tq-name" title="${escHtml(j.dst)}">${escHtml(j.filename)}</span>
            <span class="tq-state">${escHtml(stateLabel)}</span>
            <div class="tq-track">
              <div class="tq-fill ${indeterminate ? "tq-fill--indeterminate" : ""}" style="width:${pct.toFixed(1)}%"></div>
            </div>
            ${j.error ? `<span class="tq-error" title="${escHtml(j.error)}">${escHtml(j.error)}</span>` : ""}
            ${cancellable ? `<button class="tq-cancel" data-job="${j.id}" title="${escHtml(t("transferQueue.cancel"))}">✕</button>` : ""}
          </div>`;
      })
      .join("");

    this.container.innerHTML = `
      <div class="tq-header">
        <span class="tq-title">${escHtml(t("transferQueue.title"))}</span>
        ${this.activeOrQueued() ? `<button class="tq-cancel-all" id="tq-cancel-all">${escHtml(t("transferQueue.cancelAll"))}</button>` : ""}
        <button class="tq-clear" id="tq-clear">${escHtml(t("transferQueue.clearFinished"))}</button>
      </div>
      <div class="tq-rows">${rows}</div>`;

    this.lastStructureSig = this.structureSig();

    this.container.querySelectorAll<HTMLButtonElement>(".tq-cancel").forEach((btn) => {
      btn.addEventListener("click", () => {
        const id = Number(btn.dataset.job);
        if (!Number.isNaN(id)) api.cancelTransfer(id).catch(() => {});
      });
    });
    this.container.querySelector("#tq-cancel-all")?.addEventListener("click", () => {
      api.cancelAllTransfers().catch(() => {});
    });
    this.container.querySelector("#tq-clear")?.addEventListener("click", () => {
      for (const [id, j] of [...this.jobs.entries()]) {
        if (j.state === "done" || j.state === "failed" || j.state === "cancelled") this.jobs.delete(id);
      }
      api.clearFinishedTransfers().catch(() => {});
      this.render();
    });
  }
}
