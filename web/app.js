import init, { start, command, run_lua, state_json, rendered_frames } from './pkg/phase_vector.js';

const output = document.querySelector('#console-output');
const input = document.querySelector('#lua-input');
const toggle = document.querySelector('#toggle');
const step = document.querySelector('#step');
const reset = document.querySelector('#reset');
const run = document.querySelector('#run');
const status = document.querySelector('#engine-status');
let ready = false;

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
  document.querySelector('#mode').textContent = state.paused ? 'PAUSED' : 'RUNNING';
  document.querySelector('#ticks').textContent = state.ticks;
  toggle.textContent = state.paused ? 'Resume' : 'Pause';
  step.disabled = !ready || !state.paused;
  if (!ready && rendered_frames() > 2) {
    ready = true;
    status.textContent = 'ENGINE ONLINE';
    document.body.dataset.ready = 'true';
    for (const control of [toggle, step, reset, run, input]) control.disabled = false;
    log('Lua ready. Try return 1 + 2 or game.state().ticks.', 'hint');
  }
}

function send(name) {
  try { command(name); refresh(); }
  catch (error) { log(String(error), 'error-line'); }
}

toggle.addEventListener('click', () => send(JSON.parse(state_json()).paused ? 'resume' : 'pause'));
step.addEventListener('click', () => send('step'));
reset.addEventListener('click', () => send('reset'));
document.querySelector('#repl-form').addEventListener('submit', (event) => {
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
    document.querySelector('#repl-form').requestSubmit();
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
  status.textContent = 'ENGINE UNAVAILABLE';
  const error = document.querySelector('#error');
  error.textContent = message;
  error.hidden = false;
}
