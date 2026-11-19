//! Shared SI-scale assumptions for the provisional railgun sandbox.

use serde::Serialize;

use crate::physics::{Ship, Vector2, World};

pub const LIGHT_SPEED: f64 = 299_792_458.0;
pub const RAILGUN_SPEED: f64 = 660_000.0;
pub const INITIAL_SEPARATION: f64 = 240_000_000.0;
pub const REFERENCE_FLIGHT_SECONDS: f64 = INITIAL_SEPARATION / RAILGUN_SPEED;
pub const SHIP_RADIUS: f64 = 500.0;
pub const STANDARD_GRAVITY: f64 = 9.80665;
pub const MAX_ACCELERATION: f64 = 5.0 * STANDARD_GRAVITY;
pub const INITIAL_DELTA_V: f64 = 60_000.0;
pub const FRIENDLY_SPEED: f64 = 75_000.0;
pub const CONTACT_SPEED: f64 = 37_500.0;

#[derive(Clone, Copy, Serialize)]
pub struct Scale {
    pub light_speed_mps: f64,
    pub railgun_speed_mps: f64,
    pub reference_flight_seconds: f64,
    pub reference_range_m: f64,
}

pub const SCALE: Scale = Scale {
    light_speed_mps: LIGHT_SPEED,
    railgun_speed_mps: RAILGUN_SPEED,
    reference_flight_seconds: REFERENCE_FLIGHT_SECONDS,
    reference_range_m: INITIAL_SEPARATION,
};

pub fn initial_world() -> World {
    World::new(vec![
        Ship::new(
            1,
            Vector2::new(-INITIAL_SEPARATION / 2.0, 0.0),
            Vector2::new(0.0, FRIENDLY_SPEED),
            SHIP_RADIUS,
            MAX_ACCELERATION,
            INITIAL_DELTA_V,
        )
        .expect("valid friendly scenario"),
        Ship::new(
            2,
            Vector2::new(INITIAL_SEPARATION / 2.0, 0.0),
            Vector2::new(0.0, -CONTACT_SPEED),
            SHIP_RADIUS,
            MAX_ACCELERATION,
            INITIAL_DELTA_V,
        )
        .expect("valid passive scenario"),
    ])
    .expect("unique scenario ship IDs")
}
