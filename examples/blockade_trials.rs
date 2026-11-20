use phase_vector::{
    physics::Vector2 as V,
    session::{Command as C, Session},
};
fn main() {
    for offset in [None, Some(0.0), Some(10_000_000.0)] {
        let mut s = Session::blockade();
        for turn in 0..10 {
            if s.finished() {
                break;
            }
            if turn < 7
                && let Some(y) = offset
            {
                let target = s.aim().unwrap().target + V::new(0.0, y);
                s.dispatch(C::QueueShot(target)).unwrap();
            }
            s.dispatch(C::NextTurn).unwrap();
        }
        let m = s.mission().unwrap();
        println!(
            "offset={offset:?} outcome={:?} time={:?} shots={} evasions={} enemy_dv={:.2}km/s goal_miss={:.0}km",
            m.outcome,
            m.ended_seconds,
            m.shots.len(),
            m.decision.evasions,
            s.world().ships[1].spent_dv / 1000.0,
            (s.world().ships[1].position - m.config.goal).length() / 1000.0
        );
    }
}
