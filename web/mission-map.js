import { mapProjection } from './map-overlay.js';

const ns = 'http://www.w3.org/2000/svg';

// Render a supplied observation snapshot and aim preview; never issue gameplay commands.
export function drawMissionMap(svg, state, { aim, solution, fixedTarget, done }) {
  const el = (id) => svg.querySelector(`#${id}`);
  const handle = el('aim-handle'), m = state.mission;
  const view = mapProjection(svg, state.map), { project, path } = view;
  const [gx, gy] = project(m.goal);
  el('mission-goal').setAttribute('cx', gx); el('mission-goal').setAttribute('cy', gy);
  el('mission-goal').setAttribute('r', Math.max(3, m.goal_radius / view.metresPerPixel));
  el('goal-label').setAttribute('x', gx + 8); el('goal-label').setAttribute('y', gy + 4);
  el('enemy-prediction').setAttribute('d', path(m.prediction));
  const [px, py] = project(m.prediction[1]);
  el('prediction-label').setAttribute('text-anchor', px > view.width - 180 ? 'end' : 'start');
  el('prediction-label').setAttribute('x', px + (px > view.width - 180 ? -8 : 8));
  el('prediction-label').setAttribute('y', py + 16);
  el('aim-line').setAttribute('d', solution && !done ? path([solution.origin, solution.target]) : '');
  el('queued-shot-line').setAttribute('d', m.queued_solution && !done ? path([m.queued_solution.origin, m.queued_solution.target]) : '');
  handle.style.display = aim && !done ? '' : 'none';
  if (aim) {
    const [x, y] = project(aim);
    handle.setAttribute('transform', `translate(${x} ${y})`);
    handle.querySelector('text').setAttribute('text-anchor', x > view.width - 100 ? 'end' : 'start');
    handle.querySelector('text').setAttribute('x', x > view.width - 100 ? -14 : 14);
  }
  handle.querySelector('text').textContent = fixedTarget ? 'Aim · fixed' : 'Aim · lead';
  el('shot-trails').replaceChildren();
  for (const projectile of state.world.projectiles) {
    const shot = m.shots.find((shot) => shot.projectile_id === projectile.id);
    if (!shot) continue;
    const trail = document.createElementNS(ns, 'path');
    trail.setAttribute('d', path([shot.origin, projectile.position]));
    el('shot-trails').append(trail);
  }
}
