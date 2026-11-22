import { RECENT_LAUNCHES } from './ui-settings.js';

const el = (id) => document.getElementById(id);
const decimal = (n) => n.toFixed(1);


// Render mission feedback and forward user actions; commands belong to the controller.
export function createMissionPanel({ onCancelShot, onRetry, onConfirmRetry, onCancelRetry }) {
  const result = el('mission-result');
  const confirmation = el('retry-confirmation');
  let shownOutcome;

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
  function dismissDialog(dialog, dismiss) {
    const outside = (event) => {
      const box = dialog.getBoundingClientRect();
      return event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom;
    };
    let pressedOutside = false;
    dialog.addEventListener('pointerdown', (event) => { pressedOutside = event.target === dialog && outside(event); });
    dialog.addEventListener('click', (event) => {
      if (pressedOutside && event.target === dialog && outside(event)) dismiss();
      pressedOutside = false;
    });
    dialog.addEventListener('cancel', (event) => { event.preventDefault(); dismiss(); });
  }
  dismissDialog(result, dismissResult);
  el('result-view').addEventListener('click', dismissResult);
  const cancelRetry = () => { confirmation.close(); onCancelRetry(); };
  dismissDialog(confirmation, cancelRetry);
  el('retry-cancel').addEventListener('click', cancelRetry);
  el('retry-confirm').addEventListener('click', () => { confirmation.close(); onConfirmRetry(); });

  function renderQueue(m, stepSeconds, done) {
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
        button.addEventListener('click', () => onCancelShot(order.id));
      }
      // Keep nodes and focus stable across polling updates, including insertion into a freed slot.
      const sibling = list.querySelectorAll('[data-shot-id]')[index];
      if (sibling !== row) list.insertBefore(row, sibling ?? null);
      row.querySelector('span').textContent = order.error ? `#${order.id} · ${order.error}` :
        `#${order.id} · fire ${decimal(order.launch_tick * stepSeconds)} s · arrival ${order.solution ? decimal(order.solution.arrival_seconds) + ' s' : 'unreachable'}${order.requested_arrival_seconds !== null ? ` · requested ${decimal(order.requested_arrival_seconds)} s` : ''}`;
      row.title = order.error || `Target (${decimal(order.target.x / 1000)}, ${decimal(order.target.y / 1000)}) km`;
      row.classList.toggle('invalid', Boolean(order.error));
      row.querySelector('button').disabled = done || order.locked;
      row.querySelector('button').title = order.locked ? 'Committed to the current command interval' : 'Cancel this reservation';
    });
  }

  function renderLaunches(m, stepSeconds) {
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
      row.textContent = `#${shot.order_id} · fired ${decimal(shot.launch_tick * stepSeconds)} s`;
      row.title = `Target passage ${decimal(shot.arrival_seconds)} s. Arrival at the target point does not guarantee a hit.`;
    });
  }
  el('retry').addEventListener('click', onRetry);
  el('result-retry').addEventListener('click', onRetry);

  return { confirmRetry() { confirmation.showModal(); }, render(state, ready) {
    const m = state?.mission;
    el('retry').disabled = !m || !ready;
    if (!m || !ready) { result.close(); confirmation.close(); shownOutcome = undefined; return; }
    const done = m.outcome !== 'running';
    const titles = { running: 'Prevent breakthrough', breakthrough: 'Defeat · Enemy reached the exit',
      disabled: 'Victory · Enemy disabled', timeout: 'Victory · Deadline exceeded' };
    el('mission-status').textContent = titles[m.outcome];
    el('mission-status').dataset.outcome = m.outcome;
    const own = state.world.ships.find((ship) => ship.id === state.player_id);
    renderResult(m, own);
    const clock = done ? `Ended ${decimal(m.ended_seconds)} s` : `${decimal(Math.max(0, m.deadline_seconds - state.time_seconds))} s left`;
    el('mission-metrics').textContent = `${clock} · ${m.shots.length} shots · Δv ${decimal(own.spent_dv / 1000)} km/s`;
    renderQueue(m, state.timing.step_seconds, done);
    renderLaunches(m, state.timing.step_seconds);
  } };
}
