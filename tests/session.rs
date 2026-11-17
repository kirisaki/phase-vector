use phase_vector::{
    physics::Vector2 as V,
    session::{Command as C, INTERVAL_TICKS, Phase, Session},
};

fn finish(session: &mut Session) {
    let mut pulses = 0;
    while session.snapshot().phase == Phase::Executing {
        session.pulse();
        pulses += 1;
        assert!(pulses <= 7200);
    }
}

#[test]
fn planning_is_unlimited_and_does_not_consume_time_or_delta_v() {
    let mut s = Session::default();
    let original = s.world().clone();
    s.dispatch(C::PlanBurn(V::new(1.0, 0.0))).unwrap();
    for _ in 0..100_000 {
        s.pulse();
    }
    assert_eq!(s.world(), &original);
    assert_eq!(s.snapshot().planned_burn, V::new(0.02, 0.0));
    assert!(s.dispatch(C::Resume).is_err());
    assert!(s.dispatch(C::Step).is_err());
}

#[test]
fn every_speed_stops_at_exactly_120_seconds_without_overrun() {
    for speed in [1.0, 4.0, 16.0, 64.0] {
        let mut s = Session::default();
        s.dispatch(C::SetSpeed(speed)).unwrap();
        s.dispatch(C::Commit).unwrap();
        finish(&mut s);
        assert_eq!(s.world().tick, INTERVAL_TICKS);
        assert_eq!(s.snapshot().time_seconds, 120.0);
        assert!(s.snapshot().paused);
        assert_eq!(s.snapshot().phase, Phase::Planning);
        assert_eq!(s.snapshot().remaining_seconds, 120.0);
        assert_eq!(s.snapshot().command_number, 2);
        for _ in 0..100 {
            s.pulse();
        }
        assert_eq!(s.world().tick, INTERVAL_TICKS);
        // Both actors drift through the same time interval.
        assert_eq!(s.world().ships[0].position.x, -480.0);
        assert!((s.world().ships[1].position.x - 164.0).abs() < 1e-9);
    }
}

#[test]
fn pausing_does_not_open_a_command_opportunity() {
    let mut s = Session::default();
    s.dispatch(C::PlanBurn(V::new(0.01, 0.0))).unwrap();
    s.dispatch(C::Commit).unwrap();
    assert!(s.dispatch(C::Step).is_err());
    s.dispatch(C::Pause).unwrap();
    let frozen = s.world().clone();
    for _ in 0..1000 {
        s.pulse();
    }
    assert_eq!(s.world(), &frozen);
    assert!(s.dispatch(C::PlanBurn(V::ZERO)).is_err());
    assert!(s.dispatch(C::Commit).is_err());
    s.dispatch(C::Step).unwrap();
    assert_eq!(s.world().tick, 1);
    assert_eq!(s.snapshot().time_seconds, 0.25);
    assert_eq!(s.snapshot().active_burn, V::new(0.01, 0.0));
}

#[test]
fn manual_steps_speed_changes_and_pauses_reproduce_the_same_world() {
    let mut normal = Session::default();
    let mut varied = normal.clone();
    for burn in [V::new(0.015, -0.01), V::new(-0.02, 0.0), V::ZERO] {
        for s in [&mut normal, &mut varied] {
            s.dispatch(C::PlanBurn(burn)).unwrap();
            s.dispatch(C::Commit).unwrap();
        }
        finish(&mut normal);
        varied.dispatch(C::Pause).unwrap();
        for _ in 0..73 {
            varied.dispatch(C::Step).unwrap();
        }
        varied.dispatch(C::SetSpeed(4.0)).unwrap();
        varied.dispatch(C::Resume).unwrap();
        for _ in 0..120 {
            varied.pulse();
        }
        varied.dispatch(C::Pause).unwrap();
        for _ in 0..100 {
            varied.pulse();
        }
        varied.dispatch(C::SetSpeed(64.0)).unwrap();
        varied.dispatch(C::Resume).unwrap();
        finish(&mut varied);
        assert_eq!(normal.world(), varied.world());
        assert_eq!(normal.snapshot().command_log, varied.snapshot().command_log);
    }
}

#[test]
fn history_replays_physics_without_the_presentation_clock() {
    let mut s = Session::default();
    let mut replay = s.world().clone();
    for burn in [V::new(0.02, 0.0), V::new(0.0, -0.01)] {
        s.dispatch(C::PlanBurn(burn)).unwrap();
        s.dispatch(C::Commit).unwrap();
        finish(&mut s);
    }
    for record in s.snapshot().command_log {
        assert_eq!(record.start_tick, replay.tick);
        for _ in 0..INTERVAL_TICKS {
            replay.step(&[record.burn]).unwrap();
        }
    }
    assert_eq!(&replay, s.world());
}

#[test]
fn a_new_interval_defaults_to_coasting_and_preserves_world_state() {
    let mut s = Session::default();
    s.dispatch(C::PlanBurn(V::new(0.02, 0.0))).unwrap();
    s.dispatch(C::Commit).unwrap();
    finish(&mut s);
    let end = s.world().ships[0].clone();
    assert_eq!(s.snapshot().planned_burn, V::ZERO);
    s.dispatch(C::Commit).unwrap();
    finish(&mut s);
    assert_eq!(s.world().ships[0].velocity, end.velocity);
    assert_eq!(s.world().ships[0].remaining_dv, end.remaining_dv);
    assert_eq!(s.world().tick, 960);
}

#[test]
fn projectiles_and_contact_history_survive_command_boundaries() {
    let mut world = Session::default().world().clone();
    world
        .launch(1, V::new(0.0, -400.0), V::new(3.0, 0.0), 1.0)
        .unwrap();
    let mut s = Session::new(world, 1).unwrap();
    for _ in 0..2 {
        s.dispatch(C::Commit).unwrap();
        finish(&mut s);
    }
    assert_eq!(s.world().projectiles.len(), 1);
    assert_eq!(s.world().projectiles[0].position, V::new(720.0, -400.0));
}

#[test]
fn invalid_values_do_not_change_the_plan_or_playback_speed() {
    let mut s = Session::default();
    for x in [f64::NAN, f64::INFINITY, -f64::INFINITY, 1e30] {
        assert!(s.dispatch(C::PlanBurn(V::new(x, 0.0))).is_err());
        assert_eq!(s.snapshot().planned_burn, V::ZERO);
    }
    for speed in [0.0, -1.0, 1.5, 1000.0, f64::NAN] {
        assert!(s.dispatch(C::SetSpeed(speed)).is_err());
        assert_eq!(s.snapshot().speed, 1);
    }
}

#[test]
fn reset_restores_initial_conditions_and_clears_orders_and_history() {
    let mut s = Session::default();
    let initial = s.world().clone();
    s.dispatch(C::PlanBurn(V::new(0.0, 0.02))).unwrap();
    s.dispatch(C::SetSpeed(64.0)).unwrap();
    s.dispatch(C::Commit).unwrap();
    for _ in 0..10 {
        s.pulse();
    }
    s.dispatch(C::Reset).unwrap();
    assert_eq!(s.world(), &initial);
    assert!(s.snapshot().command_log.is_empty());
    assert_eq!(s.snapshot().speed, 1);
    assert_eq!(s.snapshot().phase, Phase::Planning);
    assert!(s.snapshot().paused);
}
