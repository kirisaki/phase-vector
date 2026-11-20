//! Delayed sensor samples. Controllers receive observations, never pending packets.

use crate::{
    physics::{Projectile, STEP_SECONDS, Ship, Vector2, World},
    timing::seconds_to_ticks,
    units::LIGHT_SPEED,
};
use serde::Serialize;

pub const SAMPLE_TICKS: u64 = seconds_to_ticks(1);
pub const TRACK_RETENTION_TICKS: u64 = seconds_to_ticks(300);

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Track {
    pub id: u64,
    pub sampled_tick: u64,
    pub received_tick: u64,
    pub position: Vector2,
    pub velocity: Vector2,
    pub radius: f64,
}
impl Track {
    pub fn position_at(&self, tick: u64) -> Vector2 {
        self.position
            + self.velocity * (tick.saturating_sub(self.sampled_tick) as f64 * STEP_SECONDS)
    }
    fn ship(ship: &Ship, tick: u64, receive: u64) -> Self {
        Self {
            id: ship.id as u64,
            sampled_tick: tick,
            received_tick: receive,
            position: ship.position,
            velocity: ship.velocity,
            radius: ship.radius,
        }
    }
    fn projectile(shot: &Projectile, tick: u64, receive: u64) -> Self {
        Self {
            id: shot.id,
            sampled_tick: tick,
            received_tick: receive,
            position: shot.position,
            velocity: shot.velocity,
            radius: shot.radius,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sensors {
    pub contact: Track,
    pub threats: Vec<Track>,
    pending_contact: Vec<Track>,
    pending_threats: Vec<Track>,
}
impl Sensors {
    pub fn new(enemy: &Ship, tick: u64) -> Self {
        // The mission briefing supplies the initial contact fix.
        Self {
            contact: Track::ship(enemy, tick, tick),
            threats: Vec::new(),
            pending_contact: Vec::new(),
            pending_threats: Vec::new(),
        }
    }
    pub fn update(
        &mut self,
        world: &World,
        friendly_id: u32,
        enemy_id: u32,
        processing_ticks: u64,
    ) {
        let now = world.tick;
        self.pending_contact.retain(|packet| {
            if packet.received_tick > now {
                return true;
            }
            if packet.sampled_tick >= self.contact.sampled_tick {
                self.contact = packet.clone();
            }
            false
        });
        self.pending_threats.retain(|packet| {
            if packet.received_tick > now {
                return true;
            }
            if let Some(old) = self.threats.iter_mut().find(|t| t.id == packet.id) {
                if packet.sampled_tick >= old.sampled_tick {
                    *old = packet.clone();
                }
            } else {
                self.threats.push(packet.clone());
            }
            false
        });
        self.threats.sort_by_key(|t| t.id);
        // A missed projectile may leave stale knowledge; expiration uses sample age only.
        self.threats
            .retain(|t| now.saturating_sub(t.sampled_tick) <= TRACK_RETENTION_TICKS);
        if !now.is_multiple_of(SAMPLE_TICKS) {
            return;
        }
        let friendly = world.ships.iter().find(|s| s.id == friendly_id).unwrap();
        let enemy = world.ships.iter().find(|s| s.id == enemy_id).unwrap();
        let delay = |distance: f64| {
            (distance / LIGHT_SPEED / STEP_SECONDS).ceil() as u64 + processing_ticks
        };
        let receive = now + delay((friendly.position - enemy.position).length()).max(1);
        self.pending_contact.push(Track::ship(enemy, now, receive));
        for shot in world.projectiles.iter().filter(|p| p.owner == friendly_id) {
            let receive = now + delay((shot.position - enemy.position).length()).max(1);
            self.pending_threats
                .push(Track::projectile(shot, now, receive));
        }
    }
}
