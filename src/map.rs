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
    auto_zoom: bool,
    fit_pending: bool,
}

#[derive(Serialize)]
pub struct MapFrame {
    pub focus: Focus,
    pub auto_zoom: bool,
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
            auto_zoom: true,
            fit_pending: true,
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
        self.auto_zoom = matches!(focus, Focus::Overview);
        self.fit_pending = self.auto_zoom;
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
        self.auto_zoom = false;
        self.fit_pending = false;
        Ok(())
    }

    /// Adapt presentation scale only; seconds is real frame time, not simulation time.
    pub fn update_auto_zoom(&mut self, world: &World, player_id: u32, aspect: f64, seconds: f64) {
        if !self.auto_zoom
            || !aspect.is_finite()
            || aspect <= 0.0
            || !seconds.is_finite()
            || seconds < 0.0
        {
            return;
        }
        let (friendly, contact) = positions(world, player_id);
        let separation = contact - friendly;
        let half_width = 1000_f64.max(600.0 * aspect);
        let half_height = 600_f64.max(1000.0 / aspect);
        // Fit an ellipse inside the viewport, leaving room for ship symbols.
        // This changes continuously even when separation grows diagonally.
        let extent = (separation.x / (2.0 * half_width)).hypot(separation.y / (2.0 * half_height));
        let target = (extent / 0.65).clamp(MIN_METRES_PER_UNIT, MAX_METRES_PER_UNIT);
        if self.fit_pending {
            self.metres_per_unit = target;
            self.fit_pending = false;
        } else {
            let blend = 1.0 - (-seconds / 0.35).exp();
            self.metres_per_unit = (self.metres_per_unit.ln()
                + (target.ln() - self.metres_per_unit.ln()) * blend)
                .exp();
            // A skipped turn or resize must not leave either ship outside the map.
            self.metres_per_unit = self
                .metres_per_unit
                .max(extent / 0.90)
                .clamp(MIN_METRES_PER_UNIT, MAX_METRES_PER_UNIT);
            if (self.metres_per_unit / target - 1.0).abs() < 1e-6 {
                self.metres_per_unit = target;
            }
        }
    }

    pub fn frame(&self, world: &World, player_id: u32) -> MapFrame {
        let (friendly, contact) = positions(world, player_id);
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
            auto_zoom: self.auto_zoom,
            center,
            metres_per_unit: self.metres_per_unit,
            grid_metres: decade * multiple,
            can_zoom_in: self.metres_per_unit > MIN_METRES_PER_UNIT,
            can_zoom_out: self.metres_per_unit < MAX_METRES_PER_UNIT,
        }
    }
}

fn positions(world: &World, player_id: u32) -> (Vector2, Vector2) {
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
    (friendly, contact)
}

impl MapFrame {
    pub fn project(&self, position: Vector2) -> Vector2 {
        // Subtract the large origin in f64 before the renderer converts to f32.
        (position - self.center) * (1.0 / self.metres_per_unit)
    }
}
