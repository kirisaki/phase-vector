import init, { start, command, plan_burn, set_speed, set_map_view, zoom_map, run_lua, state_json, rendered_frames } from './pkg/phase_vector.js';
import { createRouteEditor } from './route-editor.js';
import { createMissionControls } from './mission.js';
import { createMapNavigation } from './map-navigation.js';

const element = (id) => document.getElementById(id);
const output = element('console-output');
const input = element('lua-input');
const speedButtons = document.querySelectorAll('[data-speed]');
const viewButtons = document.querySelectorAll('[data-view]');
const kmFormat = new Intl.NumberFormat('en-US', { maximumFractionDigits: 3 });
const km = (metres) => kmFormat.format(metres / 1000);

let ready = false;
let dirtyPlan = false;
let lastPlan = '';
const routeEditor = createRouteEditor((applied) => {
  if (applied) { dirtyPlan = false; lastPlan = ''; }
  refresh();
});

const missionControls = createMissionControls(() => refresh(), () => {
  routeEditor.cancel();
  mapNavigation.cancel();
  dirtyPlan = false;
  lastPlan = '';
});
const mapNavigation = createMapNavigation({
  onChange: () => refresh(),
  onError: (error) => invoke(() => { throw error; }),
  cancelEditors: () => { routeEditor.cancel(); missionControls.cancel(); },
});

const narrowScreen = matchMedia('(max-width: 900px)');

function syncConsoleLayout() {
  const overlay = narrowScreen.matches || document.documentElement.classList.contains('short-viewport');
  document.querySelector('.map-panel').inert = !element('console-panel').hidden && overlay;
}

function fitViewport() {
  const viewport = window.visualViewport;
  // Preserve browser zoom; only fit the keyboard/browser-chrome viewport at 1x.
  if (viewport && Math.abs(viewport.scale - 1) > 0.01) return;
  const height = viewport?.height ?? innerHeight;
  document.documentElement.style.setProperty('--app-height', `${height}px`);
  document.documentElement.style.setProperty('--app-top', `${viewport?.offsetTop ?? 0}px`);
  document.documentElement.classList.toggle('short-viewport', height < 500);
  syncConsoleLayout();
}

window.addEventListener('resize', fitViewport);
window.visualViewport?.addEventListener('resize', fitViewport);
window.visualViewport?.addEventListener('scroll', fitViewport);
narrowScreen.addEventListener('change', syncConsoleLayout);
fitViewport();

function showConsole(open, moveFocus = true) {
  element('console-panel').hidden = !open;
  document.querySelector('.workspace').classList.toggle('console-open', open);
  element('console-toggle').setAttribute('aria-expanded', String(open));
  element('console-toggle').textContent = open ? 'Hide Lua console' : 'Show Lua console';
  syncConsoleLayout();
  if (moveFocus) {
    (open ? (input.disabled ? element('console-panel') : input) : element('console-toggle')).focus();
  }
}

element('console-toggle').addEventListener('click', () => showConsole(element('console-panel').hidden));
element('console-close').addEventListener('click', () => showConsole(false));
element('console-panel').addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    event.preventDefault();
    showConsole(false);
  }
});

function log(text, className) {
  const line = document.createElement('p');
  line.className = className;
  line.textContent = text;
  output.append(line);
  while (output.children.length > 100) output.firstElementChild.remove();
  output.scrollTop = output.scrollHeight;
}

function setVector(id, x, y) {
  const values = element(id).children;
  values[0].textContent = `${x}, `;
  values[1].textContent = y;
}

