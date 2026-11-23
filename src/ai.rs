//! Deterministic autonomous evasion using only own state and delivered tracks.

use crate::{
    observation::Track,
    physics::{STEP_SECONDS, Ship, Vector2},
};
use serde::Serialize;

pub const DECISION_TICKS: u64 = crate::timing::seconds_to_ticks(1);
// These tolerances serve different physical quantities and must not share an epsilon.
const COAST_ACCELERATION_TOLERANCE: f64 = 0.001; // m/s²
const EVASION_FRACTIONS: [f64; 5] = [0.125, 0.25, 0.5, 0.75, 1.0];
const PREDICTION_STEP_SECONDS: f64 = 2.0;
const CLEARANCE_TIE_TOLERANCE: f64 = 1e-9;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Decision {
    pub acceleration: Vector2,
    pub reason: String,
    pub threat_id: Option<u64>,
    pub hold_until: u64,
    pub evasions: u32,
}
impl Default for Decision {
    fn default() -> Self {
        Self {
            acceleration: Vector2::ZERO,
            reason: "Following mission route".into(),
            threat_id: None,
            hold_until: 0,
            evasions: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Policy {
    pub reaction_seconds: f64,
    pub safety_radius: f64,
    pub hold_seconds: f64,
    pub recovery_seconds: f64,
}
// These durations are independent tuning knobs, not aliases of the command interval.
impl Default for Policy {
    fn default() -> Self {
        Self {
            reaction_seconds: 90.0,
            safety_radius: 500.0 * crate::units::METRES_PER_KILOMETRE,
            hold_seconds: 1.0,
            recovery_seconds: 120.0,
        }
    }
}

pub fn decide(
    own: &Ship,
    goal: Vector2,
    cruise_speed: f64,
    tracks: &[Track],
    now: u64,
    policy: Policy,
    previous: &Decision,
) -> Decision {
    decide_with_terrain(
        own,
        goal,
        cruise_speed,
        (tracks, &[]),
        now,
        policy,
        previous,
    )
}

pub fn decide_with_terrain(
    own: &Ship,
    goal: Vector2,
    cruise_speed: f64,
    observed: (&[Track], &[crate::physics::Asteroid]),
    now: u64,
    policy: Policy,
    previous: &Decision,
) -> Decision {
    let (tracks, asteroids) = observed;
    let mut hazards: Vec<_> = tracks
        .iter()
        .map(|track| Hazard {
            id: Some(track.id),
            position: track.position_at(now),
            velocity: track.velocity,
            radius: track.radius,
            lifetime: crate::physics::asteroid_contact(
                asteroids,
                track.position,
                track.velocity,
                -((now - track.sampled_tick) as f64 * STEP_SECONDS),
                (now - track.sampled_tick) as f64 * STEP_SECONDS + policy.reaction_seconds,
                track.radius,
            )
            .map_or(policy.reaction_seconds, |hit| {
                (hit.seconds - (now - track.sampled_tick) as f64 * STEP_SECONDS).max(0.0)
            }),
        })
        .filter(|hazard| hazard.lifetime > 0.0)
        .collect();
    // Ephemerides are shared briefing data. Unknown rounds never enter this list.
    hazards.extend(asteroids.iter().map(|a| Hazard {
        id: None,
        position: a.position,
        velocity: a.velocity,
        radius: a.radius,
        lifetime: policy.reaction_seconds,
    }));
    let mut decision = previous.clone();
    if own.remaining_dv <= 0.0 {
        decision.acceleration = Vector2::ZERO;
        decision.reason = "Delta-v exhausted; coasting (mission still active)".into();
        decision.threat_id = None;
        return decision;
    }
    if now < previous.hold_until {
        return decision;
    }
    let offset = goal - own.position;
    let desired = if offset.length() > 0.0 {
        offset * (cruise_speed / offset.length())
    } else {
        Vector2::ZERO
    };
    decision.acceleration =
        ((desired - own.velocity) * (1.0 / policy.recovery_seconds)).clamped(own.max_acceleration);
    decision.threat_id = None;
    decision.reason = if decision.acceleration.length() < COAST_ACCELERATION_TOLERANCE {
        "Following mission route"
    } else {
        "Returning to mission route and cruise speed"
    }
    .into();
    let goal_direction = if offset.length() > 0.0 {
        offset * (1.0 / offset.length())
    } else {
        Vector2::new(1.0, 0.0)
    };
    // Broad phase includes any track reachable under maximum acceleration in the horizon.
    let travel = 0.5
        * own.max_acceleration
        * policy.reaction_seconds
        * (policy.reaction_seconds + STEP_SECONDS);
    let relevant: Vec<_> = hazards
        .iter()
        .filter(|track| {
            let r = track.position - own.position;
            let v = track.velocity - own.velocity;
            let speed2 = dot(v, v);
            if speed2 == 0.0 {
                return r.length() <= policy.safety_radius + own.radius + track.radius + travel;
            }
            let t = (-dot(r, v) / speed2).clamp(0.0, track.lifetime);
            (r + v * t).length() <= policy.safety_radius + own.radius + track.radius + travel
        })
        .collect();
    if relevant.is_empty() {
        return decision;
    }
    let recovery = decision.acceleration;
    let (recovery_clearance, threat) = clearance(own, recovery, &relevant, policy);
    if recovery_clearance >= 1.0 {
        return decision;
    }
    // Evaluate all observed threats together. Prefer a safe low-cost burn, then forward progress.
    let mut best = recovery;
    let mut best_clearance = recovery_clearance;
    let mut best_cost = f64::INFINITY;
    let mut consider = |acceleration: Vector2| {
        let (margin, _) = clearance(own, acceleration, &relevant, policy);
        let cost = acceleration.length() + 2.0 * (-dot(acceleration, goal_direction)).max(0.0);
        let safe = margin >= 1.0;
        let best_safe = best_clearance >= 1.0;
        if (safe && !best_safe)
            || (safe && best_safe && cost < best_cost)
            || (!safe
                && !best_safe
                && (margin > best_clearance + CLEARANCE_TIE_TOLERANCE
                    || ((margin - best_clearance).abs() <= CLEARANCE_TIE_TOLERANCE
                        && cost < best_cost)))
        {
            best = acceleration;
            best_clearance = margin;
            best_cost = cost;
        }
    };
    consider(Vector2::ZERO);
    // Rotate from the mission direction to make equal-cost choices deterministic and useful.
    for fraction in EVASION_FRACTIONS {
        for i in 0..8 {
            let angle = i as f64 * std::f64::consts::FRAC_PI_4;
            let (sin, cos) = angle.sin_cos();
            let direction = Vector2::new(
                goal_direction.x * cos - goal_direction.y * sin,
                goal_direction.x * sin + goal_direction.y * cos,
            );
            consider(direction * (own.max_acceleration * fraction));
        }
    }
    decision.acceleration = best;
    decision.threat_id = threat;
    decision.hold_until = now + (policy.hold_seconds / STEP_SECONDS).ceil() as u64;
    if previous.threat_id != threat {
        decision.evasions += 1;
    }
    decision.reason = format!(
        "Avoiding {} with {:.1}% thrust; evaluating {} observed tracks / terrain",
        threat.map_or_else(
            || "known asteroid".to_string(),
            |id| format!("observed projectile {id}")
        ),
        100.0 * best.length() / own.max_acceleration.max(1.0),
        relevant.len()
    );

    decision
}

struct Hazard {
    id: Option<u64>,
    position: Vector2,
    velocity: Vector2,
    radius: f64,
    lifetime: f64,
}

fn dot(a: Vector2, b: Vector2) -> f64 {
    a.x * b.x + a.y * b.y
}

// Predict a candidate burn with finite fuel. Segment closest-approach checks retain fast rounds.
// This is an AI approximation, never the authoritative collision detector.
fn clearance(
    own: &Ship,
    acceleration: Vector2,
    tracks: &[&Hazard],
    policy: Policy,
) -> (f64, Option<u64>) {
    let magnitude = acceleration.length();
    let burn_time = if magnitude > 0.0 {
        own.remaining_dv / magnitude
    } else {
        0.0
    };
    let position = |t: f64| {
        let burning = t.min(burn_time);
        own.position
            + own.velocity * t
            + acceleration * (burning * (t - 0.5 * burning + 0.5 * STEP_SECONDS))
    };
    let mut minimum = f64::INFINITY;
    let mut threat = None;
    for track in tracks {
        let origin = track.position;
        let radius = (policy.safety_radius + own.radius + track.radius).max(1.0);
        let mut time = 0.0;
        let mut relative = own.position - origin;
        while time < track.lifetime {
            let next_time = (time + PREDICTION_STEP_SECONDS).min(track.lifetime);
            let next = position(next_time) - (origin + track.velocity * next_time);
            let delta = next - relative;
            let u = if dot(delta, delta) > 0.0 {
                (-dot(relative, delta) / dot(delta, delta)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let distance = relative + delta * u;
            let ratio = dot(distance, distance) / (radius * radius);
            if ratio < minimum {
                minimum = ratio;
                threat = track.id;
            }
            relative = next;
            time = next_time;
        }
    }
    (minimum, threat)
}
