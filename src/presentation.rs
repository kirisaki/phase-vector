//! The normal UI's explicit, observation-based state contract, without browser dependencies.

use serde::Serialize;

use crate::{
    maneuver::Preview,
    map::{MapFrame, MapView},
    mission::{Outcome, QueuedShot, Shot, Solution},
    observation::Track,
    physics::{Impact, Projectile, STEP_SECONDS, Ship, Vector2},
    scenario::Scale,
    session::{CommandRecord, Phase, Snapshot},
};

/// Only these fields cross into the normal UI. Privileged inspection belongs to Lua.
#[derive(Serialize)]
pub struct State<'a> {
    pub world: VisibleWorld<'a>,
    pub mission: Option<MissionView<'a>>,
    pub scale: Scale,
    pub timing: crate::timing::Timing,
    pub player_id: u32,
    pub phase: Phase,
    pub paused: bool,
    pub stop_at_boundary: bool,
    pub speed: u32,
    pub planned_burn: Vector2,
    pub active_burn: Vector2,
    pub time_seconds: f64,
    pub remaining_seconds: f64,
    pub order_start_seconds: f64,
    pub command_number: usize,
    pub command_log: &'a [CommandRecord],
    pub maneuver: Preview,
    pub map: MapFrame,
}

#[derive(Serialize)]
pub struct VisibleWorld<'a> {
    pub tick: u64,
    pub ships: Vec<VisibleShip>,
    pub projectiles: &'a [Projectile],
    pub impacts: &'a [Impact],
    next_projectile_id: u64,
}

#[derive(Serialize)]
pub struct VisibleShip {
    pub id: u32,
    pub position: Vector2,
    pub velocity: Vector2,
    pub radius: f64,
    pub max_acceleration: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_dv: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spent_dv: Option<f64>,
}

impl VisibleShip {
    fn new(ship: &Ship, hide_fuel: bool) -> Self {
        Self {
            id: ship.id,
            position: ship.position,
            velocity: ship.velocity,
            radius: ship.radius,
            max_acceleration: ship.max_acceleration,
            remaining_dv: (!hide_fuel).then_some(ship.remaining_dv),
            spent_dv: (!hide_fuel).then_some(ship.spent_dv),
        }
    }
}

#[derive(Serialize)]
pub struct QueuedShotView<'a> {
    #[serde(flatten)]
    pub order: &'a QueuedShot,
    pub locked: bool,
    pub solution: Option<Solution>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct MissionView<'a> {
    pub outcome: Outcome,
    pub ended_seconds: Option<f64>,
    pub goal: Vector2,
    pub goal_radius: f64,
    pub deadline_seconds: f64,
    pub ready_seconds: f64,
    pub reload_seconds: f64,
    pub available_launch_seconds: Vec<f64>,
    pub shots: &'a [Shot],
    pub queued: Vec<QueuedShotView<'a>>,
    pub queue_capacity: usize,
    pub planning_horizon_seconds: f64,
    pub arrival_tolerance_seconds: f64,
    pub queue_error: Option<String>,
    pub aim: Option<Solution>,
    pub contact: &'a Track,
    pub contact_age: f64,
    pub observed_acceleration: Option<Vector2>,
    pub prediction: [Vector2; 2],
}

impl<'a> State<'a> {
    pub fn new(snapshot: &Snapshot<'a>, map: &MapView) -> Self {
        let visible = snapshot.presentation_world();
        let mission = snapshot.mission.map(|mission| {
            let contact = &mission.sensors.contact;
            let position = contact.position_at(snapshot.world.tick);
            let launch_tick = snapshot.next_shot_tick();
            MissionView {
                outcome: mission.outcome,
                ended_seconds: mission.ended_seconds,
                goal: mission.config.goal,
                goal_radius: mission.config.goal_radius,
                deadline_seconds: mission.config.deadline_tick as f64 * STEP_SECONDS,
                ready_seconds: mission.ready_tick as f64 * STEP_SECONDS,
                reload_seconds: mission.config.reload_ticks as f64 * STEP_SECONDS,
                available_launch_seconds: mission
                    .available_launch_ticks(snapshot.order_start_tick)
                    .into_iter()
                    .map(|tick| tick as f64 * STEP_SECONDS)
                    .collect(),
                shots: &mission.shots,
                queue_capacity: crate::mission::MAX_QUEUED_SHOTS,
                planning_horizon_seconds: crate::planning::MAX_PLANNING_SECONDS,
                arrival_tolerance_seconds: crate::planning::ARRIVAL_TOLERANCE_SECONDS,
                queued: mission
                    .queued
                    .iter()
                    .map(|order| {
                        let solution = snapshot.preview_order(order);
                        QueuedShotView {
                            order,
                            locked: order.planning_error.is_none()
                                && order.launch_tick < snapshot.order_start_tick,
                            error: solution.as_ref().err().cloned(),
                            solution: solution.ok(),
                        }
                    })
                    .collect(),
                queue_error: launch_tick.as_ref().err().cloned(),
                aim: snapshot.aim().ok(),
                contact,
                contact_age: (snapshot.world.tick - contact.sampled_tick) as f64 * STEP_SECONDS,
                observed_acceleration: mission.sensors.contact_acceleration(),
                prediction: [
                    position,
                    position + contact.velocity * crate::timing::FORECAST_SECONDS,
                ],
            }
        });
        Self {
            world: VisibleWorld {
                tick: visible.tick,
                ships: visible
                    .ships
                    .iter()
                    .map(|ship| {
                        VisibleShip::new(
                            ship,
                            snapshot
                                .mission
                                .is_some_and(|m| ship.id == m.config.enemy_id),
                        )
                    })
                    .collect(),
                projectiles: &snapshot.world.projectiles,
                impacts: &snapshot.world.impacts,
                next_projectile_id: snapshot.world.next_projectile_id(),
            },
            mission,
            scale: snapshot.scale,
            timing: crate::timing::SETTINGS,
            player_id: snapshot.player_id,
            phase: snapshot.phase,
            paused: snapshot.paused,
            stop_at_boundary: snapshot.stop_at_boundary,
            speed: snapshot.speed,
            planned_burn: snapshot.planned_burn,
            active_burn: snapshot.active_burn,
            time_seconds: snapshot.time_seconds,
            remaining_seconds: snapshot.remaining_seconds,
            order_start_seconds: snapshot.order_start_seconds,
            command_number: snapshot.command_number,
            command_log: snapshot.command_log,
            maneuver: snapshot.planned_route(snapshot.planned_burn),
            map: map.frame(&visible, snapshot.player_id),
        }
    }
}
