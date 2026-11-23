//! Renderer-independent, fixed-step inertial motion and swept projectile contacts.

use std::ops::{Add, Mul, Sub};

use serde::Serialize;

pub use crate::timing::STEP_SECONDS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

impl Vector2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    pub fn clamped(self, maximum: f64) -> Self {
        let length = self.length();
        if length > maximum {
            self * (maximum / length)
        } else {
            self
        }
    }

    pub fn valid(self) -> bool {
        valid_number(self.x) && valid_number(self.y)
    }

    fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }
}

impl Add for Vector2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vector2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f64> for Vector2 {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

// Keep externally supplied values far below overflow in collision products.
fn valid_number(value: f64) -> bool {
    value.is_finite() && value.abs() <= 1.0e12
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Ship {
    pub id: u32,
    pub position: Vector2,
    pub velocity: Vector2,
    pub radius: f64,
    pub max_acceleration: f64,
    pub remaining_dv: f64,
    pub spent_dv: f64,
    pub disabled: bool,
}

impl Ship {
    pub fn new(
        id: u32,
        position: Vector2,
        velocity: Vector2,
        radius: f64,
        max_acceleration: f64,
        delta_v: f64,
    ) -> Result<Self, String> {
        if !position.valid()
            || !velocity.valid()
            || [radius, max_acceleration, delta_v]
                .iter()
                .any(|v| !valid_number(*v) || *v < 0.0)
        {
            return Err(
                "Ship parameters must be finite, within bounds, and nonnegative where applicable."
                    .into(),
            );
        }
        Ok(Self {
            id,
            position,
            velocity,
            radius,
            max_acceleration,
            remaining_dv: delta_v,
            spent_dv: 0.0,
            disabled: false,
        })
    }

    pub(crate) fn advance_in_terrain(
        &mut self,
        acceleration: Vector2,
        asteroids: &[Asteroid],
        offset: f64,
    ) -> Option<AsteroidHit> {
        let start = self.position;
        let disabled = self.disabled;
        self.advance(acceleration);
        if disabled {
            return None;
        }
        let hit = asteroid_contact(
            asteroids,
            start,
            self.velocity,
            offset,
            STEP_SECONDS,
            self.radius,
        )?;
        self.disabled = true;
        Some(hit)
    }

    pub(crate) fn advance(&mut self, acceleration: Vector2) {
        let acceleration = if self.disabled {
            Vector2::ZERO
        } else {
            acceleration
        };
        let requested = acceleration.clamped(self.max_acceleration) * STEP_SECONDS;
        let magnitude = requested.length();
        let consumed = magnitude.min(self.remaining_dv);
        if magnitude > 0.0 {
            self.velocity = self.velocity + requested * (consumed / magnitude);
        }
        self.remaining_dv = (self.remaining_dv - consumed).max(0.0);
        self.spent_dv += consumed;
        // Semi-implicit Euler: kick once, then drift linearly for this whole tick.
        self.position = self.position + self.velocity * STEP_SECONDS;
    }
}

/// Known, indestructible circular terrain. Positions use the world's current tick.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Asteroid {
    pub id: u32,
    pub position: Vector2,
    pub velocity: Vector2,
    pub radius: f64,
}
impl Asteroid {
    pub fn new(id: u32, position: Vector2, velocity: Vector2, radius: f64) -> Result<Self, String> {
        if !position.valid() || !velocity.valid() || !valid_number(radius) || radius <= 0.0 {
            return Err(
                "Asteroid position, velocity and positive radius must be finite and within bounds."
                    .into(),
            );
        }
        Ok(Self {
            id,
            position,
            velocity,
            radius,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AsteroidHit {
    pub asteroid_id: u32,
    /// Seconds from the start of the tested segment.
    pub seconds: f64,
    pub position: Vector2,
}

/// Shared swept terrain query for physics, ballistic previews, and observed AI threats.
pub fn asteroid_contact(
    asteroids: &[Asteroid],
    origin: Vector2,
    velocity: Vector2,
    start_seconds: f64,
    duration: f64,
    radius: f64,
) -> Option<AsteroidHit> {
    asteroids
        .iter()
        .filter_map(|asteroid| {
            let start = asteroid.position + asteroid.velocity * start_seconds;
            swept_contact(
                origin,
                origin + velocity * duration,
                start,
                start + asteroid.velocity * duration,
                radius + asteroid.radius,
            )
            .map(|fraction| AsteroidHit {
                asteroid_id: asteroid.id,
                seconds: fraction * duration,
                position: origin + velocity * (fraction * duration),
            })
        })
        .min_by(|a, b| {
            a.seconds
                .total_cmp(&b.seconds)
                .then(a.asteroid_id.cmp(&b.asteroid_id))
        })
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AsteroidImpact {
    pub tick: u64,
    pub fraction: f64,
    pub asteroid_id: u32,
    pub projectile_id: Option<u64>,
    pub ship_id: Option<u32>,
    pub position: Vector2,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Burn {
    pub ship_id: u32,
    pub acceleration: Vector2,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Projectile {
    pub id: u64,
    pub owner: u32,
    pub position: Vector2,
    pub velocity: Vector2,
    pub radius: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Impact {
    pub tick: u64,
    pub fraction: f64,
    pub projectile_id: u64,
    pub ship_id: u32,
    pub position: Vector2,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct World {
    pub tick: u64,
    pub ships: Vec<Ship>,
    pub projectiles: Vec<Projectile>,
    pub impacts: Vec<Impact>,
    pub asteroids: Vec<Asteroid>,
    pub asteroid_impacts: Vec<AsteroidImpact>,
    next_projectile_id: u64,
}

impl World {
    pub(crate) fn next_projectile_id(&self) -> u64 {
        self.next_projectile_id
    }

    pub fn new(ships: Vec<Ship>) -> Result<Self, String> {
        for (index, ship) in ships.iter().enumerate() {
            if ships[..index].iter().any(|other| other.id == ship.id) {
                return Err("Ship IDs must be unique.".into());
            }
        }
        Ok(Self {
            tick: 0,
            ships,
            projectiles: Vec::new(),
            impacts: Vec::new(),
            asteroids: Vec::new(),
            asteroid_impacts: Vec::new(),
            next_projectile_id: 0,
        })
    }

    pub fn launch(
        &mut self,
        owner: u32,
        position: Vector2,
        velocity: Vector2,
        radius: f64,
    ) -> Result<u64, String> {
        if !position.valid() || !velocity.valid() || !valid_number(radius) || radius < 0.0 {
            return Err("Projectile parameters must be finite and within bounds; radius must be nonnegative.".into());
        }
        let id = self.next_projectile_id;
        let next = id
            .checked_add(1)
            .ok_or("Projectile ID capacity exhausted.")?;
        self.projectiles.push(Projectile {
            id,
            owner,
            position,
            velocity,
            radius,
        });
        self.next_projectile_id = next;
        Ok(id)
    }

    pub fn step(&mut self, burns: &[Burn]) -> Result<(), String> {
        let next_tick = self
            .tick
            .checked_add(1)
            .ok_or("Simulation tick capacity exhausted.")?;
        for (index, burn) in burns.iter().enumerate() {
            if !burn.acceleration.valid()
                || !self.ships.iter().any(|s| s.id == burn.ship_id)
                || burns[..index].iter().any(|b| b.ship_id == burn.ship_id)
            {
                return Err(
                    "Burns require a known, unique ship ID and finite acceleration within bounds."
                        .into(),
                );
            }
        }
        let previous = self.ships.clone();
        for ship in &mut self.ships {
            let acceleration = burns
                .iter()
                .find(|b| b.ship_id == ship.id)
                .map_or(Vector2::ZERO, |b| b.acceleration);
            ship.advance(acceleration);
        }
        let mut terrain_contacts = Vec::new();
        for (old, ship) in previous.iter().zip(&mut self.ships) {
            if !old.disabled
                && let Some(hit) = asteroid_contact(
                    &self.asteroids,
                    old.position,
                    ship.velocity,
                    0.0,
                    STEP_SECONDS,
                    ship.radius,
                )
            {
                ship.disabled = true;
                terrain_contacts.push(AsteroidImpact {
                    tick: self.tick,
                    fraction: hit.seconds / STEP_SECONDS,
                    asteroid_id: hit.asteroid_id,
                    projectile_id: None,
                    ship_id: Some(ship.id),
                    position: hit.position,
                });
            }
        }
        let mut contacts = Vec::new();
        self.projectiles.retain_mut(|projectile| {
            let start = projectile.position;
            let end = start + projectile.velocity * STEP_SECONDS;
            let first = previous
                .iter()
                .zip(&self.ships)
                .filter(|(_, ship)| ship.id != projectile.owner)
                .filter_map(|(old, ship)| {
                    swept_contact(
                        start,
                        end,
                        old.position,
                        ship.position,
                        projectile.radius + ship.radius,
                    )
                    .map(|fraction| (fraction, ship.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let obstacle = asteroid_contact(
                &self.asteroids,
                start,
                projectile.velocity,
                0.0,
                STEP_SECONDS,
                projectile.radius,
            );
            // Terrain wins exact ties: a round touching cover cannot penetrate it.
            if let Some(hit) = obstacle.filter(|hit| {
                first.is_none_or(|(fraction, _)| hit.seconds / STEP_SECONDS <= fraction)
            }) {
                terrain_contacts.push(AsteroidImpact {
                    tick: self.tick,
                    fraction: hit.seconds / STEP_SECONDS,
                    asteroid_id: hit.asteroid_id,
                    projectile_id: Some(projectile.id),
                    ship_id: None,
                    position: hit.position,
                });
                false
            } else if let Some((fraction, ship_id)) = first {
                contacts.push(Impact {
                    tick: self.tick,
                    fraction,
                    projectile_id: projectile.id,
                    ship_id,
                    position: start + (end - start) * fraction,
                });
                false
            } else {
                projectile.position = end;
                true
            }
        });
        contacts.sort_by(|a, b| {
            a.fraction
                .total_cmp(&b.fraction)
                .then(a.projectile_id.cmp(&b.projectile_id))
                .then(a.ship_id.cmp(&b.ship_id))
        });
        self.impacts.extend(contacts);
        terrain_contacts.sort_by(|a, b| {
            a.fraction
                .total_cmp(&b.fraction)
                .then(a.asteroid_id.cmp(&b.asteroid_id))
                .then(a.ship_id.cmp(&b.ship_id))
                .then(a.projectile_id.cmp(&b.projectile_id))
        });
        self.asteroid_impacts.extend(terrain_contacts);
        for asteroid in &mut self.asteroids {
            asteroid.position = asteroid.position + asteroid.velocity * STEP_SECONDS;
        }
        self.tick = next_tick;
        Ok(())
    }
}

/// Earliest contact in [0, 1] for two circles drifting during the same tick.
/// Spatially crossing paths alone are insufficient: the relative motion must meet.
pub fn swept_contact(
    a0: Vector2,
    a1: Vector2,
    b0: Vector2,
    b1: Vector2,
    radius: f64,
) -> Option<f64> {
    let relative = a0 - b0;
    let drift = (a1 - a0) - (b1 - b0);
    if relative.length() <= radius {
        return Some(0.0);
    }
    let length = drift.length();
    if length == 0.0 {
        return None;
    }
    let direction = drift * (1.0 / length);
    let approach = -relative.dot(direction);
    if approach <= 0.0 {
        return None;
    }
    // A closest-approach calculation avoids subtracting astronomical squared
    // distances in the quadratic discriminant, which can erase small radii.
    let miss_distance = (relative.x * direction.y - relative.y * direction.x).abs();
    if miss_distance > radius {
        return None;
    }
    let half_chord = ((radius - miss_distance) * (radius + miss_distance)).sqrt();
    let fraction = (approach - half_chord) / length;
    (0.0..=1.0).contains(&fraction).then_some(fraction)
}
