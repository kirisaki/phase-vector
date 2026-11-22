import { state_json, preview_arrival, preview_shot, preview_launch_shot, forecast_at } from './pkg/phase_vector.js';
import type { State, Ship, ManeuverPreview, Solution, Forecast } from './types.js';

// JSON is serialized by the local Rust engine. Keep assertions at this boundary;
// generated wasm-bindgen declarations check the function arguments and raw results.
export const readState = (): State => JSON.parse(state_json()) as State;
export const previewArrival = (x: number, y: number): ManeuverPreview => JSON.parse(preview_arrival(x, y)) as ManeuverPreview;
export const previewShot = (x: number, y: number): Solution => JSON.parse(preview_shot(x, y)) as Solution;
export const previewLaunchShot = (x: number, y: number, seconds: number): Solution => JSON.parse(preview_launch_shot(x, y, seconds)) as Solution;
export const forecastAt = (seconds: number): Forecast => JSON.parse(forecast_at(seconds)) as Forecast;

export function ownShip(state: State): Ship {
  const ship = state.world.ships.find(ship => ship.id === state.player_id);
  if (!ship || ship.remaining_dv === undefined || ship.spent_dv === undefined) {
    throw new Error('The normal-view state must include the friendly ship and its fuel.');
  }
  return ship as Ship;
}
