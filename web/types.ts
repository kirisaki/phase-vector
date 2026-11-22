// Normal-view JSON contracts from presentation.rs, planning.rs, and maneuver.rs.
// These describe trusted local WASM output, not arbitrary external input.
export interface Vector2 { x: number; y: number }
export interface VisibleShip {
  id: number; position: Vector2; velocity: Vector2; radius: number;
  max_acceleration: number; remaining_dv?: number; spent_dv?: number;
}
export interface Ship extends VisibleShip { remaining_dv: number; spent_dv: number }
export interface Projectile { id: number; position: Vector2; velocity: Vector2; radius: number }
export interface Solution {
  origin: Vector2; target: Vector2; direction: Vector2; flight_seconds: number;
  launch_tick: number; arrival_seconds: number;
}
export interface Shot {
  projectile_id: number; order_id: number; launch_tick: number; arrival_seconds: number;
  origin: Vector2; target: Vector2; velocity: Vector2;
}
export interface QueuedShot {
  id: number; target: Vector2; launch_tick: number; requested_arrival_seconds: number | null;
  planning_error: string | null; locked: boolean; solution: Solution | null; error: string | null;
}
export type Outcome = 'running' | 'breakthrough' | 'disabled' | 'timeout';
export interface Evaluation {
  total: number; victory_points: number; shot_points: number; fuel_points: number;
  shots_fired: number; spent_delta_v: number;
}
export interface Track {
  id: number; position: Vector2; velocity: Vector2; radius: number;
  sampled_tick: number; received_tick: number;
}
export interface Mission {
  run_kind: 'standard' | 'custom' | 'debug'; evaluation: Evaluation | null;
  outcome: Outcome; ended_seconds: number | null; goal: Vector2; goal_radius: number;
  deadline_seconds: number; ready_seconds: number; reload_seconds: number;
  available_launch_seconds: number[]; shots: Shot[]; queued: QueuedShot[];
  queue_capacity: number; planning_horizon_seconds: number; arrival_tolerance_seconds: number;
  queue_error: string | null; aim: Solution | null; contact: Track; contact_age: number;
  observed_acceleration: Vector2 | null; prediction: [Vector2, Vector2];
}
export interface MapFrame {
  focus: 'overview' | 'friendly' | 'contact' | 'free'; auto_zoom: boolean;
  center: Vector2; metres_per_unit: number; grid_metres: number;
  can_zoom_in: boolean; can_zoom_out: boolean;
}
export interface ManeuverPreview {
  coast_endpoint: Vector2; endpoint: Vector2; arrival_velocity: Vector2;
  delta_v: number; remaining_dv: number; reachable_radius: number; points: Vector2[];
  // Snapshot::planned_route always attaches the multi-interval forecast.
  forecast: { points: Vector2[]; boundaries: {
    seconds_ahead: number; position: Vector2; velocity: Vector2; remaining_dv: number;
  }[] };
}
export interface Forecast {
  time_seconds: number; own: Ship; own_path: Vector2[]; enemy_position: Vector2;
  enemy_velocity: Vector2; aim: Solution | null;
  rounds: { order_id: number; position: Vector2; velocity: Vector2; reserved: boolean }[];
}
export interface State {
  world: { tick: number; ships: VisibleShip[]; projectiles: Projectile[] };
  mission: Mission | null;
  scale: { light_speed_mps: number; railgun_speed_mps: number; reference_flight_seconds: number; reference_range_m: number };
  timing: {
    step_seconds: number; interval_ticks: number; interval_seconds: number;
    step_forward_ticks: number; step_forward_seconds: number; forecast_intervals: number; forecast_seconds: number;
  };
  player_id: number; phase: 'planning' | 'executing'; paused: boolean; stop_at_boundary: boolean; speed: number;
  planned_burn: Vector2; active_burn: Vector2; time_seconds: number; remaining_seconds: number;
  order_start_seconds: number; command_number: number; command_log: { start_tick: number }[];
  maneuver: ManeuverPreview; map: MapFrame;
}
export type MissionState = State & { mission: Mission };
export interface ArrivalEstimate { position: Vector2; secondsFromNow: number; offsetMetres: number }
