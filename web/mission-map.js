import { mapProjection } from './map-overlay.js';

const ns = 'http://www.w3.org/2000/svg';

// Render a supplied observation snapshot and aim preview; never issue gameplay commands.
export function drawMissionMap(svg, state, { aim, solution, arrival, forecast, fixedTarget, done }) {
  const el = (id) => svg.querySelector(`#${id}`);
  const handle = el('aim-handle'), m = state.mission;
  const view = mapProjection(svg, state.map), { project, path } = view;
  svg.dataset.preview = String(Boolean(forecast));
  drawForecast(el('time-forecast'), view, forecast);
  const [gx, gy] = project(m.goal);
  el('mission-goal').setAttribute('cx', gx); el('mission-goal').setAttribute('cy', gy);
  el('mission-goal').setAttribute('r', Math.max(3, m.goal_radius / view.metresPerPixel));
  el('goal-label').setAttribute('x', gx + 8); el('goal-label').setAttribute('y', gy + 4);
  el('enemy-prediction').setAttribute('d', path(forecast ? [m.prediction[0], forecast.enemy_position, ...(arrival ? [arrival.position] : [])] : m.prediction));
  el('prediction-label').textContent = `Observed velocity · +${state.timing.forecast_seconds} s`;
  const [px, py] = project(m.prediction[1]);
  el('prediction-label').setAttribute('text-anchor', px > view.width - 180 ? 'end' : 'start');
  el('prediction-label').setAttribute('x', px + (px > view.width - 180 ? -8 : 8));
  el('prediction-label').setAttribute('y', py + 16);
  drawContact(el, state, view, done);
  el('aim-line').setAttribute('d', solution && !done ? path([solution.origin, solution.target]) : '');
  el('queued-shot-line').setAttribute('d', done ? '' : m.queued
    .filter((order) => order.solution)
    .map(({ solution }) => path([solution.origin, solution.target])).join(' '));
  el('queued-shot-markers').replaceChildren();
  if (!done) for (const order of m.queued) {
    if (forecast?.rounds.some((round) => round.order_id === order.id)) continue;
    const [x, y] = project(order.target);
    const marker = document.createElementNS(ns, 'text');
    marker.setAttribute('x', x + 8); marker.setAttribute('y', y - 10);
    marker.textContent = `#${order.id}`;
    el('queued-shot-markers').append(marker);
  }
  handle.style.display = aim && !done ? '' : 'none';
  if (aim) {
    const [x, y] = project(aim);
    handle.setAttribute('transform', `translate(${x} ${y})`);
    handle.querySelector('text').setAttribute('text-anchor', x > view.width - 100 ? 'end' : 'start');
    handle.querySelector('text').setAttribute('x', x > view.width - 100 ? -14 : 14);
    const gripX = Math.max(25, Math.min(view.width - 25, x - 48)) - x;
    const gripY = Math.max(25, Math.min(view.height - 38, y + 55)) - y;
    el('aim-grip').querySelector('.grip-link').setAttribute('d', `M0 0L${gripX} ${gripY}`);
    el('aim-grip').querySelector('.grip-control').setAttribute('transform', `translate(${gripX} ${gripY})`);
  }
  const arrivalContact = el('arrival-contact');
  arrivalContact.style.display = arrival && !done ? '' : 'none';
  el('prediction-label').style.display = (arrival && !done) || forecast ? 'none' : '';
  if (arrival && !done) {
    const [x, y] = project(arrival.position);
    arrivalContact.setAttribute('transform', `translate(${x} ${y})`);
    arrivalContact.querySelector('path').setAttribute('d', 'M0 -10L10 0L0 10L-10 0Z');
    const label = arrivalContact.querySelector('text');
    label.setAttribute('x', x > view.width - 130 ? -14 : 14);
    label.setAttribute('y', 22);
    label.setAttribute('text-anchor', x > view.width - 130 ? 'end' : 'start');
    label.textContent = `Enemy +${arrival.secondsFromNow.toFixed(0)} s · estimate`;
  }
  handle.querySelector('text').textContent = fixedTarget ? 'Aim · fixed' : 'Aim · lead';
  el('shot-paths').replaceChildren();
  el('shot-labels').replaceChildren();
  for (const projectile of state.world.projectiles) {
    const shot = m.shots.find((shot) => shot.projectile_id === projectile.id);
    if (!shot) continue;
    // Only the remaining ballistic segment is shown. Passing a target is not a hit.
    if (state.time_seconds < shot.arrival_seconds) {
      const remaining = document.createElementNS(ns, 'path');
      remaining.dataset.shotId = shot.order_id;
      remaining.setAttribute('d', path([projectile.position, shot.target]));
      el('shot-paths').append(remaining);
    }
    const [x, y] = project(projectile.position);
    const label = document.createElementNS(ns, 'text');
    label.dataset.shotId = shot.order_id;
    label.setAttribute('x', x + (x > view.width - 35 ? -8 : 8));
    label.setAttribute('y', y - 8);
    label.setAttribute('text-anchor', x > view.width - 35 ? 'end' : 'start');
    label.textContent = `#${shot.order_id}`;
    el('shot-labels').append(label);
  }
}

