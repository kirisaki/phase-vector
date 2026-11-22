import { queue_shot, queue_launch_shot, cancel_shot, preview_shot, preview_launch_shot, command, set_route_editing } from './pkg/phase_vector.js';

import { mapProjection } from './map-overlay.js';
import { drawMissionMap } from './mission-map.js';
import { createMissionPanel } from './mission-panel.js';
import { shotArrivalEstimate } from './mission-preview.js';
import { createTimeline } from './timeline.js';
import { createAimEditor } from './aim-editor.js';

const el = (id) => document.getElementById(id);
const decimal = (n) => n.toFixed(1);

export function createMissionControls(onChange) {
  const svg = el('mission-overlay'), handle = el('aim-handle');
  let state, ready = false, blocked = false, solution, error = '';
  let lastTick = 0;
  const timeline = createTimeline(() => { error = ''; onChange(); });
  const panel = createMissionPanel({
    onCancelShot: (id) => invoke(() => cancel_shot(id)),
    onRetry: () => { invoke(() => { aimEditor.clear(); timeline.reset(); command('retry'); }); el('toggle').focus({ preventScroll: true }); },
  });
  const finished = () => state?.mission?.outcome !== 'running';
  const leadTarget = () => timeline.seconds === null ? state.mission.aim?.target : timeline.forecast?.aim?.target;

  const aimEditor = createAimEditor({
    svg, handle, projection: () => mapProjection(svg, state.map), leadTarget,
    enabled: () => ready && !finished() && !blocked,
    onChange: render,
    onMove: () => { error = ''; render(); },
    onEditingChange: (editing) => { set_route_editing(editing); onChange(); },
  });

  function render() {
    const m = state?.mission;
    el('mission-controls').hidden = !m;
    el('game-controls').hidden = !m;
    document.body.classList.toggle('mission-active', Boolean(m));
    document.querySelector('.badge').textContent = m ? '04 / PREDICTION' : '02 / PHYSICS & WEGO';
    panel.render(state, ready);
    timeline.sync(state, ready, aimEditor.dragging || blocked);
    if (!m || !ready) { svg.setAttribute('hidden', ''); return; }
    svg.removeAttribute('hidden');
    const done = finished();
    const aim = aimEditor.target ?? leadTarget();
    let failure = '';
    try { solution = aim ? JSON.parse(timeline.seconds === null ? preview_shot(aim.x, aim.y)
      : preview_launch_shot(aim.x, aim.y, timeline.seconds)) : null; }
    catch (e) {
      solution = null; failure = String(e);
      if (failure.startsWith('Unreachable arrival:')) failure =
        'Unreachable arrival: the round cannot reach this target at the selected time. Choose a later arrival or a closer target.';
    }
    el('queue-shot').disabled = !solution || done || blocked || aimEditor.dragging;
    const unreachable = failure.startsWith('Unreachable arrival:');
    el('queue-shot').textContent = `Queue ${m.queued.length}/${m.queue_capacity}${unreachable ? '\nUnreachable' : ''}`;
    el('queue-shot').dataset.unreachable = String(unreachable);
    el('queue-shot').title = failure || 'Queue the current target and firing time. Q works while the timeline is focused.';
    el('gun-feedback').dataset.invalid = String(Boolean(failure || error));
    el('queue-shot').setAttribute('aria-label', `Queue shot (${m.queued.length} of ${m.queue_capacity} reserved)`);
    el('track-aim').disabled = done || aimEditor.dragging;
    el('cancel-shot').disabled = !m.queued.some((order) => !order.locked) || done;
    const arrival = shotArrivalEstimate(state, solution);
    const estimate = arrival ? `◇ predicted offset ${decimal(arrival.offsetMetres / 1000)} km; enemy may evade` : 'No solution';
    el('gun-feedback').textContent = error || timeline.error || failure || (done ? 'Retry restores the initial scenario.' :
      blocked ? 'Apply numeric edits or finish the burn gesture before queuing a shot.' :
      m.queued.find((order) => order.error)?.error || m.queue_error ||
      (timeline.seconds !== null && solution ? `Fire ${decimal(solution.launch_tick * state.timing.step_seconds)} s · arrival ${solution.arrival_seconds.toFixed(2)} s · flight ${decimal(solution.flight_seconds)} s · ${estimate}` :
      `${aimEditor.target ? 'Fixed point' : 'Lead assist'} · fire ${solution ? decimal(solution.launch_tick * state.timing.step_seconds) : '—'} s · flight ${solution ? decimal(solution.flight_seconds) : '—'} s · ${estimate}`));
    drawMissionMap(svg, state, { aim, solution, arrival, forecast: timeline.forecast, fixedTarget: Boolean(aimEditor.target), done });
  }

  function invoke(action) {
    try { action(); error = ''; } catch (e) { error = String(e); }
    onChange();
  }

  const queue = el('queue-shot');
  function queueCurrentShot() {
    if (!ready || !state?.mission || finished() || blocked || aimEditor.dragging || queue.disabled || !solution) return;
    // Capture one immutable target/firing-time pair for every input path.
    const { x, y } = solution.target, seconds = timeline.seconds;
    invoke(() => {
      if (seconds === null) queue_shot(x, y);
      else queue_launch_shot(x, y, seconds);
    });
  }
  el('forecast-time').addEventListener('keydown', (event) => {
    if (event.key.toLowerCase() !== 'q' || event.isComposing || event.ctrlKey || event.altKey || event.metaKey) return;
    event.preventDefault();
    if (!event.repeat) queueCurrentShot();
  });
  // Secondary touches do not reliably generate clicks. Commit on release inside
  // Queue, preserving timeline focus and suppressing a possible synthesized click.
  let queuePointer, handledPointer;
  queue.addEventListener('pointerdown', (event) => {
    handledPointer = undefined;
    if (event.pointerType !== 'touch' || !timeline.dragging || queue.disabled || queuePointer !== undefined) return;
    event.preventDefault();
    queuePointer = event.pointerId;
    queue.setPointerCapture(queuePointer);
  });
  queue.addEventListener('pointerup', (event) => {
    if (event.pointerId !== queuePointer) return;
    handledPointer = queuePointer; queuePointer = undefined;
    const box = queue.getBoundingClientRect();
    if (event.clientX >= box.left && event.clientX <= box.right && event.clientY >= box.top && event.clientY <= box.bottom) queueCurrentShot();
  });
  queue.addEventListener('pointercancel', () => { queuePointer = undefined; });
  queue.addEventListener('lostpointercapture', () => { queuePointer = undefined; });
  queue.addEventListener('click', (event) => {
    if (event.pointerType === 'touch' && event.pointerId === handledPointer) return;
    queueCurrentShot();
  });
  el('cancel-shot').addEventListener('click', () => invoke(() => command('cancel_shot')));
  el('track-aim').addEventListener('click', () => { aimEditor.clear(); error = ''; render(); });

  new ResizeObserver(() => { if (state) render(); }).observe(svg.parentElement);
  return { get dragging() { return aimEditor.dragging; }, cancel() { aimEditor.cancel(); }, sync(value, isReady, isBlocked) {
    const reset = value.world.tick < lastTick || Boolean(state?.mission) !== Boolean(value.mission);
    state = value; ready = isReady; blocked = isBlocked;
    if (aimEditor.dragging && (reset || finished())) aimEditor.cancel();
    if (reset) { aimEditor.clear(); error = ''; timeline.reset(); }
    lastTick = state.world.tick;
    render();
  } };
}
