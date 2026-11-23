//! Equal-budget cover trials. Targets use only observed lead points.
use phase_vector::{
    ai::Policy,
    mission::Config,
    physics::Vector2 as V,
    scenario,
    session::{Command as C, Session},
};

fn main() {
    let compare = std::env::args().any(|arg| arg == "--compare");
    let variants = if compare {
        vec![
            (1, 120.0),
            (2, 120.0),
            (2, 110.0),
            (2, 105.0),
            (2, 90.0),
            (2, 60.0),
        ]
    } else {
        vec![(2, Policy::default().reaction_seconds)]
    };
    for (count, reaction) in variants {
        println!("\nAsteroids={count}, reaction horizon={reaction:.0}s");
        for (name, burn, start_turn, salvos, spacing) in [
            ("No attack", 0.0, 0, 0, 0.0),
            ("Coast / 200 km spread", 0.0, 2, 3, 200_000.0),
            ("Slow 240 s / lead", -scenario::MAX_ACCELERATION, 2, 3, 0.0),
            (
                "Slow 240 s / 200 km spread",
                -scenario::MAX_ACCELERATION,
                2,
                3,
                200_000.0,
            ),
            (
                "Slow 240 s / 600 km spread",
                -scenario::MAX_ACCELERATION,
                2,
                3,
                600_000.0,
            ),
            (
                "Slow 240 s / 300 km spread",
                -scenario::MAX_ACCELERATION,
                2,
                3,
                300_000.0,
            ),
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
            let mut world = scenario::blockade_world();
            world.asteroids.truncate(count);
            let mut session = Session::with_mission(
                world,
                1,
                Config {
                    policy: Policy {
                        reaction_seconds: reaction,
                        ..Policy::default()
                    },
                    ..Config::default()
                },
            )
            .unwrap();
            session.dispatch(C::PlanBurn(V::new(0.0, burn))).unwrap();
            for _ in 0..start_turn {
                session.dispatch(C::NextTurn).unwrap();
            }
            session.dispatch(C::PlanBurn(V::ZERO)).unwrap();
            for _ in 0..salvos {
                for round in 0..4 {
                    let target =
                        session.aim().unwrap().target + V::new(0.0, round as f64 * spacing);
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
}
