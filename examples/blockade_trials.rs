//! Compare shot placement using only observed lead points for targeting.
use phase_vector::{
    physics::Vector2 as V,
    session::{Command as C, Session},
    units::METRES_PER_KILOMETRE,
};

fn main() {
    for (name, firing_turns, spacing_km) in [
        ("No attack", 0, 0.0),
        ("Lead / 16 rounds", 4, 0.0),
        ("Continuous lead salvos", usize::MAX, 0.0),
        ("Spread / 12 rounds", 3, 900.0),
        ("Spread / 16 rounds", 4, 300.0),
    ] {
        let mut s = Session::blockade();
        let mut turn = 0;
        while !s.finished() {
            if turn < firing_turns {
                for round in 0..4 {
                    let offset = V::new(0.0, round as f64 * spacing_km * METRES_PER_KILOMETRE);
                    s.dispatch(C::QueueShot(s.aim().unwrap().target + offset))
                        .unwrap();
                }
            }
            s.dispatch(C::NextTurn).unwrap();
            turn += 1;
        }
        let m = s.mission().unwrap();
        println!(
            "{name}: outcome={:?} time={:.3}s shots={} hits={} enemy_dv={:.2}km/s score={}",
            m.outcome,
            m.ended_seconds.unwrap(),
            m.shots.len(),
            s.world().impacts.len(),
            s.world().ships[1].spent_dv / METRES_PER_KILOMETRE,
            phase_vector::evaluation::evaluate(&s.snapshot())
                .unwrap()
                .total,
        );
    }
}
