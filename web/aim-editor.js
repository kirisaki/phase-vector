import { AIM } from './ui-settings.js';

// Own the draft target and pointer lifecycle; the mission controller issues commands.
export function createAimEditor({ svg, handle, projection, leadTarget, enabled, onChange, onMove, onEditingChange }) {
  let target = null, pointer, beforeDrag, aimGesture;
  let dragging = false;
  function finish(apply) {
    if (!dragging) return;
    dragging = false;
    aimGesture = undefined;
    if (!apply) target = beforeDrag;
    const captured = pointer; pointer = undefined;
    if (svg.hasPointerCapture(captured)) svg.releasePointerCapture(captured);
    onEditingChange(false);
  }
  handle.addEventListener('pointerdown', (event) => {
    if (!enabled() || dragging || event.button !== 0) return;
    const source = target ?? leadTarget();
    if (!source) return;
    aimGesture = { view: projection(), source, x: event.clientX, y: event.clientY,
      gain: event.target.closest('#aim-grip') ? AIM.fineGain : 1 };
    event.preventDefault(); beforeDrag = target; dragging = true; pointer = event.pointerId;
    handle.focus({ preventScroll: true }); svg.setPointerCapture(pointer);
    onEditingChange(true);
  });
  svg.addEventListener('pointermove', (event) => {
    if (!dragging || event.pointerId !== pointer) return;
    const { view, source, x, y, gain } = aimGesture;
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
    const directions = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, 1], ArrowDown: [0, -1] };
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
    clear() { target = null; },
    cancel() { finish(false); },
  };
}