function drawForecast(group, view, forecast) {
  group.replaceChildren();
  if (!forecast) return;
  const route = document.createElementNS(ns, 'path');
  route.setAttribute('d', view.path(forecast.own_path));
  route.setAttribute('class', 'forecast-route');
  group.append(route);
  const marker = (position, velocity, kind, label, symbol) => {
    const node = document.createElementNS(ns, 'g');
    node.setAttribute('class', `forecast-${kind}`);
    const [x, y] = view.project(position);
    node.setAttribute('transform', `translate(${x} ${y})`);
    const shape = document.createElementNS(ns, 'path');
    shape.setAttribute('d', symbol);
    node.append(shape);
    if (velocity) {
      const speed = Math.hypot(velocity.x, velocity.y);
      if (speed) {
        const arrow = document.createElementNS(ns, 'path');
        const dx = velocity.x / speed, dy = -velocity.y / speed, length = Math.min(60, speed / 1000);
        const tip = 12 + length;
        arrow.setAttribute('d', `M${dx * 12} ${dy * 12}L${dx * tip} ${dy * tip}M${dx * (tip - 5) - dy * 3} ${dy * (tip - 5) + dx * 3}L${dx * tip} ${dy * tip}L${dx * (tip - 5) + dy * 3} ${dy * (tip - 5) - dx * 3}`);
        node.append(arrow);
      }
    }
    const text = document.createElementNS(ns, 'text');
    text.textContent = label;
    if (kind === 'enemy' && view.width < 600) text.style.display = 'none';
    text.setAttribute('x', x > view.width - 155 ? -14 : 14);
    text.setAttribute('y', kind === 'enemy' ? Math.min(34, view.height - y - 8)
      : kind === 'round' ? -30 : y < 30 ? 22 : -12);
    text.setAttribute('text-anchor', x > view.width - 155 ? 'end' : 'start');
    node.append(text); group.append(node);
    return node;
  };
  const speed = Math.hypot(forecast.own.velocity.x, forecast.own.velocity.y) / 1000;
  marker(forecast.own.position, forecast.own.velocity, 'own', `${view.width < 600 ? 'Own' : 'Own preview'} · ${speed.toFixed(1)} km/s`, 'M0 -10L9 8H-9Z');
  marker(forecast.enemy_position, forecast.enemy_velocity, 'enemy', `Enemy at fire · ${forecast.time_seconds.toFixed(0)} s`, 'M0 -10L10 0L0 10L-10 0Z');
  for (const round of forecast.rounds) {
    marker(round.position, null, 'round', `#${round.order_id}`, 'M-4 -4L4 4M-4 4L4 -4').dataset.shotId = round.order_id;
  }
}

function drawContact(el, state, view, done) {
  const m = state.mission, velocity = m.contact.velocity;
  const [x, y] = view.project(m.prediction[0]);
  const speed = Math.hypot(velocity.x, velocity.y);
  const direction = speed > 0 ? [velocity.x / speed, -velocity.y / speed] : [1, 0];
  const arrow = (dx, dy, length) => {
    const end = 14 + length;
    return `M${dx * 14} ${dy * 14}L${dx * end} ${dy * end}M${dx * (end - 5) - dy * 3} ${dy * (end - 5) + dx * 3}L${dx * end} ${dy * end}L${dx * (end - 5) + dy * 3} ${dy * (end - 5) - dx * 3}`;
  };
  el('enemy-contact').setAttribute('transform', `translate(${x} ${y})`);
  // Orientation shows observed travel direction, not an unknown hull attitude.
  el('enemy-symbol').setAttribute('transform', `rotate(${Math.atan2(direction[1], direction[0]) * 180 / Math.PI})`);
  el('enemy-velocity').setAttribute('d', speed > 0 ? arrow(...direction, Math.min(72, speed / 1000)) : '');
  const a = m.observed_acceleration;
  const magnitude = a ? Math.hypot(a.x, a.y) : 0;
  // Ignore sub-display precision changes; this threshold is not an AI threat rule.
  const changing = magnitude >= 0.1 && !done;
  const along = a && speed > 0 ? (a.x * velocity.x + a.y * velocity.y) / speed : 0;
  const motion = done ? 'last' : !a ? 'unknown' : !changing ? 'coasting' : along < -0.1 ? 'slowing' : along > 0.1 ? 'speeding' : 'turning';
  const labels = { last: 'Last observation', unknown: 'Awaiting track', coasting: 'Coasting · observed',
    slowing: 'Slowing · observed', speeding: 'Speeding up · observed', turning: 'Turning · observed' };
  const contact = state.world.ships.find((ship) => ship.id === m.contact.id);
  el('enemy-acceleration').setAttribute('d', changing
    ? arrow(a.x / magnitude, -a.y / magnitude, 14 + 20 * Math.min(1, magnitude / Math.max(1, contact.max_acceleration))) : '');
  el('enemy-contact').dataset.motion = motion;
  const speedLabel = `Enemy · ${(speed / 1000).toFixed(1)} km/s`;
  el('enemy-speed').textContent = speedLabel;
  el('enemy-motion').textContent = labels[motion];
  el('enemy-contact').setAttribute('aria-label', `${speedLabel}. ${labels[motion]}. Contact age ${m.contact_age.toFixed(1)} seconds.`);
  const left = x > view.width - 155;
  for (const [id, offset] of [['enemy-speed', -20], ['enemy-motion', -7]]) {
    el(id).setAttribute('x', left ? -18 : 18);
    el(id).setAttribute('y', offset);
    el(id).setAttribute('text-anchor', left ? 'end' : 'start');
  }
}
