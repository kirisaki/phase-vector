import type { State, Vector2 } from './types.js';
import type { Projection } from './map-overlay.js';
import { element, query, required } from './dom.js';
import { readState } from './api.js';
import { navigate_map } from './pkg/phase_vector.js';
import { mapProjection } from './map-overlay.js';
import { NAVIGATION } from './ui-settings.js';

interface Pointer { x: number; y: number; type: string }
interface Gesture { view: Projection; start: Pointer | Vector2; anchor: Vector2; scale: number; span: number | null; moved: boolean }
interface NavigationOptions { onChange: () => void; onError: (error: unknown) => void; cancelEditors: () => void }

const editorTarget = '#arrival-handle, .reach-area, #aim-handle';

export function createMapNavigation({ onChange, onError, cancelEditors }: NavigationOptions) {
  const surface = query('.canvas-wrap');
  const overlay = element('route-editor');
  const pointers = new Map<number, Pointer>();
  let ready = false, owned = false, lastTick = 0;
  let gesture: Gesture | undefined, mission: boolean | undefined;
  const point = (event: PointerEvent) => ({ x: event.clientX, y: event.clientY, type: event.pointerType });
  const pair = () => [...pointers.values()].filter((p) => p.type === 'touch').slice(0, 2);
  const midpoint = (a: Vector2, b: Vector2) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
  const span = (a: Vector2, b: Vector2) => Math.hypot(a.x - b.x, a.y - b.y);
  const suppress = (event: Event) => { event.preventDefault(); event.stopImmediatePropagation(); };

  function startGesture() {
    const state = readState();
    const view = mapProjection(overlay, state.map);
    const touches = pair();
    const pinch = touches.length === 2;
    const start = pinch ? midpoint(touches[0], touches[1]) : required(pointers.values().next().value);
    gesture = { view, start, anchor: view.unproject(start.x, start.y), scale: state.map.metres_per_unit,
      span: pinch ? Math.max(NAVIGATION.minPinchSpanPixels, span(touches[0], touches[1])) : null, moved: false };
  }

  function move() {
    if (!gesture) return;
    const touches = pair();
    const pinch = gesture.span !== null;
    const current = pinch ? midpoint(touches[0], touches[1]) : required(pointers.values().next().value);
    if (!pinch && !gesture.moved && span(current, gesture.start) < NAVIGATION.dragThresholdPixels) return;
    const { view, anchor } = gesture;
    const scale = pinch ? gesture.scale * required(gesture.span) / Math.max(NAVIGATION.minPinchSpanPixels, span(touches[0], touches[1])) : gesture.scale;
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
    } else if (owned || !(event.target instanceof Element && event.target.closest(editorTarget))) {
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
    sync(state: State, isReady: boolean) {
      ready = isReady;
      if (state.world.tick < lastTick || (mission !== undefined && mission !== Boolean(state.mission))) cancel();
      lastTick = state.world.tick;
      mission = Boolean(state.mission);
    },
  };
}
