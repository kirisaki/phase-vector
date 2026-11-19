//! Single-interval maneuver previews and inverse endpoint planning in SI units.

use serde::Serialize;

use crate::{
    physics::{STEP_SECONDS, Ship, Vector2},
    session::INTERVAL_TICKS,
};

#[derive(Serialize)]
pub struct Preview {
    pub coast_endpoint: Vector2,
    pub endpoint: Vector2,
    pub arrival_velocity: Vector2,
    pub delta_v: f64,
    pub remaining_dv: f64,
    pub reachable_radius: f64,
    pub points: Vec<Vector2>,
    pub after_coast: Vector2,
}

fn displacement(ship: &Ship, acceleration: f64, ticks: u64) -> f64 {
    // Evaluate in a local, initially stationary frame to retain endpoint precision.
    let mut remaining = ship.remaining_dv;
    let mut velocity = 0.0;
    let mut position = 0.0;
    for _ in 0..ticks {
        let dv = (acceleration * STEP_SECONDS).min(remaining);
        velocity += dv;
        remaining = (remaining - dv).max(0.0);
        position += velocity * STEP_SECONDS;
    }
    position
}

pub fn preview(ship: &Ship, acceleration: Vector2) -> Preview {
    preview_ticks(ship, acceleration, INTERVAL_TICKS)
}

/// Predict the remaining execution ticks using exactly the same integration as playback.
pub fn preview_ticks(ship: &Ship, acceleration: Vector2, ticks: u64) -> Preview {
    let mut coast = ship.clone();
    let mut planned = ship.clone();
    let mut points = vec![ship.position];
    for tick in 1..=ticks {
        coast.advance(Vector2::ZERO);
        planned.advance(acceleration);
        if tick % 8 == 0 || tick == ticks {
            points.push(planned.position);
        }
    }
    Preview {
        coast_endpoint: coast.position,
        endpoint: planned.position,
        arrival_velocity: planned.velocity,
        delta_v: ship.remaining_dv - planned.remaining_dv,
        remaining_dv: planned.remaining_dv,
        reachable_radius: displacement(ship, ship.max_acceleration, ticks),
        points,
        after_coast: planned.position + planned.velocity * 30.0,
    }
}

/// Solve a constant thrust vector, including the final partial burn on fuel exhaustion.
/// Reject unreachable endpoints instead of silently clamping them to a different route.
pub fn solve(ship: &Ship, target: Vector2) -> Result<Vector2, String> {
    if !target.valid() {
        return Err("Arrival position must be finite and within bounds.".into());
    }
    let context = preview(ship, Vector2::ZERO);
    let offset = target - context.coast_endpoint;
    let distance = offset.length();
    if distance > context.reachable_radius + 0.001 {
        return Err("Unreachable: outside the acceleration and delta-v limits.".into());
    }
    if distance <= 0.001 {
        return Ok(Vector2::ZERO);
    }
    let mut low = 0.0;
    let mut high = ship.max_acceleration;
    for _ in 0..48 {
        let middle = (low + high) * 0.5;
        if displacement(ship, middle, INTERVAL_TICKS) < distance {
            low = middle;
        } else {
            high = middle;
        }
    }
    Ok(offset * (high / distance))
}
