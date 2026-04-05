/**
 * Shared drag-and-drop state module.
 *
 * When a drag starts in either the local or remote file browser, the source
 * records its type and payload here. The target browser reads this on dragover/drop
 * to decide whether to accept and what to do.
 *
 * This avoids relying on dataTransfer.getData() restrictions (which only works in
 * `drop`, not `dragover`) while keeping both browsers decoupled.
 */

export type DndSource =
  | { type: "local"; paths: string[] }
  | { type: "remote"; profileId: string; currentPath: string; names: string[] };

let _active: DndSource | null = null;

/** Called by the dragging browser when a drag starts. */
export function setDragSource(source: DndSource): void {
  _active = source;
}

/** Called by the dropping browser to inspect the current drag source. */
export function getDragSource(): DndSource | null {
  return _active;
}

/** Clear after drag ends (dragend / drop). */
export function clearDragSource(): void {
  _active = null;
}
