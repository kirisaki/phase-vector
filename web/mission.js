import { queue_shot, cancel_shot, preview_shot, command, set_route_editing } from './pkg/phase_vector.js';

import { mapProjection } from './map-overlay.js';
import { drawMissionMap } from './mission-map.js';
import { createMissionPanel } from './mission-panel.js';
import { shotArrivalEstimate } from './mission-preview.js';

const el = (id) => document.getElementById(id);
const decimal = (n) => n.toFixed(1);
const AIM_SNAP_PIXELS = 12;
const FINE_AIM_GAIN = 0.35;

export function createMissionControls(onChange) {
  const svg = el('mission-overlay'), handle = el('aim-handle');
  let state, ready = false, blocked = false, target = null, solution, error = '';
  let pointer, beforeDrag, lastTick = 0, dragging = false;
  let aimGesture;
  const panel = createMissionPanel({
    onCancelShot: (id) => invoke(() => cancel_shot(id)),
    onRetry: () => { invoke(() => { target = null; command('retry'); }); el('toggle').focus({ preventScroll: true }); },
  });
  const finished = () => state?.mission?.outcome !== 'running';

  function projection() {
    return mapProjection(svg, state.map);
  }

  function render() {
    const m = state?.mission;
    el('mission-controls').hidden = !m;
    el('game-controls').hidden = !m;
    document.body.classList.toggle('mission-active', Boolean(m));
    document.querySelector('.badge').textContent = m ? '03 / BLOCKADE' : '02 / PHYSICS & WEGO';
    panel.render(state, ready);
    if (!m || !ready) { svg.setAttribute('hidden', ''); return; }
    svg.removeAttribute('hidden');
    const done = finished();
    const aim = target ?? m.aim?.target;
    let failure = '';
    try { solution = aim ? JSON.parse(preview_shot(aim.x, aim.y)) : null; }
    catch (e) { solution = null; failure = String(e); }
    el('queue-shot').disabled = !solution || done || blocked || dragging;
    el('queue-shot').textContent = `Queue ${m.queued.length}/${m.queue_capacity}`;
    el('queue-shot').setAttribute('aria-label', `Queue shot (${m.queued.length} of ${m.queue_capacity} reserved)`);
    el('track-aim').disabled = done || dragging;
    el('cancel-shot').disabled = !m.queued.some((order) => !order.locked) || done;
    const arrival = shotArrivalEstimate(state, solution);
    const estimate = arrival ? `◇ predicted offset ${decimal(arrival.offsetMetres / 1000)} km; enemy may evade` : 'No solution';
    el('gun-feedback').textContent = error || failure || (done ? 'Retry restores the initial scenario.' :
      blocked ? 'Apply numeric edits or finish the burn gesture before queuing a shot.' :
      m.queue_error ||
      `${target ? 'Fixed point' : 'Lead assist'} · fire ${solution ? decimal(solution.launch_tick * state.timing.step_seconds) : '—'} s · flight ${solution ? decimal(solution.flight_seconds) : '—'} s · ${estimate}`);
    drawMissionMap(svg, state, { aim, solution, arrival, fixedTarget: Boolean(target), done });
  }

  function invoke(action) {
    try { action(); error = ''; } catch (e) { error = String(e); }
    onChange();
  }

  el('queue-shot').addEventListener('click', () => invoke(() => {
    if (solution) queue_shot(solution.target.x, solution.target.y);
  }));
  el('cancel-shot').addEventListener('click', () => invoke(() => command('cancel_shot')));
  el('track-aim').addEventListener('click', () => { target = null; error = ''; render(); });

  function finish(apply) {
    if (!dragging) return;
    dragging = false;
    aimGesture = undefined;
    if (!apply) target = beforeDrag;
    const captured = pointer; pointer = undefined;
    if (svg.hasPointerCapture(captured)) svg.releasePointerCapture(captured);
    set_route_editing(false);
    onChange();
  }
  handle.addEventListener('pointerdown', (event) => {
    if (!ready || finished() || blocked || dragging || event.button !== 0) return;
    const source = target ?? state.mission.aim?.target;
    if (!source) return;
    aimGesture = { view: projection(), source, x: event.clientX, y: event.clientY,
      gain: event.target.closest('#aim-grip') ? FINE_AIM_GAIN : 1 };
    event.preventDefault(); beforeDrag = target; dragging = true; pointer = event.pointerId;
    handle.focus({ preventScroll: true }); svg.setPointerCapture(pointer);
    set_route_editing(true); onChange();
  });
  svg.addEventListener('pointermove', (event) => {
    if (!dragging || event.pointerId !== pointer) return;
    const { view, source, x, y, gain } = aimGesture;
    target = { x: source.x + (event.clientX - x) * view.metresPerPixel * gain,
      y: source.y - (event.clientY - y) * view.metresPerPixel * gain };
    const lead = state.mission.aim?.target;
    // Fine aim deliberately bypasses snapping so near-intercept offsets remain editable.
    if (gain === 1 && !event.shiftKey && lead &&
      Math.hypot(target.x - lead.x, target.y - lead.y) <= AIM_SNAP_PIXELS * view.metresPerPixel) target = null;
    error = ''; render();
  });
  svg.addEventListener('pointerup', (event) => { if (event.pointerId === pointer) finish(true); });
  svg.addEventListener('pointercancel', () => finish(false));
  svg.addEventListener('lostpointercapture', () => finish(false));
  window.addEventListener('blur', () => finish(false));
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && dragging) { event.preventDefault(); finish(false); }
  });
  handle.addEventListener('keydown', (event) => {
    if (!ready || finished() || blocked || dragging) return;
    const directions = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, 1], ArrowDown: [0, -1] };
    if (event.key === 'Home') { event.preventDefault(); target = null; render(); return; }
    const delta = directions[event.key];
    if (!delta) return;
    event.preventDefault();
    const source = target ?? state.mission.aim?.target, step = event.shiftKey ? 10000 : 100000;
    if (!source) return;
    target = { x: source.x + delta[0] * step, y: source.y + delta[1] * step }; error = ''; render();
  });
  new ResizeObserver(() => { if (state) render(); }).observe(svg.parentElement);
  return { get dragging() { return dragging; }, cancel() { finish(false); }, sync(value, isReady, isBlocked) {
    const reset = value.world.tick < lastTick || Boolean(state?.mission) !== Boolean(value.mission);
    state = value; ready = isReady; blocked = isBlocked;
    if (dragging && (reset || finished())) finish(false);
    if (reset) { target = null; error = ''; }
    lastTick = state.world.tick;
    render();
  } };
}
