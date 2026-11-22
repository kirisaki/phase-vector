import type { State, Vector2, ManeuverPreview } from './types.js';
import type { Projection } from './map-overlay.js';
import { element as el, query, required } from './dom.js';
import { readState, ownShip, previewArrival } from './api.js';
import { plan_arrival, set_route_editing } from './pkg/phase_vector.js';

import { mapProjection } from './map-overlay.js';
import { BURN, COMMAND_PULSE } from './ui-settings.js';

const number = new Intl.NumberFormat('en-US', { maximumFractionDigits: 3 });
const km = (value: number) => number.format(value / 1000);

export function createRouteEditor(onChange: (applied: boolean) => void) {
  const svg = el('route-editor');
  const handle = el('arrival-handle');
  let state: State, context: State, target: Vector2;
  let route: ManeuverPreview | null = null, fingerprint: string, pointer: number | undefined;
  let dragging = false, valid = false, ready = false, dirtyNumeric = false;
  let grabX = 0, grabY = 0;
  // Keep running gestures stable; rebase their control offset at command boundaries.
  const identity = (value: State) => JSON.stringify([value.order_start_seconds, value.planned_burn,
    value.command_number, value.active_burn]);
  let lastCommand: number | undefined, pulseStarted = -Infinity;
  function animatePulse(now: number) {
    const pulse = el('command-pulse');
    const age = (now - pulseStarted) / COMMAND_PULSE.durationMs;
    const active = state && age >= 0 && age < 1;
    pulse.style.opacity = active ? String((1 - age) * COMMAND_PULSE.opacity) : '0';
    if (active) {
      const view = projection();
      const ship = ownShip(state);
      const [x, y] = view.project(ship.position);
      pulse.setAttribute('cx', String(x)); pulse.setAttribute('cy', String(y));
      pulse.setAttribute('r', String(COMMAND_PULSE.startRadiusPixels + age * COMMAND_PULSE.growthPixels));
    }
    requestAnimationFrame(animatePulse);
  }
  requestAnimationFrame(animatePulse);

  function projection() {
    return mapProjection(svg, state.map);
  }

  function controls(view: Projection) {
    const source = dragging ? context : state;
    const maneuver = source.maneuver;
    const coast = maneuver.coast_endpoint;
    const ship = ownShip(state);
    const [cx, cy] = view.project(ship.position);
    // Size the control for the viewport, never its distance from an edge.
    // Panning clips the circle instead of changing the thrust-to-pixel ratio.
    const radius = Math.max(BURN.minRadiusPixels, Math.min(BURN.maxRadiusPixels,
      view.width / 2 - BURN.edgePaddingPixels, view.height / 2 - BURN.edgePaddingPixels));
    const metresPerPixel = Math.max(maneuver.reachable_radius, 1) / radius;
    return { coast, cx, cy, radius: maneuver.reachable_radius > 0 ? radius : 0,
      metresPerPixel, ship };
  }

  function render(error = '') {
    if (!state || !ready) {
      svg.setAttribute('hidden', '');
      for (const id of ['route-cost', 'route-remaining', 'route-velocity'] as const) el(id).textContent = '—';
      el('route-feedback').textContent = '';
      return;
    }
    svg.removeAttribute('hidden');
    const planning = state.phase === 'planning';
    svg.dataset.phase = state.phase;
    const candidate = dragging ? route : state.maneuver;
    const arrival = dragging ? target : required(candidate).endpoint;
    const coast = (dragging ? context : state).maneuver.coast_endpoint;
    const view = projection();
    const { project, path } = view;
    const control = controls(view);
    const { cx, cy } = control;
    const [actualX, actualY] = project(arrival);
    const x = cx + (arrival.x - coast.x) / control.metresPerPixel;
    const y = cy - (arrival.y - coast.y) / control.metresPerPixel;
    handle.setAttribute('transform', `translate(${x} ${y})`);
    const area = query<SVGCircleElement>('.reach-area', svg);
    area.setAttribute('cx', String(cx)); area.setAttribute('cy', String(cy));
    area.setAttribute('r', String(control.radius));

    area.style.display = finished() ? 'none' : '';
    handle.style.display = finished() ? 'none' : '';
    handle.tabIndex = finished() ? -1 : 0;
    handle.style.pointerEvents = area.style.pointerEvents = finished() ? 'none' : '';
    handle.setAttribute('aria-disabled', String(Boolean(finished())));
    // Leave the ship itself visible at zero thrust, while retaining the hit target.
    query<SVGCircleElement>('.handle-dot', handle).style.display = Math.hypot(x - cx, y - cy) > 2 ? '' : 'none';
    el('map-coast-route').setAttribute('d', dragging ? path([context.maneuver.points[0], coast]) : '');
    el('map-planned-route').setAttribute('d', candidate ? path(candidate.forecast.points) : '');
    svg.dataset.invalid = String(dragging && !valid);
    query<SVGTextElement>('text', handle).textContent = 'Burn';
    const markers = el('forecast-boundaries');
    markers.replaceChildren();
    for (const boundary of candidate?.forecast.boundaries ?? []) {
      const [mx, my] = project(boundary.position);
      const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
      group.setAttribute('transform', `translate(${mx} ${my})`);
      const tick = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      tick.setAttribute('d', 'M-3 0H3');
      const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      text.setAttribute('x', '8'); text.setAttribute('y', '3');
      text.textContent = `+${number.format(boundary.seconds_ahead)} s`;
      group.append(tick, text); markers.append(group);
    }
    const dx = x - cx, dy = y - cy, length = Math.hypot(dx, dy);
    const ux = length > 0 ? dx / length : 0, uy = length > 0 ? dy / length : 0;
    el('burn-vector').setAttribute('d', length > 2
      ? `M${cx} ${cy}L${x} ${y}M${x - ux * 7 - uy * 4} ${y - uy * 7 + ux * 4}L${x} ${y}L${x - ux * 7 + uy * 4} ${y - uy * 7 - ux * 4}` : '');
    const label = el('burn-label');
    label.setAttribute('x', String(Math.max(6, cx - control.radius)));
    label.setAttribute('y', String(Math.min(view.height - 6, cy + control.radius + 14)));
    label.textContent = `${planning ? 'ORDER' : 'NEXT ORDER'} / ${number.format(state.timing.interval_seconds)} s`;
    handle.setAttribute('aria-label', 'Drag to set burn. Arrow keys adjust; Shift for fine adjustment; Home to coast.');
    let arrow = '';
    if (candidate) {
      const v = candidate.arrival_velocity, length = Math.hypot(v.x, v.y);
      if (length > 0) {
        const dx = v.x / length, dy = -v.y / length;
        const ex = actualX + dx * 30, ey = actualY + dy * 30;
        arrow = `M${actualX} ${actualY}L${ex} ${ey}M${ex - dx * 7 - dy * 4} ${ey - dy * 7 + dx * 4}L${ex} ${ey}L${ex - dx * 7 + dy * 4} ${ey - dy * 7 - dx * 4}`;
      }
    }
    el('arrival-arrow').setAttribute('d', arrow);

    el('route-cost-label').textContent = 'NEXT Δv / km/s';
    el('route-cost').textContent = candidate ? km(candidate.delta_v) : '—';
    el('route-remaining').textContent = candidate ? km(candidate.remaining_dv) : '—';
    el('route-velocity').textContent = candidate ? `${km(candidate.arrival_velocity.x)}, ${km(candidate.arrival_velocity.y)}` : '—';
    el('route-feedback').dataset.invalid = String(dragging && !valid);
    el('route-feedback').textContent = (finished() ? 'Mission complete. Retry to plan another blockade.' : '') || error || (dirtyNumeric ? 'Apply edited numeric values to update the next order.' : '') || (
      !valid && dragging ? 'Unreachable: outside the acceleration and delta-v limits.' :
      `Drag Burn · Applies ${planning ? 'on Auto or Commit' : 'at ' + number.format(state.order_start_seconds) + ' s'} · Repeats every ${number.format(state.timing.interval_seconds)} s. ` +
      (planning ? 'Home: coast.' : `Active: ${number.format(state.active_burn.x)}, ${number.format(state.active_burn.y)} m/s².`));
  }

  const finished = () => Boolean(state?.mission && state.mission.outcome !== 'running');

  function begin() {
    if (!ready || !state || dragging || finished()) return false;
    context = state;
    fingerprint = identity(state);
    target = state.maneuver.endpoint;
    route = state.maneuver;
    valid = true; dragging = true;
    set_route_editing(true);
    onChange(false);
    return true;
  }

  function rebaseGesture(fresh: State) {
    if (identity(fresh) === fingerprint) return true;
    if (fresh.world.tick < context.world.tick || fresh.order_start_seconds < context.order_start_seconds ||
        JSON.stringify(fresh.planned_burn) !== JSON.stringify(context.planned_burn)) return false;
    // Keep the chosen direction and fraction of the reachable disk as time advances.
    const radius = context.maneuver.reachable_radius;
    const dx = radius > 0 ? (target.x - context.maneuver.coast_endpoint.x) / radius : 0;
    const dy = radius > 0 ? (target.y - context.maneuver.coast_endpoint.y) / radius : 0;
    state = fresh; context = fresh; fingerprint = identity(fresh);
    choose({ x: fresh.maneuver.coast_endpoint.x + dx * fresh.maneuver.reachable_radius,
      y: fresh.maneuver.coast_endpoint.y + dy * fresh.maneuver.reachable_radius });
    return true;
  }

  function finish(apply: boolean) {
    if (!dragging) return;
    const fresh = readState();
    let applied = false, error = '';
    if (apply && rebaseGesture(fresh) && valid) {
      try { plan_arrival(target.x, target.y); applied = true; }
      catch (failure) { error = String(failure); }
    }
    dragging = false;
    const captured = pointer;
    pointer = undefined;
    if (captured !== undefined && svg.hasPointerCapture(captured)) svg.releasePointerCapture(captured);
    set_route_editing(false);
    state = readState();
    onChange(applied);
    render(error);
  }

  function choose(position: Vector2) {
    const coast = context.maneuver.coast_endpoint;
    const dx = position.x - coast.x, dy = position.y - coast.y;
    const distance = Math.hypot(dx, dy);
    const radius = context.maneuver.reachable_radius;
    const fraction = distance > radius ? radius / distance : 1;
    target = { x: coast.x + dx * fraction, y: coast.y + dy * fraction };
    try {
      route = previewArrival(target.x, target.y);
      valid = true; render();
    } catch (error) { route = null; valid = false; render(String(error)); }
  }

  function fromPointer(event: PointerEvent) {
    const view = projection();
    const control = controls(view);
    // Keep the cursor direction outside the canvas; radial clamping happens in choose.
    choose({ x: control.coast.x + (event.clientX - view.box.left - grabX - control.cx) * control.metresPerPixel,
      y: control.coast.y - (event.clientY - view.box.top - grabY - control.cy) * control.metresPerPixel });
  }
  svg.addEventListener('pointerdown', (event) => {
    if (event.button !== 0 || pointer !== undefined || !begin()) return;
    pointer = event.pointerId;
    grabX = 0; grabY = 0;
    if (event.target instanceof Element && event.target.closest('#arrival-handle')) {
      const view = projection(), control = controls(view);
      grabX = event.clientX - view.box.left - control.cx - (target.x - control.coast.x) / control.metresPerPixel;
      grabY = event.clientY - view.box.top - control.cy + (target.y - control.coast.y) / control.metresPerPixel;
    }
    svg.setPointerCapture(pointer);
    handle.focus({ preventScroll: true });
    fromPointer(event);
    event.preventDefault();
  });
  svg.addEventListener('pointermove', (event) => { if (event.pointerId === pointer) fromPointer(event); });
  svg.addEventListener('pointerup', (event) => {
    if (event.pointerId !== pointer) return;
    fromPointer(event); finish(true);
  });
  svg.addEventListener('pointercancel', () => finish(false));
  svg.addEventListener('lostpointercapture', () => finish(false));
  window.addEventListener('blur', () => finish(false));
  document.addEventListener('visibilitychange', () => { if (document.hidden) finish(false); });
  handle.addEventListener('keydown', (event) => {
    const directions: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, 1], ArrowDown: [0, -1] };
    const delta = directions[event.key];
    if ((!delta && event.key !== 'Home') || !begin()) return;
    event.preventDefault();
    if (event.key === 'Home') choose(context.maneuver.coast_endpoint);
    else {
      const step = Math.max(context.maneuver.reachable_radius, 1) * (event.shiftKey ? BURN.fineKeyFraction : BURN.keyFraction);
      choose({ x: target.x + delta[0] * step, y: target.y + delta[1] * step });
    }
    finish(true);
  });
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && dragging) { event.preventDefault(); finish(false); }
  });
  new ResizeObserver(() => render()).observe(required(svg.parentElement));
  return {
    get dragging() { return dragging; },
    cancel() { finish(false); },
    sync(value: State, isReady: boolean, hasNumericEdits: boolean) {
      state = value; ready = isReady; dirtyNumeric = hasNumericEdits;
      const record = state.command_log.at(-1);
      const command = record?.start_tick;
      if (command !== lastCommand) {
        pulseStarted = command === undefined ? -Infinity : performance.now();
        lastCommand = command;
        svg.dataset.commandTick = command === undefined ? '' : String(command);
      }
      if (dragging && (finished() || !rebaseGesture(state))) finish(false);
      render();
    },
  };
}
