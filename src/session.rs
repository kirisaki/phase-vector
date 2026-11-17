//! WEGO command boundaries and playback scheduling, independent of the renderer.

use serde::Serialize;

use crate::physics::{Burn, STEP_SECONDS, Ship, Vector2, World};

pub const INTERVAL_TICKS: u64 = 480;
pub const INTERVAL_SECONDS: f64 = INTERVAL_TICKS as f64 * STEP_SECONDS;
// The browser calls pulse at 60 Hz. Physics advances at 4 Hz at normal speed.
pub const PULSES_PER_TICK: u32 = 15;

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
    Commit,
    Pause,
    Resume,
    Step,
    SetSpeed(f64),
    Reset,
}

impl Command {
    pub fn named(name: &str) -> Result<Self, String> {
        match name {
            "commit" => Ok(Self::Commit),
            "pause" => Ok(Self::Pause),
            "resume" => Ok(Self::Resume),
            "step" => Ok(Self::Step),
            "reset" => Ok(Self::Reset),
            _ => Err(format!("Unknown command: {name}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommandRecord {
    pub start_tick: u64,
    pub burn: Burn,
}

#[derive(Clone, Debug)]
pub struct Session {
    world: World,
    initial: World,
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
    pub player_id: u32,
    pub phase: Phase,
    pub paused: bool,
    pub speed: u32,
    pub planned_burn: Vector2,
    pub active_burn: Vector2,
    pub time_seconds: f64,
    pub remaining_seconds: f64,
    pub command_number: usize,
    pub command_log: &'a [CommandRecord],
}

impl Default for Session {
    fn default() -> Self {
        let ships = vec![
            Ship::new(
                1,
                Vector2::new(-600.0, -150.0),
                Vector2::new(1.0, 0.0),
                12.0,
                0.02,
                20.0,
            )
            .unwrap(),
            Ship::new(
                2,
                Vector2::new(200.0, 180.0),
                Vector2::new(-0.3, 0.0),
                12.0,
                0.02,
                20.0,
            )
            .unwrap(),
        ];
        Self::new(World::new(ships).unwrap(), 1).unwrap()
    }
}

impl Session {
    pub fn new(world: World, player_id: u32) -> Result<Self, String> {
        if !world.ships.iter().any(|ship| ship.id == player_id) {
            return Err("The controlled ship must exist.".into());
        }
        Ok(Self {
            initial: world.clone(),
            interval_end: world.tick,
            world,
            player_id,
            phase: Phase::Planning,
            paused: true,
            planned_burn: Vector2::ZERO,
            active_burn: Vector2::ZERO,
            speed: 1,
            playback_quanta: 0,
            command_log: Vec::new(),
        })
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            world: &self.world,
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
            command_number: self.command_log.len() + usize::from(self.phase == Phase::Planning),
            command_log: &self.command_log,
        }
    }

    pub fn dispatch(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::PlanBurn(requested) => {
                self.require_planning()?;
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
            Command::Commit => {
                self.require_planning()?;
                let end = self
                    .world
                    .tick
                    .checked_add(INTERVAL_TICKS)
                    .ok_or("Simulation tick capacity exhausted.")?;
                self.active_burn = self.planned_burn;
                self.interval_end = end;
                self.command_log.push(CommandRecord {
                    start_tick: self.world.tick,
                    burn: Burn {
                        ship_id: self.player_id,
                        acceleration: self.active_burn,
                    },
                });
                self.phase = Phase::Executing;
                self.paused = false;
                self.playback_quanta = 0;
            }
            Command::Pause => {
                self.paused = true;
                self.playback_quanta = 0;
            }
            Command::Resume => {
                self.require_execution()?;
                self.paused = false;
            }
            Command::Step => {
                self.require_execution()?;
                if !self.paused {
                    return Err("Pause execution before stepping.".into());
                }
                self.advance_tick();
            }
            Command::SetSpeed(speed) => {
                if ![1.0, 4.0, 16.0, 64.0].contains(&speed) {
                    return Err("Playback speed must be 1, 4, 16, or 64.".into());
                }
                self.speed = speed as u32;
            }
            Command::Reset => {
                *self = Self::new(self.initial.clone(), self.player_id)
                    .expect("validated initial scenario");
            }
        }
        Ok(())
    }

    /// Consume one presentation-clock pulse, without changing the physical timestep.
    pub fn pulse(&mut self) {
        if self.phase != Phase::Executing || self.paused {
            return;
        }
        self.playback_quanta += self.speed;
        while self.playback_quanta >= PULSES_PER_TICK && self.phase == Phase::Executing {
            self.playback_quanta -= PULSES_PER_TICK;
            self.advance_tick();
        }
    }

    fn advance_tick(&mut self) {
        self.world
            .step(&[Burn {
                ship_id: self.player_id,
                acceleration: self.active_burn,
            }])
            .expect("frozen valid orders and prevalidated tick capacity");
        if self.world.tick == self.interval_end {
            self.phase = Phase::Planning;
            self.paused = true;
            self.planned_burn = Vector2::ZERO;
            self.active_burn = Vector2::ZERO;
            self.playback_quanta = 0;
        }
    }

    fn require_planning(&self) -> Result<(), String> {
        if self.phase == Phase::Planning {
            Ok(())
        } else {
            Err("Orders can only change during planning, even when execution is paused.".into())
        }
    }

    fn require_execution(&self) -> Result<(), String> {
        if self.phase == Phase::Executing {
            Ok(())
        } else {
            Err("Commit orders before resuming or stepping execution.".into())
        }
    }
}