function refresh() {
  const state = JSON.parse(state_json());
  if (!ready && rendered_frames() > 2) {
    ready = true;
    element('engine-status').textContent = 'ENGINE ONLINE';
    document.body.dataset.ready = 'true';
    log(`Lua ready. Inspect game.state(), plan with game.burn(ax, ay), then press Auto. Orders repeat every ${state.timing.interval_seconds} seconds.`, 'hint');
  }
  element('step').textContent = `+${state.timing.step_forward_seconds} s`;
  element('step').setAttribute('aria-label', `Advance up to ${state.timing.step_forward_seconds} seconds`);
  element('next-turn').title = `Skip to the next ${state.timing.interval_seconds}-second turn boundary and pause`;
  element('commit').title = `Confirm orders and play at ${state.speed}× to the next ${state.timing.interval_seconds}-second turn boundary, then pause`;
  element('commit').setAttribute('aria-pressed', String(state.stop_at_boundary));
  element('burn-fields').querySelector('legend').textContent = `Constant acceleration for the next ${state.timing.interval_seconds} s`;
  const planning = state.phase === 'planning';
  const plan = JSON.stringify(state.planned_burn);
  if (lastPlan !== plan) {
    element('burn-x').value = state.planned_burn.x;
    element('burn-y').value = state.planned_burn.y;
    dirtyPlan = false;
    lastPlan = plan;
  }
  const finished = state.mission && state.mission.outcome !== 'running';
  mapNavigation.sync(state, ready);
  routeEditor.sync(state, ready, dirtyPlan);
  missionControls.sync(state, ready, dirtyPlan || routeEditor.dragging);
  element('mode').textContent = finished ? 'COMPLETE' : state.paused ? 'PAUSED' : 'EXECUTING';
  element('ticks').textContent = state.world.tick;
  element('time').textContent = state.time_seconds.toFixed(2);
  element('command-number').textContent = `COMMAND ${String(state.command_number).padStart(2, '0')}`;
  element('window-time').textContent = finished ? 'Mission complete' : state.stop_at_boundary ? `Auto-pause: ${state.remaining_seconds.toFixed(2)} s` : `Next order: ${planning ? 'now' : state.remaining_seconds.toFixed(2) + ' s'}`;
  element('toggle').textContent = state.paused ? 'Auto' : 'Pause';
  element('toggle').title = state.paused ? 'Play continuously across turns until paused or the mission ends' : 'Pause playback';
  element('toggle').disabled = !ready || finished || (state.paused && (dirtyPlan || routeEditor.dragging || missionControls.dragging));
  element('step').disabled = !ready || finished || !state.paused || dirtyPlan || routeEditor.dragging || missionControls.dragging;
  element('commit').disabled = !ready || finished || dirtyPlan || routeEditor.dragging || missionControls.dragging;
  element('next-turn').disabled = !ready || finished || dirtyPlan || routeEditor.dragging || missionControls.dragging;
  element('burn-fields').disabled = !ready || finished;
  for (const button of speedButtons) {
    button.disabled = !ready;
    button.setAttribute('aria-pressed', String(Number(button.dataset.speed) === state.speed));
  }
  for (const button of viewButtons) {
    button.disabled = !ready || mapNavigation.active;
    button.setAttribute('aria-pressed', String(button.dataset.view === state.map.focus));
    if (button.dataset.view === 'overview') {
      button.textContent = state.map.auto_zoom ? 'Overview · Auto' : 'Overview';
    }
  }
  element('zoom-in').disabled = !ready || mapNavigation.active || !state.map.can_zoom_in;
  element('zoom-out').disabled = !ready || mapNavigation.active || !state.map.can_zoom_out;
  element('grid-label').textContent = `GRID / ${km(state.map.grid_metres)} km`;
  const other = state.world.ships.find((ship) => ship.id !== state.player_id);
  const own = state.world.ships.find((ship) => ship.id === state.player_id);
  const range = Math.hypot(other.position.x - own.position.x, other.position.y - own.position.y);
  element('range-label').textContent = `RANGE / ${km(range)} km`;
  element('railgun-label').textContent = `RAILGUN / ${km(state.scale.railgun_speed_mps)} km/s · reference flight ${state.scale.reference_flight_seconds.toFixed(1)} s`;
  element('run').disabled = !ready;
  input.disabled = !ready;
  const ship = state.world.ships.find((ship) => ship.id === state.player_id);
  setVector('position', km(ship.position.x), km(ship.position.y));
  setVector('velocity', (ship.velocity.x / 1000).toFixed(3), (ship.velocity.y / 1000).toFixed(3));
  element('dv').textContent = (ship.remaining_dv / 1000).toFixed(3);
  element('spent-dv').textContent = (ship.spent_dv / 1000).toFixed(3);

}

function invoke(action) {
  try { action(); }
  catch (error) { log(String(error), 'error-line'); showConsole(true, false); }
  refresh();
}

element('toggle').addEventListener('click', () => invoke(() => command(JSON.parse(state_json()).paused ? 'play' : 'pause')));
element('step').addEventListener('click', () => invoke(() => command('step_forward')));
element('commit').addEventListener('click', () => invoke(() => command('commit')));
element('next-turn').addEventListener('click', () => invoke(() => command('skip')));
for (const button of speedButtons) {
  button.addEventListener('click', () => invoke(() => set_speed(Number(button.dataset.speed))));
}
for (const button of viewButtons) {
  button.addEventListener('click', () => invoke(() => {
    const box = element('tactical-map').getBoundingClientRect();
    set_map_view(button.dataset.view, box.width / Math.max(box.height, 1));
  }));
}
element('zoom-in').addEventListener('click', () => invoke(() => zoom_map(0.5)));
element('zoom-out').addEventListener('click', () => invoke(() => zoom_map(2)));
for (const id of ['burn-x', 'burn-y']) {
  element(id).addEventListener('input', () => { dirtyPlan = true; refresh(); });
}
element('burn-form').addEventListener('submit', (event) => {
  event.preventDefault();
  if (!ready) return;
  invoke(() => {
    plan_burn(element('burn-x').valueAsNumber, element('burn-y').valueAsNumber);
    dirtyPlan = false;
    lastPlan = '';
  });
});
element('repl-form').addEventListener('submit', (event) => {
  event.preventDefault();
  if (!ready || !input.value.trim()) return;
  log(`> ${input.value}`, 'input-line');
  try { log(run_lua(input.value), 'result'); }
  catch (error) { log(String(error), 'error-line'); }
  refresh();
});
input.addEventListener('keydown', (event) => {
  if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    element('repl-form').requestSubmit();
  }
});
document.querySelectorAll('[data-example]').forEach((button) => {
  button.addEventListener('click', () => { input.value = button.dataset.example; input.focus(); });
});

try {
  await init();
  if (new URLSearchParams(location.search).get('sandbox') === '1') command('sandbox');
  start();
  const started = performance.now();
  const poll = setInterval(() => {
    refresh();
    if (!ready && performance.now() - started > 30000) {
      clearInterval(poll);
      fail('The renderer did not start. Check that WebGL2 is enabled, then reload.');
    }
  }, 100);
} catch (error) {
  fail(`The application could not start: ${error}`);
}

function fail(message) {
  element('engine-status').textContent = 'ENGINE UNAVAILABLE';
  element('error').textContent = message;
  element('error').hidden = false;
}
