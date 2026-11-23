//! Small deterministic placement sample, not an estimated player win rate.
use phase_vector::{
    ai::Policy,
    mission::{Config, Outcome},
    physics::Vector2 as V,
    scenario,
    session::{Command as C, Session},
};
fn main() {
    for reaction in [90.0, 60.0, 110.0] {
        let mut wins = 0;
        for spacing in [
            100.0, 200.0, 400.0, 500.0, 700.0, 800.0, 1000.0, 1100.0, 1200.0, 1500.0, -300.0,
            -600.0, -900.0,
        ] {
            for offset in [-600.0, 0.0, 600.0] {
                let mut s = Session::with_mission(
                    scenario::blockade_world(),
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
                s.dispatch(C::PlanBurn(V::new(0.0, -scenario::MAX_ACCELERATION)))
                    .unwrap();
                for _ in 0..2 {
                    s.dispatch(C::NextTurn).unwrap();
                }
                s.dispatch(C::PlanBurn(V::ZERO)).unwrap();
                for _ in 0..3 {
                    for i in 0..4 {
                        let target = s.aim().unwrap().target
                            + V::new(0.0, (offset + i as f64 * spacing) * 1000.0);
                        s.dispatch(C::QueueShot(target)).unwrap();
                    }
                    s.dispatch(C::NextTurn).unwrap();
                }
                while !s.finished() {
                    s.dispatch(C::NextTurn).unwrap();
                }
                if matches!(
                    s.mission().unwrap().outcome,
                    Outcome::Disabled | Outcome::Timeout
                ) {
                    wins += 1;
                    println!(
                        "reaction={reaction} spacing={spacing} offset={offset} outcome={:?} shots={} hits={} blocks={}",
                        s.mission().unwrap().outcome,
                        s.mission().unwrap().shots.len(),
                        s.world().impacts.len(),
                        s.world().asteroid_impacts.len()
                    );
                }
            }
        }
        println!("reaction={reaction} winning sampled placements={wins}/39");
    }
}
