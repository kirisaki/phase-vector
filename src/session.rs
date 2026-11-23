//! WEGO command boundaries and playback scheduling, independent of the renderer.

use serde::Serialize;

use crate::physics::{Burn, STEP_SECONDS, Vector2, World};

use crate::parameters::{Edit, Parameter, RunKind};
use crate::timing::STEP_FORWARD_TICKS;
pub use crate::timing::{INTERVAL_SECONDS, INTERVAL_TICKS, PULSES_PER_TICK};

pub const DEFAULT_SPEED: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Planning,
    Executing,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Executing => "executing",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Command {
    PlanBurn(Vector2),
    PlanArrival(Vector2),
    QueueShot(Vector2),
    QueueTimedShot(Vector2, f64),
    QueueLaunchShot(Vector2, f64),
    CancelShot,
    CancelQueuedShot(u32),
    StartMission,
    Sandbox,
    Commit,
    Pause,
    Resume,
    Step,
    StepForward,
    NextTurn,
    SetSpeed(f64),
    Configure(Parameter, f64),
    Reset,
}

impl Command {
    pub fn named(name: &str) -> Result<Self, String> {
        match name {
            "commit" => Ok(Self::Commit),
            "pause" => Ok(Self::Pause),
            "play" | "resume" => Ok(Self::Resume),
            "step" => Ok(Self::Step),
            "step_forward" => Ok(Self::StepForward),
            "skip" | "next_turn" => Ok(Self::NextTurn),
            "reset" | "retry" => Ok(Self::Reset),
            "mission" => Ok(Self::StartMission),
            "sandbox" => Ok(Self::Sandbox),
            "cancel_shot" => Ok(Self::CancelShot),
            _ => Err(format!("Unknown command: {name}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommandRecord {
    pub start_tick: u64,
    pub burn: Burn,
    pub shots: Vec<crate::mission::QueuedShot>,
}

#[derive(Clone, Debug)]
pub struct Session {
    world: World,
    initial: World,
    mission: Option<crate::mission::Mission>,
    initial_mission: Option<crate::mission::Config>,
    player_id: u32,
    phase: Phase,
    paused: bool,
    stop_at_boundary: bool,
    planned_burn: Vector2,
    active_burn: Vector2,
    interval_end: u64,
    speed: u32,
    playback_quanta: u32,
    command_log: Vec<CommandRecord>,
    parameter_edits: Vec<Edit>,
}

/// Authoritative state for commands and diagnostics; use presentation::State for the UI.
pub struct Snapshot<'a> {
    pub world: &'a World,
    pub mission: Option<&'a crate::mission::Mission>,
    pub initial_mission: Option<&'a crate::mission::Config>,
    pub initial_delta_v: f64,
    pub scale: crate::scenario::Scale,
    pub player_id: u32,
    pub phase: Phase,
    pub paused: bool,
    pub stop_at_boundary: bool,
    pub speed: u32,
    pub planned_burn: Vector2,
    pub active_burn: Vector2,
    pub time_seconds: f64,
    pub remaining_seconds: f64,
    pub order_start_tick: u64,
    pub order_start_seconds: f64,
    pub command_number: usize,
    pub command_log: &'a [CommandRecord],
    pub run_kind: RunKind,
    pub parameter_edits: &'a [Edit],
}

impl Default for Session {
    fn default() -> Self {
        Self::new(crate::scenario::initial_world(), 1).unwrap()
    }
}

impl Session {
    pub fn new(world: World, player_id: u32) -> Result<Self, String> {
        if !world.ships.iter().any(|ship| ship.id == player_id) {
            return Err("The controlled ship must exist.".into());
        }
        Ok(Self {
            initial: world.clone(),
            mission: None,
            initial_mission: None,
            interval_end: world.tick,
            world,
            player_id,
            phase: Phase::Planning,
            paused: true,
            stop_at_boundary: false,
            planned_burn: Vector2::ZERO,
            active_burn: Vector2::ZERO,
            speed: DEFAULT_SPEED,
            playback_quanta: 0,
            command_log: Vec::new(),
            parameter_edits: Vec::new(),
        })
    }

    pub fn blockade() -> Self {
        Self::with_mission(
            crate::scenario::blockade_world(),
            1,
            crate::mission::Config::default(),
        )
        .unwrap()
    }

    pub fn with_mission(
        world: World,
        player_id: u32,
        config: crate::mission::Config,
    ) -> Result<Self, String> {
        let mission = crate::mission::Mission::new(&world, player_id, config.clone())?;
        let mut session = Self::new(world, player_id)?;
        session.mission = Some(mission);
        session.initial_mission = Some(config);
        Ok(session)
    }

    pub fn mission(&self) -> Option<&crate::mission::Mission> {
        self.mission.as_ref()
    }

    pub fn finished(&self) -> bool {
        self.mission.as_ref().is_some_and(|m| !m.running())
    }

    pub fn next_shot_tick(&self) -> Result<u64, String> {
        self.snapshot().next_shot_tick()
    }

    pub fn aim(&self) -> Result<crate::mission::Solution, String> {
        self.snapshot().aim()
    }

    pub fn preview_shot(&self, target: Vector2) -> Result<crate::mission::Solution, String> {
        let tick = self.next_shot_tick()?;
        self.snapshot().preview_shot_at(target, tick)
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            world: &self.world,
            mission: self.mission.as_ref(),
            initial_mission: self.initial_mission.as_ref(),
            initial_delta_v: self
                .initial
                .ships
                .iter()
                .find(|ship| ship.id == self.player_id)
                .unwrap()
                .remaining_dv,
            scale: crate::scenario::SCALE,
            player_id: self.player_id,
            phase: self.phase,
            paused: self.paused,
            stop_at_boundary: self.stop_at_boundary,
            speed: self.speed,
            planned_burn: self.planned_burn,
            active_burn: self.active_burn,
            time_seconds: self.world.tick as f64 * STEP_SECONDS,
            remaining_seconds: if self.phase == Phase::Planning {
                INTERVAL_SECONDS
            } else {
                (self.interval_end - self.world.tick) as f64 * STEP_SECONDS
            },
            order_start_tick: self.interval_end,
            order_start_seconds: self.interval_end as f64 * STEP_SECONDS,
            command_number: self.command_log.len() + usize::from(self.phase == Phase::Planning),
            command_log: &self.command_log,
            run_kind: self.run_kind(),
            parameter_edits: &self.parameter_edits,
        }
    }

    pub fn dispatch(&mut self, command: Command) -> Result<(), String> {
        if self.finished()
            && !matches!(
                command,
                Command::Reset
                    | Command::StartMission
                    | Command::Sandbox
                    | Command::Pause
                    | Command::SetSpeed(_)
            )
        {
            return Err("Mission is over; retry to issue orders.".into());
        }
        match command {
            Command::Configure(parameter, value) => self.configure(parameter, value)?,
            Command::QueueLaunchShot(target, seconds) => {
                let solution = self.snapshot().preview_launch_shot(target, seconds)?;
                let ship = self.snapshot().planning_ship_at(solution.launch_tick);
                self.mission
                    .as_mut()
                    .unwrap()
                    .queue(&ship, target, solution.launch_tick)?;
            }
            Command::QueueTimedShot(target, arrival) => {
                let solution = self.snapshot().preview_timed_shot(target, arrival)?;
                let ship = self.snapshot().planning_ship_at(solution.launch_tick);
                let mission = self.mission.as_mut().unwrap();
                mission.queue(&ship, target, solution.launch_tick)?;
                mission
                    .queued
                    .iter_mut()
                    .find(|order| order.launch_tick == solution.launch_tick)
                    .unwrap()
                    .requested_arrival_seconds = Some(arrival);
            }
            Command::QueueShot(target) => {
                let tick = self.next_shot_tick()?;
                let ship = self.snapshot().planning_ship_at(tick);
                self.mission
                    .as_mut()
                    .ok_or("Start a blockade mission to use the gun.")?
                    .queue(&ship, target, tick)?;
            }
            Command::CancelShot => {
                self.mission
                    .as_mut()
                    .ok_or("No active mission.")?
                    .queued
                    .retain(|order| {
                        order.planning_error.is_none() && order.launch_tick < self.interval_end
                    });
            }
            Command::CancelQueuedShot(id) => {
                self.mission
                    .as_mut()
                    .ok_or("No active mission.")?
                    .cancel(id, self.interval_end)?;
            }
            Command::StartMission => *self = Self::blockade(),
            Command::Sandbox => *self = Self::default(),
            Command::PlanArrival(target) => {
                let ship = self.snapshot().planning_ship();
                self.planned_burn = crate::maneuver::solve(&ship, target)?;
            }
            Command::PlanBurn(requested) => {
                if !requested.valid() {
                    return Err("Acceleration must be finite and within bounds.".into());
                }
                let ship = self
                    .world
                    .ships
                    .iter()
                    .find(|s| s.id == self.player_id)
                    .unwrap();
                self.planned_burn = requested.clamped(ship.max_acceleration);
            }
            Command::Commit | Command::Resume => {
                if self.phase == Phase::Planning {
                    self.begin_interval()?;
                }
                self.paused = false;
                self.stop_at_boundary = matches!(command, Command::Commit);
            }
            Command::Pause => {
                self.paused = true;
                self.stop_at_boundary = false;
                self.playback_quanta = 0;
            }
            Command::Step | Command::StepForward => {
                if !self.paused {
                    return Err("Pause execution before stepping.".into());
                }
                if self.phase == Phase::Planning {
                    self.begin_interval()?;
                }
                let ticks = if matches!(command, Command::StepForward) {
                    STEP_FORWARD_TICKS
                } else {
                    1
                };
                for _ in 0..ticks.min(self.interval_end - self.world.tick) {
                    if self.finished() {
                        break;
                    }
                    self.advance_tick()?;
                }
            }
            Command::NextTurn => {
                if self.phase == Phase::Planning {
                    self.begin_interval()?;
                }
                self.paused = true;
                self.playback_quanta = 0;
                self.stop_at_boundary = false;
                // Complete every physical tick and stop before starting another interval.
                while self.world.tick < self.interval_end && !self.finished() {
                    self.advance_tick()?;
                }
            }
            Command::SetSpeed(speed) => {
                if ![1.0, 32.0, 64.0, 128.0].contains(&speed) {
                    return Err("Playback speed must be 1, 32, 64, or 128.".into());
                }
                self.speed = speed as u32;
            }
            Command::Reset => {
                *self = if let Some(config) = self.initial_mission.clone() {
                    Self::with_mission(self.initial.clone(), self.player_id, config)
                } else {
                    Self::new(self.initial.clone(), self.player_id)
                }
                .expect("validated initial scenario");
            }
        }
        if matches!(
            command,
            Command::PlanBurn(_)
                | Command::PlanArrival(_)
                | Command::CancelShot
                | Command::CancelQueuedShot(_)
        ) {
            self.replan_timed_shots();
        }
        Ok(())
    }

    fn run_kind(&self) -> RunKind {
        if self.parameter_edits.iter().any(|edit| edit.during_play) {
            RunKind::Debug
        } else if self.initial_mission.as_ref() != Some(&crate::mission::Config::default())
            || self.initial != crate::scenario::blockade_world()
        {
            RunKind::Custom
        } else {
            RunKind::Standard
        }
    }

    fn configure(&mut self, parameter: Parameter, value: f64) -> Result<(), String> {
        if !self.paused {
            return Err("Pause execution before changing parameters.".into());
        }
        let mission = self
            .mission
            .as_ref()
            .ok_or("Start a blockade mission to configure parameters.")?;
        let during_play = self.world.tick != self.initial.tick || !self.command_log.is_empty();
        if parameter.initial_only() && (during_play || !mission.queued.is_empty()) {
            return Err(
                "Change initial parameters before the first command and with an empty shot queue."
                    .into(),
            );
        }
        let mut config = mission.config.clone();
        parameter.apply(&mut config, value)?;
        if parameter.initial_only() {
            crate::mission::Mission::new(&self.world, self.player_id, config.clone())?;
        }
        if config == mission.config {
            return Ok(());
        }
        // Only initial-only edits affect scheduling. They require an empty queue before play.
        // Policy changes preserve observations, flying rounds, and all frozen orders.
        let mission = self.mission.as_mut().unwrap();
        if config.policy != mission.config.policy {
            mission.decision.hold_until = self.world.tick;
        }
        mission.config = config.clone();
        if !during_play {
            self.initial_mission = Some(config);
        }
        self.parameter_edits.push(Edit {
            tick: self.world.tick,
            parameter,
            value,
            during_play,
        });
        Ok(())
    }

    fn replan_timed_shots(&mut self) {
        let Some(mut slots) = self.mission.clone() else {
            return;
        };
        let mut timed: Vec<_> = slots
            .queued
            .iter()
            .filter(|order| {
                order.requested_arrival_seconds.is_some()
                    && (order.planning_error.is_some() || order.launch_tick >= self.interval_end)
            })
            .cloned()
            .collect();
        if timed.is_empty() {
            return;
        }
        slots
            .queued
            .retain(|order| !timed.iter().any(|candidate| candidate.id == order.id));
        // Earlier reservations retain priority. Fixed launch slots and active orders are anchors.
        timed.sort_by_key(|order| order.id);
        for mut order in timed {
            match self.snapshot().solve_timed_shot(
                order.target,
                order.requested_arrival_seconds.unwrap(),
                &slots,
            ) {
                Ok(solution) => {
                    order.launch_tick = solution.launch_tick;
                    order.planning_error = None;
                }
                Err(error) => {
                    order.planning_error = Some(error);
                }
            }
            slots.queued.push(order);
        }
        slots
            .queued
            .sort_by_key(|order| (order.planning_error.is_some(), order.launch_tick));
        self.mission = Some(slots);
    }

    /// Consume one presentation-clock pulse, without changing the physical timestep.
    pub fn pulse(&mut self) {
        if self.paused {
            return;
        }
        self.playback_quanta += self.speed;
        while self.playback_quanta >= PULSES_PER_TICK && !self.paused {
            if self.phase == Phase::Planning && self.begin_interval().is_err() {
                self.paused = true;
                self.playback_quanta = 0;
                return;
            }
            self.playback_quanta -= PULSES_PER_TICK;
            if self.advance_tick().is_err() {
                self.paused = true;
                self.playback_quanta = 0;
                return;
            }
        }
    }

    fn advance_tick(&mut self) -> Result<(), String> {
        if let Some(mission) = &mut self.mission {
            // Fire before this tick's acceleration and sensor sample at every reload slot.
            mission.launch_queued(&mut self.world, self.player_id)?;
            mission.step(&mut self.world, self.player_id, self.active_burn);
            if !mission.running() {
                self.paused = true;
                self.stop_at_boundary = false;
                self.playback_quanta = 0;
            }
        } else {
            self.world
                .step(&[Burn {
                    ship_id: self.player_id,
                    acceleration: self.active_burn,
                }])
                .expect("frozen valid orders and prevalidated tick capacity");
        }
        if self.world.tick == self.interval_end {
            self.phase = Phase::Planning;
            self.active_burn = Vector2::ZERO;
            if self.stop_at_boundary {
                self.paused = true;
                self.stop_at_boundary = false;
                self.playback_quanta = 0;
            }
        }
        Ok(())
    }

    fn begin_interval(&mut self) -> Result<(), String> {
        if let Some(order) = self
            .mission
            .as_ref()
            .and_then(|m| m.queued.iter().find(|o| o.planning_error.is_some()))
        {
            return Err(format!(
                "Shot #{}: {} Cancel or revise the maneuver before continuing.",
                order.id,
                order.planning_error.as_ref().unwrap()
            ));
        }
        let end = self
            .world
            .tick
            .checked_add(INTERVAL_TICKS)
            .ok_or("Simulation tick capacity exhausted.")?;
        let shots: Vec<_> = self
            .mission
            .as_ref()
            .map(|mission| {
                mission
                    .queued
                    .iter()
                    .filter(|order| order.launch_tick < end)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        // Validate the complete frozen firing plan before committing any part of it.
        for order in &shots {
            self.snapshot().preview_order(order)?;
        }
        if let Some(mission) = &mut self.mission {
            mission.launch_queued(&mut self.world, self.player_id)?;
        }
        self.active_burn = self.planned_burn;
        self.interval_end = end;
        self.command_log.push(CommandRecord {
            shots,
            start_tick: self.world.tick,
            burn: Burn {
                ship_id: self.player_id,
                acceleration: self.active_burn,
            },
        });
        self.phase = Phase::Executing;
        Ok(())
    }
}

impl Snapshot<'_> {
    /// Shared planning entry points for commands, Lua, and the normal UI.
    pub fn next_shot_tick(&self) -> Result<u64, String> {
        self.mission
            .ok_or("Start a blockade mission to use the gun.")?
            .next_launch_tick(self.order_start_tick)
    }

    pub fn aim(&self) -> Result<crate::mission::Solution, String> {
        let tick = self.next_shot_tick()?;
        let ship = self.planning_ship_at(tick);
        self.mission
            .unwrap()
            .aim(&ship, tick)
            .map(|s| self.cover_solution(s))
    }

    pub fn preview_shot_at(
        &self,
        target: Vector2,
        tick: u64,
    ) -> Result<crate::mission::Solution, String> {
        let ship = self.planning_ship_at(tick);
        if ship.disabled {
            return Err("Firing ship will be disabled by an asteroid before launch.".into());
        }
        crate::mission::solve_shot(&ship, target, tick).map(|s| self.cover_solution(s))
    }

    pub fn presentation_world(&self) -> World {
        self.mission.map_or_else(
            || self.world.clone(),
            |mission| mission.presentation_world(self.world),
        )
    }

    /// Predict the next command boundary without changing the current orders.
    pub fn planning_ship(&self) -> crate::physics::Ship {
        self.planning_ship_at(self.order_start_tick)
    }

    /// Predict any future tick using the active burn until the boundary, then the draft.
    pub fn planning_ship_at(&self, tick: u64) -> crate::physics::Ship {
        let mut ship = self
            .world
            .ships
            .iter()
            .find(|s| s.id == self.player_id)
            .unwrap()
            .clone();
        for t in self.world.tick..tick {
            self.advance_planning_ship(&mut ship, t);
        }
        ship
    }

    /// Preview the queued interval, including the frozen remainder as a route prefix.
    pub fn planned_route(&self, acceleration: Vector2) -> crate::maneuver::Preview {
        let terrain: Vec<_> = self
            .world
            .asteroids
            .iter()
            .cloned()
            .map(|mut a| {
                a.position =
                    a.position + a.velocity * (self.order_start_seconds - self.time_seconds);
                a
            })
            .collect();
        let mut route = crate::maneuver::preview_ticks_in_terrain(
            &self.planning_ship(),
            acceleration,
            crate::timing::INTERVAL_TICKS,
            &terrain,
        );
        let ship = self
            .world
            .ships
            .iter()
            .find(|s| s.id == self.player_id)
            .unwrap();
        route.forecast = Some(crate::maneuver::forecast_in_terrain(
            ship,
            if self.phase == Phase::Executing {
                self.active_burn
            } else {
                acceleration
            },
            acceleration,
            (self.remaining_seconds / STEP_SECONDS) as u64,
            &self.world.asteroids,
        ));
        if let Some(hit) = route.forecast.as_mut().and_then(|f| f.collision.as_mut()) {
            hit.seconds += self.time_seconds;
        }
        if self.phase == Phase::Executing {
            let mut prefix = crate::maneuver::preview_ticks_in_terrain(
                ship,
                self.active_burn,
                (self.remaining_seconds / STEP_SECONDS) as u64,
                &self.world.asteroids,
            )
            .points;
            prefix.pop();
            prefix.append(&mut route.points);
            route.points = prefix;
        }
        route
    }
}
