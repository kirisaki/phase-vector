use phase_vector::{
    physics::Vector2 as V,
    scenario::{CONTACT_SPEED, FRIENDLY_SPEED, INITIAL_SEPARATION, MAX_ACCELERATION},
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
    s.dispatch(C::PlanBurn(V::new(MAX_ACCELERATION * 2.0, 0.0)))
        .unwrap();
    for _ in 0..100_000 {
        s.pulse();
    }
    assert_eq!(s.world(), &original);
    assert_eq!(s.snapshot().planned_burn, V::new(MAX_ACCELERATION, 0.0));
    assert!(s.dispatch(C::Resume).is_err());
    assert!(s.dispatch(C::Step).is_err());
}

#[test]
fn every_speed_stops_at_exactly_120_seconds_without_overrun() {
    for speed in [1.0, 32.0, 64.0, 128.0] {
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
        assert_eq!(s.world().ships[0].position.x, -INITIAL_SEPARATION / 2.0);
        // Decimal near-c speeds accumulate small floating-point rounding errors.
        assert!((s.world().ships[0].position.y - FRIENDLY_SPEED * 120.0).abs() < 0.001);
        assert_eq!(s.world().ships[1].position.x, INITIAL_SEPARATION / 2.0);
        assert!((s.world().ships[1].position.y + CONTACT_SPEED * 120.0).abs() < 0.001);
    }
}

#[test]
fn pausing_does_not_open_a_command_opportunity() {
    let mut s = Session::default();
    s.dispatch(C::PlanBurn(V::new(50.0, 0.0))).unwrap();
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
    assert_eq!(s.snapshot().active_burn, V::new(50.0, 0.0));
}

#[test]
fn manual_steps_speed_changes_and_pauses_reproduce_the_same_world() {
    let mut normal = Session::default();
    let mut varied = normal.clone();
    for burn in [V::new(75.0, -50.0), V::new(-100.0, 0.0), V::ZERO] {
        for s in [&mut normal, &mut varied] {
            s.dispatch(C::PlanBurn(burn)).unwrap();
            s.dispatch(C::Commit).unwrap();
        }
        finish(&mut normal);
        varied.dispatch(C::Pause).unwrap();
        for _ in 0..73 {
            varied.dispatch(C::Step).unwrap();
        }
        varied.dispatch(C::SetSpeed(32.0)).unwrap();
        varied.dispatch(C::Resume).unwrap();
        for _ in 0..15 {
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
    for burn in [V::new(MAX_ACCELERATION, 0.0), V::new(0.0, -50.0)] {
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
    s.dispatch(C::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
        .unwrap();
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
fn ten_second_step_matches_individual_ticks_and_clips_to_the_boundary() {
    let mut base = Session::default();
    assert!(base.dispatch(C::StepForward).is_err());
    base.dispatch(C::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
        .unwrap();
    base.dispatch(C::Commit).unwrap();
    let before = base.world().clone();
    assert!(base.dispatch(C::StepForward).is_err());
    assert_eq!(base.world(), &before);
    base.dispatch(C::Pause).unwrap();
    for elapsed in [0, 1, 479] {
        let mut stepped = base.clone();
        for _ in 0..elapsed {
            stepped.dispatch(C::Step).unwrap();
        }
        let mut reference = stepped.clone();
        for _ in 0..40.min(INTERVAL_TICKS - elapsed) {
            reference.dispatch(C::Step).unwrap();
        }
        stepped.dispatch(C::StepForward).unwrap();
        assert_eq!(stepped.world(), reference.world());
        assert_eq!(stepped.snapshot().phase, reference.snapshot().phase);
        assert!(stepped.snapshot().paused);
        assert_eq!(stepped.snapshot().speed, 32);
        if elapsed == 479 {
            assert_eq!(stepped.world().tick, INTERVAL_TICKS);
            assert!(stepped.dispatch(C::StepForward).is_err());
        } else {
            assert_eq!(
                stepped.snapshot().time_seconds,
                elapsed as f64 * 0.25 + 10.0
            );
        }
    }
}

#[test]
fn next_turn_preserves_physics_and_stops_at_the_next_planning_boundary() {
    let mut world = Session::default().world().clone();
    let target = &world.ships[1];
    world
        .launch(
            1,
            target.position + V::new(-10_000.0, 0.0),
            target.velocity + V::new(1_000.0, 0.0),
            1.0,
        )
        .unwrap();
    world.launch(1, V::ZERO, V::new(3.0, 0.0), 1.0).unwrap();
    let initial = Session::new(world, 1).unwrap();
    let mut reference = initial.clone();
    reference
        .dispatch(C::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
        .unwrap();
    reference.dispatch(C::Commit).unwrap();
    reference.dispatch(C::Pause).unwrap();
    for _ in 0..INTERVAL_TICKS {
        reference.dispatch(C::Step).unwrap();
    }
    assert_eq!(reference.world().impacts.len(), 1);
    assert_eq!(reference.world().projectiles.len(), 1);
    for paused in [false, true] {
        let mut skipped = initial.clone();
        assert!(skipped.dispatch(C::NextTurn).is_err());
        skipped
            .dispatch(C::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
            .unwrap();
        skipped.dispatch(C::Commit).unwrap();
        for _ in 0..10 {
            skipped.pulse();
        }
        if paused {
            skipped.dispatch(C::Pause).unwrap();
        }
        skipped.dispatch(C::NextTurn).unwrap();
        assert_eq!(skipped.world(), reference.world());
        assert_eq!(
            skipped.snapshot().command_log,
            reference.snapshot().command_log
        );
        assert_eq!(skipped.snapshot().phase, Phase::Planning);
        assert!(skipped.snapshot().paused);
        assert_eq!(skipped.snapshot().speed, 32);
        assert_eq!(skipped.snapshot().planned_burn, V::ZERO);
        assert_eq!(skipped.snapshot().command_number, 2);
        assert!(skipped.dispatch(C::NextTurn).is_err());
        for _ in 0..100 {
            skipped.pulse();
        }
        assert_eq!(skipped.world(), reference.world());
        skipped.dispatch(C::Commit).unwrap();
        skipped.dispatch(C::NextTurn).unwrap();
        assert_eq!(skipped.world().tick, INTERVAL_TICKS * 2);
    }
}

#[test]
fn invalid_values_do_not_change_the_plan_or_playback_speed() {
    let mut s = Session::default();
    for x in [f64::NAN, f64::INFINITY, -f64::INFINITY, 1e30] {
        assert!(s.dispatch(C::PlanBurn(V::new(x, 0.0))).is_err());
        assert_eq!(s.snapshot().planned_burn, V::ZERO);
    }
    for speed in [0.0, -1.0, 1.5, 4.0, 16.0, 1000.0, f64::NAN] {
        assert!(s.dispatch(C::SetSpeed(speed)).is_err());
        assert_eq!(s.snapshot().speed, 32);
    }
}

#[test]
fn reset_restores_initial_conditions_and_clears_orders_and_history() {
    let mut s = Session::default();
    let initial = s.world().clone();
    s.dispatch(C::PlanBurn(V::new(0.0, 100.0))).unwrap();
    s.dispatch(C::SetSpeed(64.0)).unwrap();
    s.dispatch(C::Commit).unwrap();
    for _ in 0..10 {
        s.pulse();
    }
    s.dispatch(C::Reset).unwrap();
    assert_eq!(s.world(), &initial);
    assert!(s.snapshot().command_log.is_empty());
    assert_eq!(s.snapshot().speed, 32);
    assert_eq!(s.snapshot().phase, Phase::Planning);
    assert!(s.snapshot().paused);
}
