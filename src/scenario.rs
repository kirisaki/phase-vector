//! Shared SI-scale assumptions for the near-light-speed railgun sandbox.

use serde::Serialize;

use crate::physics::{Ship, Vector2, World};

pub const LIGHT_SPEED: f64 = 299_792_458.0;
pub const RAILGUN_SPEED: f64 = 0.9 * LIGHT_SPEED;
pub const REFERENCE_FLIGHT_SECONDS: f64 = 360.0;
pub const INITIAL_SEPARATION: f64 = RAILGUN_SPEED * REFERENCE_FLIGHT_SECONDS;
pub const SHIP_RADIUS: f64 = 500.0;
pub const MAX_ACCELERATION: f64 = 1_000.0;
pub const INITIAL_DELTA_V: f64 = 1_200_000.0;
pub const FRIENDLY_SPEED: f64 = 0.1 * LIGHT_SPEED;
pub const CONTACT_SPEED: f64 = 0.05 * LIGHT_SPEED;

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
