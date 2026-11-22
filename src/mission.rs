//! Blockade rules, ballistic gun, delayed observations, and autonomous enemy control.

use crate::{
    ai::{self, Decision, Policy},
    observation::Sensors,
    physics::{Burn, STEP_SECONDS, Ship, Vector2, World, swept_contact},
    scenario,
    timing::seconds_to_ticks,
    units::METRES_PER_KILOMETRE,
};
use serde::Serialize;

pub const MAX_QUEUED_SHOTS: usize = 4;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Config {
    pub enemy_id: u32,
    pub goal: Vector2,
    pub goal_radius: f64,
    pub deadline_tick: u64,
    pub reload_ticks: u64,
    pub sensor_processing_ticks: u64,
    pub cruise_speed: f64,
    pub policy: Policy,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enemy_id: 2,
            goal: Vector2::new(
                scenario::INITIAL_SEPARATION / 2.0,
                -45_000.0 * METRES_PER_KILOMETRE,
            ),
            goal_radius: 1_000.0 * METRES_PER_KILOMETRE,
            deadline_tick: seconds_to_ticks(1215),
            reload_ticks: seconds_to_ticks(30),
            sensor_processing_ticks: seconds_to_ticks(1),
            cruise_speed: scenario::CONTACT_SPEED,
            policy: Policy::default(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Running,
    Breakthrough,
    Disabled,
    Timeout,
}
impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Breakthrough => "breakthrough",
            Self::Disabled => "disabled",
            Self::Timeout => "timeout",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QueuedShot {
    pub id: u32,
    pub target: Vector2,
    pub launch_tick: u64,
    pub requested_arrival_seconds: Option<f64>,
    pub planning_error: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Shot {
    pub projectile_id: u64,
    pub order_id: u32,
    pub launch_tick: u64,
    pub arrival_seconds: f64,
    pub origin: Vector2,
    pub target: Vector2,
    pub velocity: Vector2,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Solution {
    pub origin: Vector2,
    pub target: Vector2,
    pub direction: Vector2,
    pub flight_seconds: f64,
    pub launch_tick: u64,
    pub arrival_seconds: f64,
}

const QUADRATIC_DEGENERACY_TOLERANCE: f64 = 1e-6; // (m/s)²

// Positive interception time for a fixed relative muzzle speed.
fn intercept_time(r: Vector2, velocity: Vector2) -> Result<f64, String> {
    let a = velocity.x * velocity.x + velocity.y * velocity.y - scenario::RAILGUN_SPEED.powi(2);
    let b = 2.0 * (r.x * velocity.x + r.y * velocity.y);
    let c = r.x * r.x + r.y * r.y;
    if c <= 0.0 {
        return Err("Choose a target away from the firing ship.".into());
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return Err("Target is unreachable at this muzzle speed.".into());
    }
    let mut roots = Vec::new();
    if a.abs() < QUADRATIC_DEGENERACY_TOLERANCE {
        if b != 0.0 {
            roots.push(-c / b);
        }
    } else {
        let q = -0.5 * (b + discriminant.sqrt().copysign(b));
        roots.push(q / a);
        if q != 0.0 {
            roots.push(c / q);
        }
    }
    roots
        .into_iter()
        .filter(|t| t.is_finite() && *t > 0.0)
        .min_by(f64::total_cmp)
        .ok_or_else(|| "Target is unreachable at this muzzle speed.".into())
}
pub fn solve_shot(ship: &Ship, target: Vector2, launch_tick: u64) -> Result<Solution, String> {
    if !target.valid() {
        return Err("Shot target must be finite and within bounds.".into());
    }
    let t = intercept_time(target - ship.position, ship.velocity * -1.0)?;
    let direction =
        (target - ship.position - ship.velocity * t) * (1.0 / (scenario::RAILGUN_SPEED * t));
    Ok(Solution {
        origin: ship.position,
        target,
        direction,
        flight_seconds: t,
        launch_tick,
        arrival_seconds: launch_tick as f64 * STEP_SECONDS + t,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mission {
    pub config: Config,
    pub outcome: Outcome,
    pub ended_seconds: Option<f64>,
    pub sensors: Sensors,
    pub decision: Decision,
    pub queued: Vec<QueuedShot>,
    next_order_id: u32,
    pub ready_tick: u64,
    pub shots: Vec<Shot>,
}
impl Mission {
    pub fn new(world: &World, player_id: u32, config: Config) -> Result<Self, String> {
        let enemy = world
            .ships
            .iter()
            .find(|s| s.id == config.enemy_id)
            .ok_or("Missing enemy ship.")?;
        if config.enemy_id == player_id
            || !world.ships.iter().any(|s| s.id == player_id)
            || !config.goal.valid()
            || !config.goal_radius.is_finite()
            || config.goal_radius <= 0.0
            || config.deadline_tick <= world.tick
            || config.reload_ticks == 0
            || !config.cruise_speed.is_finite()
            || config.cruise_speed < 0.0
            || [
                config.policy.reaction_seconds,
                config.policy.safety_radius,
                config.policy.hold_seconds,
                config.policy.recovery_seconds,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
            || config.policy.recovery_seconds == 0.0
        {
            return Err("Invalid mission configuration.".into());
        }
        Ok(Self {
            sensors: Sensors::new(enemy, world.tick),
            config,
            outcome: Outcome::Running,
            ended_seconds: None,
            decision: Decision::default(),
            queued: Vec::new(),
            next_order_id: 1,
            ready_tick: world.tick,
            shots: Vec::new(),
        })
    }
    pub fn running(&self) -> bool {
        self.outcome == Outcome::Running
    }
    pub fn aim(&self, ship: &Ship, launch_tick: u64) -> Result<Solution, String> {
        let contact = &self.sensors.contact;
        let position = contact.position_at(launch_tick);
        let t = intercept_time(position - ship.position, contact.velocity - ship.velocity)?;
        solve_shot(ship, position + contact.velocity * t, launch_tick)
    }
    /// Find the earliest reload-compatible tick at or after the editable command boundary.
    pub fn next_launch_tick(&self, from_tick: u64) -> Result<u64, String> {
        if !self.running() {
            return Err("Mission is over; retry to issue orders.".into());
        }
        self.check_queue_capacity()?;
        let exhausted = || "No free firing slot remains before the deadline.".to_string();
        let mut tick = from_tick.max(self.ready_tick);
        for order in &self.queued {
            if order.planning_error.is_some() {
                continue;
            }
            if tick.abs_diff(order.launch_tick) < self.config.reload_ticks {
                tick = order
                    .launch_tick
                    .checked_add(self.config.reload_ticks)
                    .ok_or_else(exhausted)?;
            }
        }
        if tick >= self.config.deadline_tick {
            return Err(exhausted());
        }
        Ok(tick)
    }
    /// Starts of free firing windows, using the same reload/capacity rules as Live.
    pub fn available_launch_ticks(&self, from_tick: u64) -> Vec<u64> {
        let starts = std::iter::once(from_tick).chain(self.queued.iter().filter_map(|order| {
            order
                .planning_error
                .is_none()
                .then(|| order.launch_tick.checked_add(self.config.reload_ticks))
                .flatten()
        }));
        let mut ticks: Vec<_> = starts
            .filter_map(|start| self.next_launch_tick(start.max(from_tick)).ok())
            .collect();
        ticks.sort_unstable();
        ticks.dedup();
        ticks
    }

    pub fn cancel(&mut self, id: u32, editable_from: u64) -> Result<(), String> {
        let index = self
            .queued
            .iter()
            .position(|order| order.id == id)
            .ok_or("Unknown queued shot.")?;
        if self.queued[index].planning_error.is_none()
            && self.queued[index].launch_tick < editable_from
        {
            return Err("This shot is committed to the active command interval.".into());
        }
        self.queued.remove(index);
        Ok(())
    }
    pub fn queue(&mut self, ship: &Ship, target: Vector2, launch_tick: u64) -> Result<(), String> {
        if !self.running() {
            return Err("Mission is over; retry to issue orders.".into());
        }
        self.check_queue_capacity()?;
        if launch_tick < self.ready_tick {
            return Err("Gun is still reloading at the requested launch time.".into());
        }
        if launch_tick >= self.config.deadline_tick {
            return Err("Launch would be after the mission deadline.".into());
        }
        if !self.slot_available(launch_tick) {
            return Err("Another reservation occupies this gun's reload window.".into());
        }
        solve_shot(ship, target, launch_tick)?;
        let next_id = self
            .next_order_id
            .checked_add(1)
            .ok_or("Shot ID limit reached.")?;
        self.queued.push(QueuedShot {
            id: self.next_order_id,
            target,
            launch_tick,
            requested_arrival_seconds: None,
            planning_error: None,
        });
        self.next_order_id = next_id;
        self.queued
            .sort_by_key(|order| (order.planning_error.is_some(), order.launch_tick));
        Ok(())
    }
    pub(crate) fn check_queue_capacity(&self) -> Result<(), String> {
        if self.queued.len() >= MAX_QUEUED_SHOTS {
            return Err(format!(
                "Shot queue is full ({MAX_QUEUED_SHOTS}/{MAX_QUEUED_SHOTS}). Wait for a launch or cancel an editable shot."
            ));
        }
        Ok(())
    }
    pub(crate) fn slot_available(&self, tick: u64) -> bool {
        tick >= self.ready_tick
            && tick < self.config.deadline_tick
            && self
                .queued
                .iter()
                .filter(|order| order.planning_error.is_none())
                .all(|order| order.launch_tick.abs_diff(tick) >= self.config.reload_ticks)
    }
    pub fn launch_queued(
        &mut self,
        world: &mut World,
        player_id: u32,
    ) -> Result<Option<Vector2>, String> {
        if !self.running() {
            return Ok(None);
        }
        let Some(order) = self.queued.first() else {
            return Ok(None);
        };
        // Invalid editable orders are retained for correction; the next boundary blocks them.
        if order.planning_error.is_some() {
            return Ok(None);
        }
        if order.launch_tick > world.tick || self.ready_tick > world.tick {
            return Ok(None);
        }
        let ready_tick = world
            .tick
            .checked_add(self.config.reload_ticks)
            .ok_or("Gun reload tick capacity exhausted.")?;
        let ship = world.ships.iter().find(|s| s.id == player_id).unwrap();
        let solution = solve_shot(ship, order.target, world.tick)?;
        let origin = ship.position;
        let velocity = ship.velocity + solution.direction * scenario::RAILGUN_SPEED;
        let target = order.target;
        let projectile_id =
            world.launch(player_id, origin, velocity, scenario::PROJECTILE_RADIUS)?;
        self.shots.push(Shot {
            projectile_id,
            order_id: order.id,
            launch_tick: world.tick,
            arrival_seconds: solution.arrival_seconds,
            origin,
            target,
            velocity,
        });
        self.ready_tick = ready_tick;
        self.queued.remove(0);
        Ok(Some(target))
    }
    pub fn step(&mut self, world: &mut World, player_id: u32, acceleration: Vector2) {
        if !self.running() {
            return;
        }
        self.sensors.update(
            world,
            player_id,
            self.config.enemy_id,
            self.config.sensor_processing_ticks,
        );
        let enemy = world
            .ships
            .iter()
            .find(|s| s.id == self.config.enemy_id)
            .unwrap();
        if world.tick.is_multiple_of(ai::DECISION_TICKS) {
            self.decision = ai::decide(
                enemy,
                self.config.goal,
                self.config.cruise_speed,
                &self.sensors.threats,
                world.tick,
                self.config.policy,
                &self.decision,
            );
        }
        let previous = enemy.position;
        let tick = world.tick;
        let impacts = world.impacts.len();
        world
            .step(&[
                Burn {
                    ship_id: player_id,
                    acceleration,
                },
                Burn {
                    ship_id: self.config.enemy_id,
                    acceleration: self.decision.acceleration,
                },
            ])
            .expect("validated mission orders");
        let enemy = world
            .ships
            .iter()
            .find(|s| s.id == self.config.enemy_id)
            .unwrap();
        let arrival = swept_contact(
            previous,
            enemy.position,
            self.config.goal,
            self.config.goal,
            self.config.goal_radius,
        );
        let hit = world.impacts[impacts..]
            .iter()
            .filter(|i| i.ship_id == self.config.enemy_id)
            .map(|i| i.fraction)
            .min_by(f64::total_cmp);
        // Earlier events win; an exact hit/entry tie favors disablement.
        let event = match (hit, arrival) {
            (Some(h), Some(a)) if h <= a => Some((Outcome::Disabled, h)),
            (_, Some(a)) => Some((Outcome::Breakthrough, a)),
            (Some(h), None) => Some((Outcome::Disabled, h)),
            _ => None,
        };
        if let Some((outcome, fraction)) = event {
            self.outcome = outcome;
            self.ended_seconds = Some((tick as f64 + fraction) * STEP_SECONDS);
        } else if world.tick >= self.config.deadline_tick {
            self.outcome = Outcome::Timeout;
            self.ended_seconds = Some(self.config.deadline_tick as f64 * STEP_SECONDS);
        }
    }
    pub fn presentation_world(&self, world: &World) -> World {
        let mut visible = world.clone();
        if let Some(enemy) = visible
            .ships
            .iter_mut()
            .find(|s| s.id == self.config.enemy_id)
        {
            enemy.position = self.sensors.contact.position_at(world.tick);
            enemy.velocity = self.sensors.contact.velocity;
            // Unknown fuel is not used by presentation or prediction.
            enemy.remaining_dv = 0.0;
            enemy.spent_dv = 0.0;
        }
        visible
    }
}
