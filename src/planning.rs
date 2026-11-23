//! Bounded, read-only planning from the current orders and delivered observations.

use crate::{
    mission::{Mission, QueuedShot, Solution},
    physics::{STEP_SECONDS, Ship, Vector2},
    session::{Phase, Snapshot},
};
use serde::Serialize;

pub const ARRIVAL_TOLERANCE_SECONDS: f64 = STEP_SECONDS;
pub const MAX_PLANNING_SECONDS: f64 = 3600.0;
const FORECAST_SAMPLE_TICKS: u64 = 60;

#[derive(Serialize)]
pub struct ForecastRound {
    pub order_id: u32,
    pub position: Vector2,
    pub velocity: Vector2,
    pub reserved: bool,
}

#[derive(Serialize)]
pub struct Forecast {
    pub time_seconds: f64,
    pub own: Ship,
    pub own_path: Vec<Vector2>,
    pub enemy_position: Vector2,
    pub enemy_velocity: Vector2,
    pub aim: Option<Solution>,
    pub rounds: Vec<ForecastRound>,
    pub asteroids: Vec<crate::physics::Asteroid>,
    pub own_collision: Option<crate::physics::AsteroidHit>,
}

impl Snapshot<'_> {
    /// Shared tick integration for own-ship planning, including known terrain.
    pub(crate) fn advance_planning_ship(
        &self,
        ship: &mut Ship,
        tick: u64,
    ) -> Option<crate::physics::AsteroidHit> {
        let offset = (tick - self.world.tick) as f64 * STEP_SECONDS;
        let mut hit =
            ship.advance_in_terrain(self.planning_burn_at(tick), &self.world.asteroids, offset)?;
        hit.seconds += tick as f64 * STEP_SECONDS;
        Some(hit)
    }

    /// A blocked ballistic solution remains queueable, but its first cover contact is explicit.
    pub(crate) fn cover_solution(&self, mut solution: Solution) -> Solution {
        let launch_seconds = solution.launch_tick as f64 * STEP_SECONDS;
        let velocity = (solution.target - solution.origin) * (1.0 / solution.flight_seconds);
        solution.obstruction = crate::physics::asteroid_contact(
            &self.world.asteroids,
            solution.origin,
            velocity,
            launch_seconds - self.time_seconds,
            solution.flight_seconds,
            crate::scenario::PROJECTILE_RADIUS,
        )
        .map(|mut hit| {
            hit.seconds += launch_seconds;
            hit
        });
        solution
    }

    fn planning_tick(&self, seconds: f64) -> Result<u64, String> {
        let mission = self
            .mission
            .ok_or("Start a blockade mission to plan shots.")?;
        if !seconds.is_finite()
            || seconds < self.time_seconds
            || seconds > mission.config.deadline_tick as f64 * STEP_SECONDS
            || seconds - self.time_seconds > MAX_PLANNING_SECONDS
        {
            return Err("Choose a future time within the mission deadline and 3600-second planning horizon.".into());
        }
        Ok((seconds / STEP_SECONDS).round() as u64)
    }

    /// Lead from the predicted firing state, using only the delivered contact.
    pub fn aim_at(&self, seconds: f64) -> Result<Solution, String> {
        let tick = self.planning_tick(seconds)?;
        self.mission
            .unwrap()
            .aim(&self.planning_ship_at(tick), tick)
            .map(|s| self.cover_solution(s))
    }

    /// Reserve an explicit firing tick; arrival follows from fixed muzzle speed.
    pub fn preview_launch_shot(&self, target: Vector2, seconds: f64) -> Result<Solution, String> {
        let tick = self.planning_tick(seconds)?;
        let mission = self.mission.unwrap();
        if !mission.running() {
            return Err("Mission is over; retry to issue orders.".into());
        }
        if seconds != tick as f64 * STEP_SECONDS {
            return Err("Choose a firing time on the 0.25 s tick grid.".into());
        }
        if tick < self.order_start_tick {
            return Err(format!(
                "Firing before {:.2} s is locked by the active command interval.",
                self.order_start_seconds
            ));
        }
        if tick >= mission.config.deadline_tick {
            return Err("Choose a firing time before the mission deadline.".into());
        }
        mission.check_queue_capacity()?;
        if !mission.slot_available(tick) {
            return Err(
                "Firing time conflicts with the gun's reload window or another reservation.".into(),
            );
        }
        self.preview_shot_at(target, tick)
    }

    pub fn preview_timed_shot(&self, target: Vector2, seconds: f64) -> Result<Solution, String> {
        let mission = self
            .mission
            .ok_or("Start a blockade mission to use the gun.")?;
        mission.check_queue_capacity()?;
        self.solve_timed_shot(target, seconds, mission)
    }

    /// Search every launch tick once, integrating the same bounded burn as execution.
    /// Equal-error solutions choose the earlier tick. Muzzle speed is never adjusted.
    pub(crate) fn solve_timed_shot(
        &self,
        target: Vector2,
        seconds: f64,
        slots: &Mission,
    ) -> Result<Solution, String> {
        let end = self.planning_tick(seconds)?;
        if !slots.running() {
            return Err("Mission is over; retry to issue orders.".into());
        }
        if !target.valid() {
            return Err("Shot target must be finite and within bounds.".into());
        }
        let mut ship = self
            .world
            .ships
            .iter()
            .find(|s| s.id == self.player_id)
            .unwrap()
            .clone();
        let mut best: Option<(f64, Solution)> = None;
        let mut reload_conflict = false;
        for tick in self.world.tick..=end {
            if !ship.disabled
                && tick >= self.order_start_tick
                && (tick as f64 * STEP_SECONDS) < seconds
                && let Ok(solution) = crate::mission::solve_shot(&ship, target, tick)
            {
                let error = (solution.arrival_seconds - seconds).abs();
                if error <= ARRIVAL_TOLERANCE_SECONDS
                    && solution.arrival_seconds <= slots.config.deadline_tick as f64 * STEP_SECONDS
                {
                    if slots.slot_available(tick) {
                        if best.as_ref().is_none_or(|(distance, _)| error < *distance) {
                            best = Some((error, self.cover_solution(solution)));
                        }
                    } else {
                        reload_conflict = true;
                    }
                }
            }
            self.advance_planning_ship(&mut ship, tick);
        }
        best.map(|(_, solution)| solution).ok_or_else(|| if reload_conflict {
            "Arrival conflicts with the gun's reload window or another reservation.".into()
        } else {
            "Unreachable arrival: no launch at or after the editable boundary reaches this point within 0.25 s.".into()
        })
    }

    pub(crate) fn planning_burn_at(&self, tick: u64) -> Vector2 {
        if self.phase == Phase::Executing && tick < self.order_start_tick {
            self.active_burn
        } else {
            self.planned_burn
        }
    }

    /// Validate both the ballistic path and a timed reservation's arrival constraint.
    pub fn preview_order(&self, order: &QueuedShot) -> Result<Solution, String> {
        if let Some(error) = &order.planning_error {
            return Err(error.clone());
        }
        let solution = self.preview_shot_at(order.target, order.launch_tick)?;
        if order
            .requested_arrival_seconds
            .is_some_and(|time| (time - solution.arrival_seconds).abs() > ARRIVAL_TOLERANCE_SECONDS)
        {
            return Err("Maneuver invalidates the requested arrival time.".into());
        }
        Ok(solution)
    }

    /// Future enemy positions are estimates; known terrain contacts use swept prediction.
    pub fn forecast(&self, seconds: f64) -> Result<Forecast, String> {
        let tick = self.planning_tick(seconds)?;
        let mission = self.mission.unwrap();
        let mut own = self
            .world
            .ships
            .iter()
            .find(|s| s.id == self.player_id)
            .unwrap()
            .clone();
        let mut own_path = vec![own.position];
        let mut own_collision = None;
        for t in self.world.tick..tick {
            let hit = self.advance_planning_ship(&mut own, t);
            if own_collision.is_none() {
                own_collision = hit;
            }
            if (t + 1 - self.world.tick).is_multiple_of(FORECAST_SAMPLE_TICKS) || t + 1 == tick {
                own_path.push(own.position);
            }
        }
        let time_seconds = tick as f64 * STEP_SECONDS;
        let mut rounds = Vec::new();
        for projectile in &self.world.projectiles {
            if let Some(shot) = mission
                .shots
                .iter()
                .find(|shot| shot.projectile_id == projectile.id)
            {
                if crate::physics::asteroid_contact(
                    &self.world.asteroids,
                    projectile.position,
                    projectile.velocity,
                    0.0,
                    time_seconds - self.time_seconds,
                    projectile.radius,
                )
                .is_some()
                {
                    continue;
                }
                rounds.push(ForecastRound {
                    order_id: shot.order_id,
                    position: projectile.position
                        + projectile.velocity * (time_seconds - self.time_seconds),
                    velocity: projectile.velocity,
                    reserved: false,
                });
            }
        }
        for order in &mission.queued {
            if order.launch_tick > tick {
                continue;
            }
            if let Ok(solution) = self.preview_order(order) {
                let ship = self.planning_ship_at(order.launch_tick);
                let velocity = ship.velocity + solution.direction * crate::scenario::RAILGUN_SPEED;
                if crate::physics::asteroid_contact(
                    &self.world.asteroids,
                    solution.origin,
                    velocity,
                    order.launch_tick as f64 * STEP_SECONDS - self.time_seconds,
                    time_seconds - order.launch_tick as f64 * STEP_SECONDS,
                    crate::scenario::PROJECTILE_RADIUS,
                )
                .is_some()
                {
                    continue;
                }
                rounds.push(ForecastRound {
                    order_id: order.id,
                    position: solution.origin
                        + velocity * (time_seconds - order.launch_tick as f64 * STEP_SECONDS),
                    velocity,
                    reserved: true,
                });
            }
        }
        Ok(Forecast {
            time_seconds,
            aim: (!own.disabled)
                .then(|| mission.aim(&own, tick).ok())
                .flatten()
                .map(|s| self.cover_solution(s)),
            asteroids: self
                .world
                .asteroids
                .iter()
                .cloned()
                .map(|mut a| {
                    a.position = a.position + a.velocity * (time_seconds - self.time_seconds);
                    a
                })
                .collect(),
            own_collision,
            own,
            own_path,
            enemy_position: mission.sensors.contact.position_at(tick),
            enemy_velocity: mission.sensors.contact.velocity,
            rounds,
        })
    }
}
