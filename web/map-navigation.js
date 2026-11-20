import { navigate_map, state_json } from './pkg/phase_vector.js';
import { mapProjection } from './map-overlay.js';

const DRAG_THRESHOLD = 4;
const MIN_PINCH_SPAN = 8;
const editorTarget = '#arrival-handle, .reach-area, #aim-handle';

export function createMapNavigation({ onChange, onError, cancelEditors }) {
  const surface = document.querySelector('.canvas-wrap');
  const overlay = document.getElementById('route-editor');
  const pointers = new Map();
  let ready = false, owned = false, gesture, lastTick = 0, mission;
  const point = (event) => ({ x: event.clientX, y: event.clientY, type: event.pointerType });
  const pair = () => [...pointers.values()].filter((p) => p.type === 'touch').slice(0, 2);
  const midpoint = (a, b) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
  const span = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
  const suppress = (event) => { event.preventDefault(); event.stopImmediatePropagation(); };

  function startGesture() {
    const state = JSON.parse(state_json());
    const view = mapProjection(overlay, state.map);
    const touches = pair();
    const pinch = touches.length === 2;
    const start = pinch ? midpoint(...touches) : pointers.values().next().value;
    gesture = { view, start, anchor: view.unproject(start.x, start.y), scale: state.map.metres_per_unit,
      span: pinch ? Math.max(MIN_PINCH_SPAN, span(...touches)) : null, moved: false };
  }

  function move() {
    if (!gesture) return;
    const touches = pair();
    const pinch = gesture.span !== null;
    const current = pinch ? midpoint(...touches) : pointers.values().next().value;
    if (!pinch && !gesture.moved && span(current, gesture.start) < DRAG_THRESHOLD) return;
    const { view, anchor } = gesture;
    const scale = pinch ? gesture.scale * gesture.span / Math.max(MIN_PINCH_SPAN, span(...touches)) : gesture.scale;
    const x = (current.x - view.box.left - view.width / 2) * view.unitsPerPixel;
    const y = (view.height / 2 - (current.y - view.box.top)) * view.unitsPerPixel;
    try {
      navigate_map(anchor.x, anchor.y, x, y, scale);
      gesture.moved = true;
      onChange();
    } catch (error) { cancel(); onError(error); }
  }

  function cancel() {
    const captured = [...pointers.keys()];
    const wasOwned = owned;
    pointers.clear(); gesture = undefined; owned = false;
    delete surface.dataset.navigating;
    for (const id of captured) {
      if (surface.hasPointerCapture(id)) surface.releasePointerCapture(id);
    }
    if (wasOwned) onChange();
  }

  // Capture before the editors: a second touch cancels their draft before taking ownership.
  surface.addEventListener('pointerdown', (event) => {
    if (!ready || event.button !== 0) return;
    if (event.pointerType !== 'touch' && pointers.size) return;
    pointers.set(event.pointerId, point(event));
    if (pair().length === 2) {
      owned = true;
      surface.dataset.navigating = 'true';
      cancelEditors();
      for (const id of pointers.keys()) surface.setPointerCapture(id);
      startGesture();
      suppress(event);
      move();
    } else if (owned || !event.target.closest(editorTarget)) {
      owned = true;
      surface.dataset.navigating = 'true';
      surface.setPointerCapture(event.pointerId);
      startGesture();
      suppress(event);
      onChange();
    }
  }, { capture: true });

  surface.addEventListener('pointermove', (event) => {
    if (!pointers.has(event.pointerId)) return;
    pointers.set(event.pointerId, point(event));
    if (!owned) return;
    suppress(event); move();
  }, { capture: true });

  surface.addEventListener('pointerup', (event) => {
    if (!pointers.has(event.pointerId)) return;
    if (!owned) { pointers.delete(event.pointerId); return; }
    pointers.set(event.pointerId, point(event));
    suppress(event); move();
    pointers.delete(event.pointerId);
    if (surface.hasPointerCapture(event.pointerId)) surface.releasePointerCapture(event.pointerId);
    if (pointers.size) startGesture();
    else cancel();
  }, { capture: true });

  surface.addEventListener('pointercancel', (event) => {
    if (!pointers.has(event.pointerId)) return;
    if (owned) suppress(event);
    cancel();
  }, { capture: true });
  surface.addEventListener('lostpointercapture', (event) => {
    if (owned && event.target === surface && pointers.has(event.pointerId) && !surface.hasPointerCapture(event.pointerId)) cancel();
    else if (!owned) pointers.delete(event.pointerId);
  });
  surface.addEventListener('keydown', (event) => {
    if (owned) { if (event.key === 'Escape') cancel(); suppress(event); }
  }, { capture: true });
  document.addEventListener('keydown', (event) => {
    if (owned && event.key === 'Escape') { suppress(event); cancel(); }
  }, { capture: true });
  window.addEventListener('blur', cancel);
  document.addEventListener('visibilitychange', () => { if (document.hidden) cancel(); });
  new ResizeObserver(cancel).observe(surface);

  return {
    get active() { return owned; },
    cancel,
    sync(state, isReady) {
      ready = isReady;
      if (state.world.tick < lastTick || (mission !== undefined && mission !== Boolean(state.mission))) cancel();
      lastTick = state.world.tick;
      mission = Boolean(state.mission);
    },
  };
}
