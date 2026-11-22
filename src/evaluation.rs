//! One deterministic terminal score for the UI and diagnostic REPL.

use serde::Serialize;

use crate::{mission::Outcome, session::Snapshot};

pub const VICTORY_POINTS: u32 = 1000;
pub const SHOT_BUDGET_POINTS: u32 = 500;
pub const POINTS_PER_SHOT: u32 = 20;
pub const FUEL_BUDGET_POINTS: u32 = 500;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Evaluation {
    pub total: u32,
    pub victory_points: u32,
    pub shot_points: u32,
    pub fuel_points: u32,
    pub shots_fired: usize,
    pub spent_delta_v: f64,
}

pub fn evaluate(snapshot: &Snapshot<'_>) -> Option<Evaluation> {
    let mission = snapshot.mission?;
    if mission.running() {
        return None;
    }
    let own = snapshot
        .world
        .ships
        .iter()
        .find(|ship| ship.id == snapshot.player_id)?;
    let won = matches!(mission.outcome, Outcome::Disabled | Outcome::Timeout);
    let shots_fired = mission.shots.len();
    let spent_delta_v = (snapshot.initial_delta_v - own.remaining_dv).max(0.0);
    let victory_points = if won { VICTORY_POINTS } else { 0 };
    let shot_points = if won {
        SHOT_BUDGET_POINTS.saturating_sub(
            POINTS_PER_SHOT.saturating_mul(u32::try_from(shots_fired).unwrap_or(u32::MAX)),
        )
    } else {
        0
    };
    let fuel_points = if won && snapshot.initial_delta_v > 0.0 {
        (FUEL_BUDGET_POINTS as f64 * (own.remaining_dv / snapshot.initial_delta_v).clamp(0.0, 1.0))
            .floor() as u32
    } else {
        0
    };
    Some(Evaluation {
        total: victory_points + shot_points + fuel_points,
        victory_points,
        shot_points,
        fuel_points,
        shots_fired,
        spent_delta_v,
    })
}
