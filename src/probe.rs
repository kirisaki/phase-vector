//! Small shared command surface for the integration probe, not mission physics.

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProbeState {
    pub paused: bool,
    pub ticks: u32,
}

impl Default for ProbeState {
    fn default() -> Self {
        Self {
            paused: true,
            ticks: 0,
        }
    }
}

impl ProbeState {
    pub fn command(&mut self, name: &str) -> Result<(), String> {
        match name {
            "pause" => self.paused = true,
            "resume" => self.paused = false,
            "step" if self.paused => self.tick(),
            "step" => return Err("Pause the probe before stepping.".into()),
            "reset" => *self = Self::default(),
            _ => return Err(format!("Unknown command: {name}")),
        }
        Ok(())
    }

    pub fn advance(&mut self) {
        if !self.paused {
            self.tick();
        }
    }

    fn tick(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
    }
}
