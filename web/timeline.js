import { forecast_at } from './pkg/phase_vector.js';

// The slider changes a read-only preview and the next shot's requested arrival.
// It never advances the simulation or rewrites existing reservations.
export function createTimeline(onChange) {
  const root = document.getElementById('timeline-controls');
  const slider = document.getElementById('forecast-time');
  const live = document.getElementById('forecast-live');
  const label = document.getElementById('forecast-label');
  let selected = null, forecast = null, error = '', key;
  function reset() { selected = null; forecast = null; error = ''; key = undefined; }
  slider.addEventListener('input', () => { selected = Number(slider.value); onChange(); });
  live.addEventListener('click', () => { reset(); onChange(); });
  return {
    reset,
    get seconds() { return selected; },
    get forecast() { return forecast; },
    get error() { return error; },
    sync(state, ready, editing) {
      const mission = state?.mission;
      root.hidden = !mission;
      if (!mission || !ready) { reset(); return; }
      if (selected !== null && state.time_seconds >= selected) reset();
      slider.min = state.time_seconds;
      slider.max = Math.min(mission.deadline_seconds, state.time_seconds + mission.planning_horizon_seconds);
      slider.step = state.timing.step_seconds;
      slider.value = selected ?? state.time_seconds;
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
        ? `Preview ${forecast.time_seconds.toFixed(2)} s · +${(forecast.time_seconds - state.time_seconds).toFixed(2)} s`
        : 'Live · drag to plan arrival');
      slider.setAttribute('aria-valuetext', forecast ? `${forecast.time_seconds.toFixed(2)} seconds, predicted positions` : 'Live; drag to preview future positions');
    },
  };
}
