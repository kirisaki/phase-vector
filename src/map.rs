//! Camera-relative presentation coordinates; the simulation always retains SI units.

use serde::Serialize;

use crate::{
    physics::{Vector2, World},
    scenario::INITIAL_SEPARATION,
};

const LOCAL_METRES_PER_UNIT: f64 = 100_000.0;
const MIN_METRES_PER_UNIT: f64 = 1.0;
const MAX_METRES_PER_UNIT: f64 = 200_000_000.0;

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Focus {
    #[default]
    Overview,
    Friendly,
    Contact,
}

#[derive(Clone, Copy)]
pub struct MapView {
    focus: Focus,
    metres_per_unit: f64,
}

#[derive(Serialize)]
pub struct MapFrame {
    pub focus: Focus,
    pub center: Vector2,
    pub metres_per_unit: f64,
    pub grid_metres: f64,
    pub can_zoom_in: bool,
    pub can_zoom_out: bool,
}

impl Default for MapView {
    fn default() -> Self {
        Self {
            focus: Focus::Overview,
            metres_per_unit: INITIAL_SEPARATION / 1300.0,
        }
    }
}

impl MapView {
    pub fn focus(&mut self, name: &str) -> Result<(), String> {
        let focus = match name {
            "overview" => Focus::Overview,
            "friendly" => Focus::Friendly,
            "contact" => Focus::Contact,
            _ => return Err("Unknown map focus.".into()),
        };
        self.focus = focus;
        self.metres_per_unit = match focus {
            Focus::Overview => Self::default().metres_per_unit,
            _ => LOCAL_METRES_PER_UNIT,
        };
        Ok(())
    }

    pub fn zoom(&mut self, factor: f64) -> Result<(), String> {
        if !factor.is_finite() || !(0.5..=2.0).contains(&factor) {
            return Err("Zoom factor must be between 0.5 and 2.".into());
        }
        self.metres_per_unit =
            (self.metres_per_unit * factor).clamp(MIN_METRES_PER_UNIT, MAX_METRES_PER_UNIT);
        Ok(())
    }

    pub fn frame(&self, world: &World, player_id: u32) -> MapFrame {
        let friendly = world
            .ships
            .iter()
            .find(|s| s.id == player_id)
            .map_or(Vector2::ZERO, |s| s.position);
        let contact = world
            .ships
            .iter()
            .find(|s| s.id != player_id)
            .map_or(friendly, |s| s.position);
        let center = match self.focus {
            Focus::Overview => friendly + (contact - friendly) * 0.5,
            Focus::Friendly => friendly,
            Focus::Contact => contact,
        };
        // Keep the grid near 100 render units, with readable 1/2/5 decade spacing.
        let target = self.metres_per_unit * 100.0;
        let decade = 10_f64.powf(target.log10().floor());
        let fraction = target / decade;
        let multiple = if fraction < 2.0 {
            1.0
        } else if fraction < 5.0 {
            2.0
        } else {
            5.0
        };
        MapFrame {
            focus: self.focus,
            center,
            metres_per_unit: self.metres_per_unit,
            grid_metres: decade * multiple,
            can_zoom_in: self.metres_per_unit > MIN_METRES_PER_UNIT,
            can_zoom_out: self.metres_per_unit < MAX_METRES_PER_UNIT,
        }
    }
}

impl MapFrame {
    pub fn project(&self, position: Vector2) -> Vector2 {
        // Subtract the large origin in f64 before the renderer converts to f32.
        (position - self.center) * (1.0 / self.metres_per_unit)
    }
}
