import { queue_shot, cancel_shot, preview_shot, command, set_route_editing } from './pkg/phase_vector.js';

import { mapProjection } from './map-overlay.js';
import { drawMissionMap } from './mission-map.js';

const el = (id) => document.getElementById(id);
const decimal = (n) => n.toFixed(1);
const RECENT_LAUNCHES = 4;
const AIM_SNAP_PIXELS = 12;
const FINE_AIM_GAIN = 0.35;

export function createMissionControls(onChange) {
  const svg = el('mission-overlay'), handle = el('aim-handle');
  let state, ready = false, blocked = false, target = null, solution, error = '';
  let pointer, beforeDrag, lastTick = 0, dragging = false;
  let aimGesture;
  let shownOutcome;
  const result = el('mission-result');
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
    if (!m || !ready) { svg.setAttribute('hidden', ''); result.close(); shownOutcome = undefined; return; }
    svg.removeAttribute('hidden');
    const done = finished();
    const titles = { running: 'Prevent breakthrough', breakthrough: 'Defeat · Enemy reached the exit',
      disabled: 'Victory · Enemy disabled', timeout: 'Victory · Deadline exceeded' };
    el('mission-status').textContent = titles[m.outcome];
    el('mission-status').dataset.outcome = m.outcome;
    const own = state.world.ships.find((ship) => ship.id === state.player_id);
    renderResult(m, own);
    const clock = done ? `Ended ${decimal(m.ended_seconds)} s` : `${decimal(Math.max(0, m.deadline_seconds - state.time_seconds))} s left`;
    el('mission-metrics').textContent = `${clock} · ${m.shots.length} shots · Δv ${decimal(own.spent_dv / 1000)} km/s`;
    const aim = target ?? m.aim?.target;
    let failure = '';
    try { solution = aim ? JSON.parse(preview_shot(aim.x, aim.y)) : null; }
    catch (e) { solution = null; failure = String(e); }
    el('queue-shot').disabled = !solution || done || blocked || dragging;
    el('queue-shot').textContent = `Queue ${m.queued.length}/${m.queue_capacity}`;
    el('queue-shot').setAttribute('aria-label', `Queue shot (${m.queued.length} of ${m.queue_capacity} reserved)`);
    el('track-aim').disabled = done || dragging;
    el('cancel-shot').disabled = !m.queued.some((order) => !order.locked) || done;
    el('retry').disabled = !done;
    el('gun-feedback').textContent = error || failure || (done ? 'Retry restores the initial scenario.' :
      blocked ? 'Apply numeric edits or finish the burn gesture before queuing a shot.' :
      m.queue_error ||
      `${target ? 'Fixed point' : 'Lead assist'} · fire ${solution ? decimal(solution.launch_tick * state.timing.step_seconds) : '—'} s · flight ${solution ? decimal(solution.flight_seconds) : '—'} s · ${aimEstimate(m)}`);
    renderQueue(m, done);
    renderLaunches(m);
    drawMissionMap(svg, state, { aim, solution, fixedTarget: Boolean(target), done });
  }

  function aimEstimate(m) {
    if (!solution) return 'No solution';
    const dt = solution.arrival_seconds - state.time_seconds;
    const miss = Math.hypot(solution.target.x - m.prediction[0].x - m.contact.velocity.x * dt,
      solution.target.y - m.prediction[0].y - m.contact.velocity.y * dt) / 1000;
    return `◇ predicted offset ${decimal(miss)} km; enemy may evade`;
  }

  function renderResult(m, own) {
    if (m.outcome === 'running') { result.close(); shownOutcome = undefined; return; }
    const victory = m.outcome !== 'breakthrough';
    result.dataset.outcome = victory ? 'victory' : 'defeat';
    el('result-title').textContent = victory ? 'VICTORY' : 'DEFEAT';
    el('result-reason').textContent = { disabled: 'Enemy disabled by a direct hit.',
      timeout: 'Blockade successful. The enemy missed its deadline.',
      breakthrough: 'The enemy reached the exit before the deadline.' }[m.outcome];
    el('result-time').textContent = `${decimal(m.ended_seconds)} s`;
    el('result-shots').textContent = m.shots.length;
    el('result-dv').textContent = `${decimal(own.spent_dv / 1000)} km/s`;
    if (shownOutcome !== m.outcome) {
      shownOutcome = m.outcome;
      result.showModal();
    }
  }

  function dismissResult() { result.close(); el('retry').focus({ preventScroll: true }); }
  const outsideResult = (event) => {
    const box = result.getBoundingClientRect();
    return event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom;
  };
  let pressedOutside = false;
  result.addEventListener('pointerdown', (event) => { pressedOutside = event.target === result && outsideResult(event); });
  result.addEventListener('click', (event) => {
    if (pressedOutside && event.target === result && outsideResult(event)) dismissResult();
    pressedOutside = false;
  });
  result.addEventListener('cancel', (event) => { event.preventDefault(); dismissResult(); });
  el('result-view').addEventListener('click', dismissResult);

  function renderQueue(m, done) {
    const list = el('shot-queue');
    const ids = new Set(m.queued.map((order) => String(order.id)));
    for (const node of list.querySelectorAll('[data-shot-id]')) {
      if (!ids.has(node.dataset.shotId)) node.remove();
    }
    el('empty-queue').hidden = Boolean(m.queued.length);
    m.queued.forEach((order, index) => {
      let row = list.querySelector(`[data-shot-id="${order.id}"]`);
      if (!row) {
        row = document.createElement('div'); row.dataset.shotId = order.id;
        row.className = 'queued-shot'; row.setAttribute('role', 'listitem');
        row.append(document.createElement('span'), document.createElement('button'));
        const button = row.querySelector('button'); button.textContent = 'Cancel';
        button.setAttribute('aria-label', `Cancel shot ${order.id}`);
        button.addEventListener('click', () => invoke(() => cancel_shot(order.id)));
      }
      // Keep nodes and focus stable across polling updates, including insertion into a freed slot.
      const sibling = list.querySelectorAll('[data-shot-id]')[index];
      if (sibling !== row) list.insertBefore(row, sibling ?? null);
      row.querySelector('span').textContent = `#${order.id} · fire ${decimal(order.launch_tick * state.timing.step_seconds)} s · arrival ${order.solution ? decimal(order.solution.arrival_seconds) + ' s' : 'unreachable'}`;
      row.title = order.error || `Target (${decimal(order.target.x / 1000)}, ${decimal(order.target.y / 1000)}) km`;
      row.classList.toggle('invalid', Boolean(order.error));
      row.querySelector('button').disabled = done || order.locked;
      row.querySelector('button').title = order.locked ? 'Committed to the current command interval' : 'Cancel this reservation';
    });
  }

  function invoke(action) {
    try { action(); error = ''; } catch (e) { error = String(e); }
    onChange();
  }

  function renderLaunches(m) {
    const list = el('launch-list');
    const recent = m.shots.slice(-RECENT_LAUNCHES).reverse();
    const ids = new Set(recent.map((shot) => String(shot.order_id)));
    for (const row of [...list.children]) {
      if (!ids.has(row.dataset.shotId)) row.remove();
    }
    el('empty-history').hidden = Boolean(recent.length);
    recent.forEach((shot, index) => {
      let row = list.querySelector(`[data-shot-id="${shot.order_id}"]`);
      if (!row) {
        row = document.createElement('div');
        row.dataset.shotId = shot.order_id;
        row.setAttribute('role', 'listitem');
      }
      if (list.children[index] !== row) list.insertBefore(row, list.children[index] ?? null);
      row.textContent = `#${shot.order_id} · fired ${decimal(shot.launch_tick * state.timing.step_seconds)} s`;
      row.title = `Target passage ${decimal(shot.arrival_seconds)} s. Arrival at the target point does not guarantee a hit.`;
    });
  }
  el('queue-shot').addEventListener('click', () => invoke(() => {
    if (solution) queue_shot(solution.target.x, solution.target.y);
  }));
  el('cancel-shot').addEventListener('click', () => invoke(() => command('cancel_shot')));
  el('track-aim').addEventListener('click', () => { target = null; error = ''; render(); });
  const retry = () => { invoke(() => { target = null; command('retry'); }); el('toggle').focus({ preventScroll: true }); };
  el('retry').addEventListener('click', retry);
  el('result-retry').addEventListener('click', retry);

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
