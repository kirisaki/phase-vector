import { forecast_at } from './pkg/phase_vector.js';

import { TIMELINE } from './ui-settings.js';
import { timeScale, snapTime } from './timeline-scale.js';

// The slider changes a read-only preview and the next shot's firing time.
// It never advances the simulation or rewrites existing reservations.
export function createTimeline(onChange) {
  const root = document.getElementById('timeline-controls');
  const slider = document.getElementById('forecast-time');
  root.style.setProperty('--timeline-thumb-size', `${TIMELINE.thumbPixels}px`);
  const live = document.getElementById('forecast-live');
  const label = document.getElementById('forecast-label');
  const axis = document.getElementById('forecast-axis');
  const ticks = document.getElementById('forecast-ticks');
  const readyMarks = document.getElementById('forecast-ready-marks');
  const readyLabel = document.getElementById('forecast-ready-label');
  const boundaryLabel = document.getElementById('forecast-boundary');
  const endLabel = document.getElementById('forecast-end');
  let selected = null, forecast = null, error = '', key, scale, step;
  let pointer, scaleKey, available = [];
  function clearSelection() { selected = null; forecast = null; error = ''; key = undefined; }
  function reset() { pointer = undefined; clearSelection(); }
  function select(seconds) {
    selected = Math.max(scale.now, Math.min(scale.end, Math.round(seconds / step) * step));
    onChange();
  }
  slider.addEventListener('pointerdown', (event) => {
    if (!slider.disabled && event.button === 0 && pointer === undefined) pointer = event.pointerId;
  });
  const release = () => { if (pointer !== undefined) { pointer = undefined; onChange(); } };
  // A second finger can tap Queue without releasing the finger holding the scale.
  window.addEventListener('pointerup', (event) => { if (event.pointerId === pointer) release(); });
  window.addEventListener('pointercancel', (event) => { if (event.pointerId === pointer) release(); });
  window.addEventListener('blur', release);
  slider.addEventListener('input', () => {
    if (!scale) return;
    const seconds = snapTime(scale, Number(slider.value), available, slider.clientWidth);
    select(seconds);
  });
  // Keyboard nudges use real simulation ticks and bypass magnetic snapping.
  slider.addEventListener('keydown', (event) => {
    if (!scale || slider.disabled) return;
    const current = selected ?? scale.now;
    const times = { ArrowLeft: current - step, ArrowDown: current - step,
      ArrowRight: current + step, ArrowUp: current + step,
      PageDown: current - scale.interval, PageUp: current + scale.interval,
      Home: scale.now, End: scale.end };
    if (!(event.key in times)) return;
    event.preventDefault();
    select(times[event.key]);
  });
  live.addEventListener('click', () => { reset(); onChange(); });
  return {
    reset,
    get dragging() { return pointer !== undefined; },
    get seconds() { return selected; },
    get forecast() { return forecast; },
    get error() { return error; },
    sync(state, ready, editing) {
      const mission = state?.mission;
      root.hidden = !mission;
      if (!mission || !ready) { reset(); return; }
      if (selected !== null && state.time_seconds >= selected) clearSelection();
      step = state.timing.step_seconds;
      // Keep the mapping fixed under the finger while Auto advances the world.
      if (pointer === undefined || !scale) scale = timeScale(state.time_seconds,
        Math.min(mission.deadline_seconds, state.time_seconds + mission.planning_horizon_seconds),
        state.timing.interval_seconds);
      slider.value = scale.position(selected ?? state.time_seconds);
      slider.setAttribute('aria-valuemin', scale.now);
      slider.setAttribute('aria-valuemax', scale.end);
      slider.setAttribute('aria-valuenow', selected ?? state.time_seconds);
      const nextScaleKey = JSON.stringify([scale.now, scale.end, scale.boundary]);
      if (nextScaleKey !== scaleKey) {
        scaleKey = nextScaleKey;
        axis.style.setProperty('--near-share', `${scale.split * 100}%`);
        boundaryLabel.style.left = `${scale.split * 100}%`;
        boundaryLabel.classList.toggle('at-end', scale.split === 1);
        boundaryLabel.textContent = `${scale.boundary.toFixed(0)} s`;
        endLabel.textContent = scale.split < 1 ? `${scale.end.toFixed(0)} s` : '';
        ticks.replaceChildren();
        for (let time = scale.boundary; time <= scale.end; time += scale.interval) {
          const tick = document.createElement('i');
          tick.style.left = `${scale.position(time) * 100}%`;
          ticks.append(tick);
        }
      }
      available = (mission.available_launch_seconds ?? []).filter(time => time >= scale.now && time <= scale.end);
      readyMarks.replaceChildren();
      for (const time of available) {
        const mark = document.createElement('i');
        mark.dataset.seconds = time;
        mark.style.left = `${scale.position(time) * 100}%`;
        readyMarks.append(mark);
      }
      const nextReady = available.find(time => time >= (selected ?? state.time_seconds)) ?? available.at(-1);
      readyLabel.hidden = nextReady === undefined;
      if (nextReady !== undefined) {
        const position = scale.position(nextReady);
        readyLabel.textContent = `Ready ${Number(nextReady.toFixed(2))} s`;
        readyLabel.style.left = `${position * 100}%`;
        readyLabel.style.transform = position < 0.2 ? 'none' : position > 0.8 ? 'translateX(-100%)' : 'translateX(-50%)';
      }
      slider.style.setProperty('--thumb-color', selected === null || selected <= scale.boundary ? 'var(--cyan)' : 'var(--amber)');
      slider.disabled = !ready || mission.outcome !== 'running' || editing;
      live.disabled = !ready || editing;
      live.setAttribute('aria-pressed', String(selected === null));
      const nextKey = JSON.stringify([selected, state.world.tick, state.planned_burn, state.active_burn, mission.queued, mission.outcome]);
      if (nextKey !== key) {
        key = nextKey; error = ''; forecast = null;
        if (selected !== null) {
          try { forecast = JSON.parse(forecast_at(selected)); }
          catch (e) { error = String(e); }
        }
      }
      label.textContent = error || (forecast
        ? `Fire ${forecast.time_seconds.toFixed(2)} s · +${(forecast.time_seconds - state.time_seconds).toFixed(2)} s`
        : 'Live · drag to plan firing');
      slider.setAttribute('aria-valuetext', forecast ? `${forecast.time_seconds.toFixed(2)} seconds firing time, predicted positions` : 'Live; drag to choose a firing time');
    },
  };
}
