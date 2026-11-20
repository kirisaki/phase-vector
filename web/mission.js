import { queue_shot, preview_shot, command, set_route_editing } from './pkg/phase_vector.js';

import { mapProjection } from './map-overlay.js';
import { drawMissionMap } from './mission-map.js';

const el = (id) => document.getElementById(id);
const decimal = (n) => n.toFixed(1);

export function createMissionControls(onChange) {
  const svg = el('mission-overlay'), handle = el('aim-handle');
  let state, ready = false, blocked = false, target = null, solution, error = '';
  let pointer, beforeDrag, lastTick = 0, dragging = false;
  const finished = () => state?.mission?.outcome !== 'running';

  function projection() {
    return mapProjection(svg, state.map);
  }

  function render() {
    const m = state?.mission;
    el('mission-controls').hidden = !m;
    document.body.classList.toggle('mission-active', Boolean(m));
    document.querySelector('.badge').textContent = m ? '03 / BLOCKADE' : '02 / PHYSICS & WEGO';
    if (!m || !ready) { svg.setAttribute('hidden', ''); return; }
    svg.removeAttribute('hidden');
    const done = finished();
    const titles = { running: 'Prevent breakthrough', breakthrough: 'Defeat · Enemy reached the exit',
      disabled: 'Victory · Enemy disabled', timeout: 'Victory · Deadline exceeded' };
    el('mission-status').textContent = titles[m.outcome];
    el('mission-status').dataset.outcome = m.outcome;
    const own = state.world.ships.find((ship) => ship.id === state.player_id);
    const clock = done ? `Ended ${decimal(m.ended_seconds)} s` : `${decimal(Math.max(0, m.deadline_seconds - state.time_seconds))} s left`;
    el('mission-metrics').textContent = `${clock} · ${m.shots.length} shots · Δv ${decimal(own.spent_dv / 1000)} km/s`;
    const aim = target ?? m.aim?.target;
    let failure = '';
    try { solution = aim ? JSON.parse(preview_shot(aim.x, aim.y)) : null; }
    catch (e) { solution = null; failure = String(e); }
    const reload = state.order_start_seconds < m.ready_seconds;
    const tooLate = state.order_start_seconds >= m.deadline_seconds;
    el('queue-shot').disabled = !solution || done || reload || tooLate || blocked || dragging;
    el('queue-shot').textContent = m.queued ? 'Replace shot' : 'Queue shot';
    el('track-aim').disabled = done || dragging;
    el('cancel-shot').disabled = !m.queued || done;
    el('retry').disabled = !done;
    const queued = m.queued ? `Queued at ${decimal(m.queued.launch_tick * state.timing.step_seconds)} s. ` : '';
    el('gun-feedback').textContent = error || failure || (done ? 'Retry restores the initial scenario.' :
      blocked ? 'Apply numeric edits or finish the burn gesture before queuing a shot.' :
      tooLate ? 'No command boundary remains before the deadline.' :
      reload ? `Reloading until ${decimal(m.ready_seconds)} s.` :
      `${queued}${target ? 'Fixed point' : 'Observed lead'} · launch ${decimal(state.order_start_seconds)} s · flight ${solution ? decimal(solution.flight_seconds) : '—'} s · contact age ${decimal(m.contact_age)} s`);
    drawMissionMap(svg, state, { aim, solution, fixedTarget: Boolean(target), done });
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
  el('retry').addEventListener('click', () => invoke(() => { target = null; command('retry'); }));

  function finish(apply) {
    if (!dragging) return;
    dragging = false;
    if (!apply) target = beforeDrag;
    const captured = pointer; pointer = undefined;
    if (svg.hasPointerCapture(captured)) svg.releasePointerCapture(captured);
    set_route_editing(false);
    onChange();
  }
  handle.addEventListener('pointerdown', (event) => {
    if (!ready || finished() || blocked || dragging || event.button !== 0) return;
    event.preventDefault(); beforeDrag = target; dragging = true; pointer = event.pointerId;
    handle.focus({ preventScroll: true }); svg.setPointerCapture(pointer);
    set_route_editing(true); onChange();
  });
  svg.addEventListener('pointermove', (event) => {
    if (!dragging || event.pointerId !== pointer) return;
    target = projection().unproject(event.clientX, event.clientY); error = ''; render();
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
    const source = target ?? state.mission.aim.target, step = event.shiftKey ? 10000 : 100000;
    target = { x: source.x + delta[0] * step, y: source.y + delta[1] * step }; error = ''; render();
  });
  new ResizeObserver(() => { if (state) render(); }).observe(svg.parentElement);
  return { get dragging() { return dragging; }, sync(value, isReady, isBlocked) {
    const reset = value.world.tick < lastTick || Boolean(state?.mission) !== Boolean(value.mission);
    state = value; ready = isReady; blocked = isBlocked;
    if (dragging && (reset || finished())) finish(false);
    if (reset) { target = null; error = ''; }
    lastTick = state.world.tick;
    render();
  } };
}
