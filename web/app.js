import init, { start, command, plan_burn, set_speed, run_lua, state_json, rendered_frames } from './pkg/phase_vector.js';

const element = (id) => document.getElementById(id);
const output = element('console-output');
const input = element('lua-input');
const speedButtons = document.querySelectorAll('[data-speed]');
let ready = false;
let dirtyPlan = false;
let lastPlan = '';

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

function refresh() {
  const state = JSON.parse(state_json());
  if (!ready && rendered_frames() > 2) {
    ready = true;
    element('engine-status').textContent = 'ENGINE ONLINE';
    document.body.dataset.ready = 'true';
    log('Lua ready. Inspect game.state(), plan with game.burn(ax, ay), then commit.', 'hint');
  }
  const planning = state.phase === 'planning';
  const plan = JSON.stringify(state.planned_burn);
  if (lastPlan !== plan) {
    element('burn-x').value = state.planned_burn.x;
    element('burn-y').value = state.planned_burn.y;
    dirtyPlan = false;
    lastPlan = plan;
  }
  element('mode').textContent = planning ? 'PLANNING' : state.paused ? 'PAUSED' : 'EXECUTING';
  element('ticks').textContent = state.world.tick;
  element('time').textContent = state.time_seconds.toFixed(2);
  element('command-number').textContent = `COMMAND ${String(state.command_number).padStart(2, '0')}`;
  element('window-time').textContent = `${planning ? 'Next window' : 'Remaining'}: ${state.remaining_seconds.toFixed(2)} s`;
  element('toggle').textContent = state.paused ? 'Resume' : 'Pause';
  element('toggle').disabled = !ready || planning;
  element('step').disabled = !ready || planning || !state.paused;
  element('commit').disabled = !ready || !planning || dirtyPlan;
  element('burn-fields').disabled = !ready || !planning;
  for (const button of speedButtons) {
    button.disabled = !ready;
    button.setAttribute('aria-pressed', String(Number(button.dataset.speed) === state.speed));
  }
  element('run').disabled = !ready;
  input.disabled = !ready;
  const ship = state.world.ships.find((ship) => ship.id === state.player_id);
  element('position').textContent = `${ship.position.x.toFixed(2)}, ${ship.position.y.toFixed(2)}`;
  element('velocity').textContent = `${ship.velocity.x.toFixed(3)}, ${ship.velocity.y.toFixed(3)}`;
  element('dv').textContent = ship.remaining_dv.toFixed(3);
  element('spent-dv').textContent = ship.spent_dv.toFixed(3);
  element('plan-feedback').textContent = !planning
    ? 'Orders are locked until this 120-second interval completes, including while paused.'
    : dirtyPlan ? 'Apply edited values before committing.'
    : `Planned a = (${state.planned_burn.x.toFixed(5)}, ${state.planned_burn.y.toFixed(5)}) m/s². Magnitude limit: ${ship.max_acceleration.toFixed(3)} m/s².`;
}

function invoke(action) {
  try { action(); }
  catch (error) { log(String(error), 'error-line'); showConsole(true, false); }
  refresh();
}

element('commit').addEventListener('click', () => invoke(() => command('commit')));
element('toggle').addEventListener('click', () => invoke(() => command(JSON.parse(state_json()).paused ? 'resume' : 'pause')));
element('step').addEventListener('click', () => invoke(() => command('step')));
for (const button of speedButtons) {
  button.addEventListener('click', () => invoke(() => set_speed(Number(button.dataset.speed))));
}
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
