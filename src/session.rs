//! WEGO command boundaries and playback scheduling, independent of the renderer.

use serde::Serialize;

use crate::physics::{Burn, STEP_SECONDS, Vector2, World};

pub const INTERVAL_TICKS: u64 = 480;
pub const INTERVAL_SECONDS: f64 = INTERVAL_TICKS as f64 * STEP_SECONDS;
// The browser calls pulse at 60 Hz. Physics advances at 4 Hz at normal speed.
pub const PULSES_PER_TICK: u32 = 15;
pub const DEFAULT_SPEED: u32 = 32;
// Ten simulated seconds at the fixed 0.25-second physics timestep.
const STEP_FORWARD_TICKS: u64 = 40;

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
    CancelShot,
    StartMission,
    Sandbox,
    Commit,
    Pause,
    Resume,
    Step,
    StepForward,
    NextTurn,
    SetSpeed(f64),
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
            "next_turn" => Ok(Self::NextTurn),
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
    pub shot_target: Option<Vector2>,
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
    planned_burn: Vector2,
    active_burn: Vector2,
    interval_end: u64,
    speed: u32,
    playback_quanta: u32,
    command_log: Vec<CommandRecord>,
}

#[derive(Serialize)]
pub struct Snapshot<'a> {
    pub world: &'a World,
    #[serde(skip)]
    pub mission: Option<&'a crate::mission::Mission>,
    pub scale: crate::scenario::Scale,
    pub player_id: u32,
    pub phase: Phase,
    pub paused: bool,
    pub speed: u32,
    pub planned_burn: Vector2,
    pub active_burn: Vector2,
    pub time_seconds: f64,
    pub remaining_seconds: f64,
    pub order_start_seconds: f64,
    pub command_number: usize,
    pub command_log: &'a [CommandRecord],
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
            planned_burn: Vector2::ZERO,
            active_burn: Vector2::ZERO,
            speed: DEFAULT_SPEED,
            playback_quanta: 0,
            command_log: Vec::new(),
        })
    }

    pub fn blockade() -> Self {
        Self::with_mission(
            crate::scenario::initial_world(),
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

    pub fn aim(&self) -> Result<crate::mission::Solution, String> {
        self.mission
            .as_ref()
            .ok_or("Start a blockade mission to use the gun.")?
            .aim(&self.snapshot().planning_ship(), self.interval_end)
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            world: &self.world,
            mission: self.mission.as_ref(),
            scale: crate::scenario::SCALE,
            player_id: self.player_id,
            phase: self.phase,
            paused: self.paused,
            speed: self.speed,
            planned_burn: self.planned_burn,
            active_burn: self.active_burn,
            time_seconds: self.world.tick as f64 * STEP_SECONDS,
            remaining_seconds: if self.phase == Phase::Planning {
                INTERVAL_SECONDS
            } else {
                (self.interval_end - self.world.tick) as f64 * STEP_SECONDS
            },
            order_start_seconds: self.interval_end as f64 * STEP_SECONDS,
            command_number: self.command_log.len() + usize::from(self.phase == Phase::Planning),
            command_log: &self.command_log,
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
            Command::QueueShot(target) => {
                let ship = self.snapshot().planning_ship();
                self.mission
                    .as_mut()
                    .ok_or("Start a blockade mission to use the gun.")?
                    .queue(&ship, target, self.interval_end)?;
            }
            Command::CancelShot => {
                self.mission.as_mut().ok_or("No active mission.")?.queued = None;
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
            }
            Command::Pause => {
                self.paused = true;
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
                    self.advance_tick();
                }
            }
            Command::NextTurn => {
                if self.phase == Phase::Planning {
                    self.begin_interval()?;
                }
                self.paused = true;
                self.playback_quanta = 0;
                // Complete every physical tick and stop before starting another interval.
                while self.world.tick < self.interval_end && !self.finished() {
                    self.advance_tick();
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
        Ok(())
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
            self.advance_tick();
        }
    }

    fn advance_tick(&mut self) {
        if let Some(mission) = &mut self.mission {
            mission.step(&mut self.world, self.player_id, self.active_burn);
            if !mission.running() {
                self.paused = true;
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
        }
    }

    fn begin_interval(&mut self) -> Result<(), String> {
        let end = self
            .world
            .tick
            .checked_add(INTERVAL_TICKS)
            .ok_or("Simulation tick capacity exhausted.")?;
        let shot_target = if let Some(mission) = &mut self.mission {
            mission.launch_queued(&mut self.world, self.player_id)?
        } else {
            None
        };
        self.active_burn = self.planned_burn;
        self.interval_end = end;
        self.command_log.push(CommandRecord {
            shot_target,
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
    pub fn presentation_world(&self) -> World {
        self.mission.map_or_else(
            || self.world.clone(),
            |mission| mission.presentation_world(self.world),
        )
    }

    /// Predict the next command boundary without changing the current orders.
    pub fn planning_ship(&self) -> crate::physics::Ship {
        let mut ship = self
            .world
            .ships
            .iter()
            .find(|s| s.id == self.player_id)
            .unwrap()
            .clone();
        if self.phase == Phase::Executing {
            for _ in 0..(self.remaining_seconds / STEP_SECONDS) as u64 {
                ship.advance(self.active_burn);
            }
        }
        ship
    }

    /// Preview the queued interval, including the frozen remainder as a route prefix.
    pub fn planned_route(&self, acceleration: Vector2) -> crate::maneuver::Preview {
        let mut route = crate::maneuver::preview(&self.planning_ship(), acceleration);
        let ship = self
            .world
            .ships
            .iter()
            .find(|s| s.id == self.player_id)
            .unwrap();
        route.forecast = Some(crate::maneuver::forecast(
            ship,
            if self.phase == Phase::Executing {
                self.active_burn
            } else {
                acceleration
            },
            acceleration,
            (self.remaining_seconds / STEP_SECONDS) as u64,
        ));
        if self.phase == Phase::Executing {
            let mut prefix = crate::maneuver::preview_ticks(
                ship,
                self.active_burn,
                (self.remaining_seconds / STEP_SECONDS) as u64,
            )
            .points;
            prefix.pop();
            prefix.append(&mut route.points);
            route.points = prefix;
        }
        route
    }
}
