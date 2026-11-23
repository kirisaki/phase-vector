//! Equal-budget cover trials. Targets use only observed lead points.
use phase_vector::{
    physics::Vector2 as V,
    scenario,
    session::{Command as C, Session},
};

fn main() {
    for (name, burn, start_turn, salvos, spacing) in [
        ("No attack", 0.0, 0, 0, 0.0),
        ("Coast / spread", 0.0, 2, 3, 900_000.0),
        ("Slow 240 s / lead", -scenario::MAX_ACCELERATION, 2, 3, 0.0),
        (
            "Slow 120 s / spread",
            -scenario::MAX_ACCELERATION,
            1,
            3,
            900_000.0,
        ),
        (
            "Slow 240 s / spread",
            -scenario::MAX_ACCELERATION,
            2,
            3,
            900_000.0,
        ),
        (
            "Slow 360 s / spread",
            -scenario::MAX_ACCELERATION,
            3,
            3,
            900_000.0,
        ),
        (
            "Speed up 360 s / reverse spread",
            scenario::MAX_ACCELERATION,
            3,
            3,
            -900_000.0,
        ),
    ] {
        let mut session = Session::blockade();
        session.dispatch(C::PlanBurn(V::new(0.0, burn))).unwrap();
        for _ in 0..start_turn {
            session.dispatch(C::NextTurn).unwrap();
        }
        session.dispatch(C::PlanBurn(V::ZERO)).unwrap();
        for _ in 0..salvos {
            for round in 0..4 {
                let target = session.aim().unwrap().target + V::new(0.0, round as f64 * spacing);
                session.dispatch(C::QueueShot(target)).unwrap();
            }
            session.dispatch(C::NextTurn).unwrap();
        }
        while !session.finished() {
            session.dispatch(C::NextTurn).unwrap();
        }
        let mission = session.mission().unwrap();
        println!(
            "{name}: outcome={:?} time={:.3}s shots={} hits={} blocked={} own_dv={:.3}km/s score={}",
            mission.outcome,
            mission.ended_seconds.unwrap(),
            mission.shots.len(),
            session.world().impacts.len(),
            session
                .world()
                .asteroid_impacts
                .iter()
                .filter(|hit| hit.projectile_id.is_some())
                .count(),
            session.world().ships[0].spent_dv / 1000.0,
            phase_vector::evaluation::evaluate(&session.snapshot())
                .unwrap()
                .total
        );
    }
}
