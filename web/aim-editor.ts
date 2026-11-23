import type { Vector2 } from './types.js';
import type { Projection } from './map-overlay.js';
import { required } from './dom.js';
import { AIM } from './ui-settings.js';

interface AimOptions {
  svg: SVGSVGElement; handle: SVGGElement; projection: () => Projection;
  leadTarget: () => Vector2 | null | undefined; enabled: () => boolean;
  onChange: () => void; onMove: () => void; onEditingChange: (editing: boolean) => void;
  onFineStart: (view: Projection) => void;
  onFineEnd: (applied: boolean) => void;
}

// Own the draft target and pointer lifecycle; the mission controller issues commands.
export function createAimEditor({ svg, handle, projection, leadTarget, enabled, onChange, onMove, onEditingChange, onFineStart, onFineEnd }: AimOptions) {
  let target: Vector2 | null = null, beforeDrag: Vector2 | null = null;
  let pointer: number | undefined;
  let aimGesture: { view: Projection; source: Vector2; x: number; y: number; gain: number; fine: boolean; pointer: Vector2 } | undefined;
  let dragging = false;
  function finish(apply: boolean) {
    if (!dragging) return;
    const fine = aimGesture?.fine;
    dragging = false;
    aimGesture = undefined;
    if (!apply) target = beforeDrag;
    const captured = pointer; pointer = undefined;
    if (captured !== undefined && svg.hasPointerCapture(captured)) svg.releasePointerCapture(captured);
    if (fine) onFineEnd(apply);
    onEditingChange(false);
  }
  handle.addEventListener('pointerdown', (event) => {
    if (!enabled() || dragging || event.button !== 0) return;
    const source = target ?? leadTarget();
    if (!source) return;
    const fine = event.target instanceof Element && Boolean(event.target.closest('#aim-grip'));
    aimGesture = { view: projection(), source, x: event.clientX, y: event.clientY,
      gain: fine ? AIM.fineGain : 1, fine, pointer: { x: event.clientX, y: event.clientY } };
    event.preventDefault(); beforeDrag = target; dragging = true; pointer = event.pointerId;
    handle.focus({ preventScroll: true }); svg.setPointerCapture(pointer);
    if (fine) onFineStart(aimGesture.view);
    onEditingChange(true);
  });
  svg.addEventListener('pointermove', (event) => {
    if (!dragging || event.pointerId !== pointer) return;
    const { view, source, x, y, gain } = required(aimGesture);
    required(aimGesture).pointer = { x: event.clientX, y: event.clientY };
    target = { x: source.x + (event.clientX - x) * view.metresPerPixel * gain,
      y: source.y - (event.clientY - y) * view.metresPerPixel * gain };
    const lead = leadTarget();
    // Fine aim deliberately bypasses snapping so near-intercept offsets remain editable.
    if (gain === 1 && !event.shiftKey && lead &&
      Math.hypot(target.x - lead.x, target.y - lead.y) <= AIM.snapPixels * view.metresPerPixel) target = null;
    onMove();
  });
  svg.addEventListener('pointerup', (event) => { if (event.pointerId === pointer) finish(true); });
  svg.addEventListener('pointercancel', () => finish(false));
  svg.addEventListener('lostpointercapture', () => finish(false));
  window.addEventListener('blur', () => finish(false));
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && dragging) { event.preventDefault(); finish(false); }
  });
  handle.addEventListener('keydown', (event) => {
    if (!enabled() || dragging) return;
    const directions: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, 1], ArrowDown: [0, -1] };
    if (event.key === 'Home') { event.preventDefault(); target = null; onChange(); return; }
    const delta = directions[event.key];
    if (!delta) return;
    event.preventDefault();
    const source = target ?? leadTarget(), step = event.shiftKey ? AIM.fineStepMetres : AIM.stepMetres;
    if (!source) return;
    target = { x: source.x + delta[0] * step, y: source.y + delta[1] * step }; onMove();
  });
  return {
    get target() { return target; },
    get dragging() { return dragging; },
    get fineDragging() { return Boolean(dragging && aimGesture?.fine); },
    get finePointer() { return aimGesture?.fine ? aimGesture.pointer : undefined; },
    clear() { target = null; },
    cancel() { finish(false); },
  };
}
