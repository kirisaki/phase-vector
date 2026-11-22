import { ownShip } from './api.js';
import type { State, Mission } from './types.js';
import { element as el, query, required } from './dom.js';
import { RECENT_LAUNCHES } from './ui-settings.js';

interface PanelActions { onCancelShot: (id: number) => void; onRetry: () => void; onConfirmRetry: () => void; onCancelRetry: () => void }

const decimal = (n: number) => n.toFixed(1);
const runLabels = { standard: 'Standard', custom: 'Custom settings', debug: 'Debug run' };

// Render mission feedback and forward user actions; commands belong to the controller.
export function createMissionPanel({ onCancelShot, onRetry, onConfirmRetry, onCancelRetry }: PanelActions) {
  const result = el('mission-result');
  const confirmation = el('retry-confirmation');
  let shownOutcome: Mission['outcome'] | undefined;

  function renderResult(m: Mission) {
    if (m.outcome === 'running') { result.close(); shownOutcome = undefined; return; }
    const victory = m.outcome !== 'breakthrough';
    result.dataset.outcome = victory ? 'victory' : 'defeat';
    el('result-title').textContent = victory ? 'VICTORY' : 'DEFEAT';
    el('result-reason').textContent = { disabled: 'Enemy disabled by a direct hit.',
      timeout: 'Blockade successful. The enemy missed its deadline.',
      breakthrough: 'The enemy reached the exit before the deadline.' }[m.outcome];
    el('result-time').textContent = `${decimal(required(m.ended_seconds))} s`;
    const score = required(m.evaluation);
    el('result-shots').textContent = String(score.shots_fired);
    el('result-dv').textContent = `${decimal(score.spent_delta_v / 1000)} km/s`;
    el('result-score').textContent = String(score.total);
    el('result-run-kind').textContent = runLabels[m.run_kind];
    el('score-victory').textContent = String(score.victory_points);
    el('score-shots').textContent = String(score.shot_points);
    el('score-fuel').textContent = String(score.fuel_points);
    el('result-settings-note').textContent = m.run_kind === 'standard'
      ? 'Both victory types earn the same base points. Fewer shots and less Δv spent earn more.'
      : m.run_kind === 'custom' ? 'Custom starting conditions. Retry repeats these settings.'
      : 'Parameters changed during play. Retry restores the starting settings; this score is diagnostic.';
    if (shownOutcome !== m.outcome) {
      shownOutcome = m.outcome;
      result.showModal();
    }
  }

  function dismissResult() { result.close(); el('retry').focus({ preventScroll: true }); }
  function dismissDialog(dialog: HTMLDialogElement, dismiss: () => void) {
    const outside = (event: MouseEvent) => {
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

  function renderQueue(m: Mission, stepSeconds: number, done: boolean) {
    const list = el('shot-queue');
    const ids = new Set(m.queued.map((order) => String(order.id)));
    for (const node of list.querySelectorAll<HTMLElement>('[data-shot-id]')) {
      if (!ids.has(required(node.dataset.shotId))) node.remove();
    }
    el('empty-queue').hidden = Boolean(m.queued.length);
    m.queued.forEach((order, index) => {
      let row = list.querySelector<HTMLElement>(`[data-shot-id="${order.id}"]`);
      if (!row) {
        row = document.createElement('div'); row.dataset.shotId = String(order.id);
        row.className = 'queued-shot'; row.setAttribute('role', 'listitem');
        row.append(document.createElement('span'), document.createElement('button'));
        const button = query<HTMLButtonElement>('button', row); button.textContent = 'Cancel';
        button.setAttribute('aria-label', `Cancel shot ${order.id}`);
        button.addEventListener('click', () => onCancelShot(order.id));
      }
      // Keep nodes and focus stable across polling updates, including insertion into a freed slot.
      const sibling = list.querySelectorAll<HTMLElement>('[data-shot-id]')[index];
      if (sibling !== row) list.insertBefore(row, sibling ?? null);
      query('span', row).textContent = order.error ? `#${order.id} · ${order.error}` :
        `#${order.id} · fire ${decimal(order.launch_tick * stepSeconds)} s · arrival ${order.solution ? decimal(order.solution.arrival_seconds) + ' s' : 'unreachable'}${order.requested_arrival_seconds !== null ? ` · requested ${decimal(order.requested_arrival_seconds)} s` : ''}`;
      row.title = order.error || `Target (${decimal(order.target.x / 1000)}, ${decimal(order.target.y / 1000)}) km`;
      row.classList.toggle('invalid', Boolean(order.error));
      query<HTMLButtonElement>('button', row).disabled = done || order.locked;
      query<HTMLButtonElement>('button', row).title = order.locked ? 'Committed to the current command interval' : 'Cancel this reservation';
    });
  }

  function renderLaunches(m: Mission, stepSeconds: number) {
    const list = el('launch-list');
    const recent = m.shots.slice(-RECENT_LAUNCHES).reverse();
    const ids = new Set(recent.map((shot) => String(shot.order_id)));
    for (const row of [...list.querySelectorAll<HTMLElement>('[data-shot-id]')]) {
      if (!ids.has(required(row.dataset.shotId))) row.remove();
    }
    el('empty-history').hidden = Boolean(recent.length);
    recent.forEach((shot, index) => {
      let row = list.querySelector<HTMLElement>(`[data-shot-id="${shot.order_id}"]`);
      if (!row) {
        row = document.createElement('div');
        row.dataset.shotId = String(shot.order_id);
        row.setAttribute('role', 'listitem');
      }
      if (list.children[index] !== row) list.insertBefore(row, list.children[index] ?? null);
      row.textContent = `#${shot.order_id} · fired ${decimal(shot.launch_tick * stepSeconds)} s`;
      row.title = `Target passage ${decimal(shot.arrival_seconds)} s. Arrival at the target point does not guarantee a hit.`;
    });
  }
  el('retry').addEventListener('click', onRetry);
  el('result-retry').addEventListener('click', onRetry);

  return { confirmRetry() { confirmation.showModal(); }, render(state: State, ready: boolean) {
    const m = state?.mission;
    el('retry').disabled = !m || !ready;
    if (!m || !ready) { result.close(); confirmation.close(); shownOutcome = undefined; return; }
    const done = m.outcome !== 'running';
    const titles = { running: 'Prevent breakthrough', breakthrough: 'Defeat · Enemy reached the exit',
      disabled: 'Victory · Enemy disabled', timeout: 'Victory · Deadline exceeded' };
    el('mission-status').textContent = `${titles[m.outcome]}${m.run_kind === 'standard' ? '' : ` · ${runLabels[m.run_kind]}`}`;
    el('mission-status').dataset.outcome = m.outcome;
    const own = ownShip(state);
    renderResult(m);
    const clock = done ? `Ended ${decimal(required(m.ended_seconds))} s` : `${decimal(Math.max(0, m.deadline_seconds - state.time_seconds))} s left`;
    el('mission-metrics').textContent = `${clock} · ${m.shots.length} shots · Δv ${decimal(own.spent_dv / 1000)} km/s${m.evaluation ? ` · ${m.evaluation.total} pts` : ''}`;
    el('retry-reason').textContent = 'Current progress, queued shots, and draft orders will be discarded. ' +
      (m.run_kind === 'standard' ? 'Restart with standard settings.' : 'Restore the starting settings and discard any mid-play parameter changes.');
    renderQueue(m, state.timing.step_seconds, done);
    renderLaunches(m, state.timing.step_seconds);
  } };
}
