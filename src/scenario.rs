//! Shared ship and weapon tuning for the provisional blockade scenario.

use serde::Serialize;

use crate::physics::{Ship, Vector2, World};

use crate::units::METRES_PER_KILOMETRE;
pub use crate::units::{LIGHT_SPEED, STANDARD_GRAVITY};
pub const RAILGUN_SPEED: f64 = 660.0 * METRES_PER_KILOMETRE;
pub const INITIAL_SEPARATION: f64 = 240_000.0 * METRES_PER_KILOMETRE;
pub const REFERENCE_FLIGHT_SECONDS: f64 = INITIAL_SEPARATION / RAILGUN_SPEED;
pub const SHIP_RADIUS: f64 = 0.5 * METRES_PER_KILOMETRE;
pub const PROJECTILE_RADIUS: f64 = 1.0; // metres
pub const MAX_ACCELERATION: f64 = 5.0 * STANDARD_GRAVITY;
pub const INITIAL_DELTA_V: f64 = 60.0 * METRES_PER_KILOMETRE;
pub const FRIENDLY_SPEED: f64 = 75.0 * METRES_PER_KILOMETRE;
pub const CONTACT_SPEED: f64 = 37.5 * METRES_PER_KILOMETRE;

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

/// Provisional cover fixture, kept separate from the original open-space sandbox.
/// Adjust this single definition when comparing maneuver and firing strategies.
pub fn blockade_world() -> World {
    let mut world = initial_world();
    world.asteroids.push(default_asteroid());
    world.asteroids.push(enemy_cover());
    world
}

pub fn default_asteroid() -> crate::physics::Asteroid {
    crate::physics::Asteroid::new(
        1,
        Vector2::new(-100_000.0, -3_100.0) * METRES_PER_KILOMETRE,
        Vector2::new(0.0, 65.625) * METRES_PER_KILOMETRE,
        450.0 * METRES_PER_KILOMETRE,
    )
    .expect("valid provisional asteroid")
}

/// Smaller cover near the enemy-side center of the firing corridor.
pub fn enemy_cover() -> crate::physics::Asteroid {
    crate::physics::Asteroid::new(
        2,
        Vector2::new(100_000.0, -3_100.0) * METRES_PER_KILOMETRE,
        Vector2::new(0.0, -28.125) * METRES_PER_KILOMETRE,
        150.0 * METRES_PER_KILOMETRE,
    )
    .expect("valid enemy-side asteroid")
}
