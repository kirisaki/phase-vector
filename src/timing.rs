//! Shared simulation clock and command timing. Durations here are whole seconds.

use serde::Serialize;

pub const TICKS_PER_SECOND: u32 = 4;
pub const STEP_SECONDS: f64 = 1.0 / TICKS_PER_SECOND as f64;
pub const PLAYBACK_HZ: u32 = 60;
// Integer playback quanta must represent every physical tick exactly.
const _: () = assert!(PLAYBACK_HZ.is_multiple_of(TICKS_PER_SECOND));
pub const PULSES_PER_TICK: u32 = PLAYBACK_HZ / TICKS_PER_SECOND;

pub const fn seconds_to_ticks(seconds: u64) -> u64 {
    seconds * TICKS_PER_SECOND as u64
}

pub const INTERVAL_TICKS: u64 = seconds_to_ticks(120);
pub const INTERVAL_SECONDS: f64 = INTERVAL_TICKS as f64 * STEP_SECONDS;
pub const STEP_FORWARD_TICKS: u64 = seconds_to_ticks(10);
pub const STEP_FORWARD_SECONDS: f64 = STEP_FORWARD_TICKS as f64 * STEP_SECONDS;
pub const FORECAST_INTERVALS: usize = 3;
pub const FORECAST_SECONDS: f64 = INTERVAL_SECONDS * FORECAST_INTERVALS as f64;

/// The UI and Lua read these values instead of maintaining another set of defaults.
#[derive(Clone, Copy, Serialize)]
pub struct Timing {
    pub step_seconds: f64,
    pub interval_ticks: u64,
    pub interval_seconds: f64,
    pub step_forward_ticks: u64,
    pub step_forward_seconds: f64,
    pub forecast_intervals: usize,
    pub forecast_seconds: f64,
}
pub const SETTINGS: Timing = Timing {
    step_seconds: STEP_SECONDS,
    interval_ticks: INTERVAL_TICKS,
    interval_seconds: INTERVAL_SECONDS,
    step_forward_ticks: STEP_FORWARD_TICKS,
    step_forward_seconds: STEP_FORWARD_SECONDS,
    forecast_intervals: FORECAST_INTERVALS,
    forecast_seconds: FORECAST_SECONDS,
};
