import { query } from './dom.js';
import type { Asteroid, Forecast, MissionState, Solution, Vector2 } from './types.js';
import type { Projection } from './map-overlay.js';

const ns = 'http://www.w3.org/2000/svg';
// Known ephemerides only. Physical radius and symbolic center marks are distinct.
export function drawAsteroids(svg: SVGSVGElement, state: MissionState, view: Projection,
  forecast: Forecast | null, solution: Solution | null) {
  const live = query<SVGGElement>('#asteroid-map', svg);
  const future = query<SVGGElement>('#asteroid-forecast', svg);
  const contact = query<SVGGElement>('#cover-contact', svg);
  live.replaceChildren(); future.replaceChildren(); contact.replaceChildren();
  const marker = (group: SVGGElement, a: Asteroid, label: string) => {
    const node = document.createElementNS(ns, 'g');
    node.dataset.asteroidId = String(a.id);
    const [x, y] = view.project(a.position);
    node.setAttribute('transform', `translate(${x} ${y})`);
    const circle = document.createElementNS(ns, 'circle');
    circle.setAttribute('r', String(a.radius / view.metresPerPixel));
    const center = document.createElementNS(ns, 'path');
    center.setAttribute('d', 'M-4 0H4M0 -4V4');
    const text = document.createElementNS(ns, 'text');
    // Enemy-side labels face inward and sit below the ship's motion labels.
    const left = x > view.width / 2;
    text.setAttribute('text-anchor', left ? 'end' : 'start');
    text.setAttribute('x', left ? '-10' : '10');
    text.setAttribute('y', String(left ? group === future ? 48 : 18 : -10));
    text.textContent = label;
    node.append(circle, center, text); group.append(node);
  };
  for (const a of state.world.asteroids) {
    const end = { x: a.position.x + a.velocity.x * state.timing.forecast_seconds,
      y: a.position.y + a.velocity.y * state.timing.forecast_seconds };
    const route = document.createElementNS(ns, 'path');
    route.setAttribute('class', 'asteroid-route');
    route.setAttribute('d', view.path([a.position, end]));
    live.append(route);
    marker(live, a, `A${a.id} · ${(a.radius / 1000).toFixed(0)} km radius`);
  }
  for (const a of forecast?.asteroids ?? []) marker(future, a, `A${a.id} · ${forecast!.time_seconds.toFixed(0)} s`);
  const warn = (position: Vector2, label: string) => {
    const [x, y] = view.project(position);
    const node = document.createElementNS(ns, 'g');
    node.setAttribute('transform', `translate(${x} ${y})`);
    const cross = document.createElementNS(ns, 'path');
    cross.setAttribute('d', 'M-6 -6L6 6M-6 6L6 -6');
    const text = document.createElementNS(ns, 'text');
    text.setAttribute('x', '10'); text.setAttribute('y', '18'); text.textContent = label;
    node.append(cross, text); contact.append(node);
  };
  if (solution?.obstruction) warn(solution.obstruction.position, `Blocked · ${solution.obstruction.seconds.toFixed(1)} s`);
  const collision = forecast?.own_collision ?? state.maneuver.forecast.collision;
  if (collision) warn(collision.position, `Collision · ${collision.seconds.toFixed(1)} s`);
}
