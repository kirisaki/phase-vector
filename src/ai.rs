//! Deterministic autonomous evasion using only own state and delivered tracks.

use crate::{
    observation::Track,
    physics::{STEP_SECONDS, Ship, Vector2},
};
use serde::Serialize;

pub const DECISION_TICKS: u64 = crate::timing::seconds_to_ticks(1);
// These tolerances serve different physical quantities and must not share an epsilon.
const COAST_ACCELERATION_TOLERANCE: f64 = 0.001; // m/s²
const SYMMETRIC_APPROACH_TOLERANCE: f64 = 1.0; // m

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
            reaction_seconds: 120.0,
            safety_radius: 75.0 * crate::units::METRES_PER_KILOMETRE,
            hold_seconds: 120.0,
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
    let danger = tracks
        .iter()
        .filter_map(|track| {
            let r = track.position_at(now) - own.position;
            let v = track.velocity - own.velocity;
            let speed2 = v.x * v.x + v.y * v.y;
            if speed2 <= 0.0 {
                return None;
            }
            let t = -(r.x * v.x + r.y * v.y) / speed2;
            let miss = (r + v * t).length();
            (t >= 0.0
                && t <= policy.reaction_seconds
                && miss < policy.safety_radius + own.radius + track.radius)
                .then_some((t, track))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
    if let Some((t, track)) = danger {
        let relative_velocity = track.velocity - own.velocity;
        let normal = Vector2::new(-relative_velocity.y, relative_velocity.x)
            * (1.0 / relative_velocity.length());
        let separation =
            own.position + own.velocity * t - (track.position_at(now) + track.velocity * t);
        let signed = separation.x * normal.x + separation.y * normal.y;
        // On a symmetric approach use a stable side, not hidden mission outcomes.
        let direction = if signed < -SYMMETRIC_APPROACH_TOLERANCE {
            normal * -1.0
        } else {
            normal
        };
        decision.acceleration = direction * own.max_acceleration;
        decision.reason = format!(
            "Evading observed projectile {} (closest approach in {:.1} s)",
            track.id, t
        );
        decision.threat_id = Some(track.id);
        decision.hold_until = now + (policy.hold_seconds / STEP_SECONDS).ceil() as u64;
        decision.evasions += 1;
    }
    decision
}
