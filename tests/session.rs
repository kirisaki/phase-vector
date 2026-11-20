use phase_vector::{
    physics::{STEP_SECONDS, Vector2 as V},
    scenario::{CONTACT_SPEED, FRIENDLY_SPEED, INITIAL_SEPARATION, MAX_ACCELERATION},
    session::{
        Command as C, DEFAULT_SPEED, INTERVAL_SECONDS, INTERVAL_TICKS, PULSES_PER_TICK, Phase,
        Session,
    },
    timing::{STEP_FORWARD_SECONDS, STEP_FORWARD_TICKS},
};

fn finish(session: &mut Session) {
    session.dispatch(C::NextTurn).unwrap();
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
    s.dispatch(C::Resume).unwrap();
    assert!(!s.snapshot().paused);
}

#[test]
fn every_speed_continues_across_four_intervals_without_losing_time() {
    let mut reference = Session::default();
    let burn = V::new(MAX_ACCELERATION, 0.0);
    reference.dispatch(C::PlanBurn(burn)).unwrap();
    for _ in 0..4 {
        reference.dispatch(C::NextTurn).unwrap();
    }
    for speed in [1, 32, 64, 128] {
        let mut s = Session::default();
        s.dispatch(C::SetSpeed(speed as f64)).unwrap();
        s.dispatch(C::PlanBurn(burn)).unwrap();
        s.dispatch(C::Resume).unwrap();
        for _ in 0..(4 * INTERVAL_TICKS * u64::from(PULSES_PER_TICK) / speed) {
            s.pulse();
        }
        assert_eq!(s.world(), reference.world());
        assert_eq!(s.snapshot().command_log, reference.snapshot().command_log);
        assert!(!s.snapshot().paused);
        assert_eq!(s.snapshot().planned_burn, burn);
        assert_eq!(s.snapshot().command_log.len(), 4);
        s.dispatch(C::Pause).unwrap();
        let frozen = s.world().clone();
        for _ in 0..100 {
            s.pulse();
        }
        assert_eq!(s.world(), &frozen);
        let own = &s.world().ships[0];
        assert!((own.position.y - FRIENDLY_SPEED * 4.0 * INTERVAL_SECONDS).abs() < 0.001);
        assert_eq!(s.world().ships[1].position.x, INITIAL_SEPARATION / 2.0);
        assert!(
            (s.world().ships[1].position.y + CONTACT_SPEED * 4.0 * INTERVAL_SECONDS).abs() < 0.001
        );
    }
}

#[test]
fn paused_edits_queue_without_changing_the_active_order() {
    let mut s = Session::default();
    s.dispatch(C::PlanBurn(V::new(20.0, 0.0))).unwrap();
    s.dispatch(C::Commit).unwrap();
    assert!(s.dispatch(C::Step).is_err());
    s.dispatch(C::Pause).unwrap();
    let frozen = s.world().clone();
    for _ in 0..1000 {
        s.pulse();
    }
    assert_eq!(s.world(), &frozen);
    s.dispatch(C::PlanBurn(V::ZERO)).unwrap();
    assert_eq!(s.snapshot().planned_burn, V::ZERO);
    s.dispatch(C::Step).unwrap();
    assert_eq!(s.world().tick, 1);
    assert_eq!(s.snapshot().time_seconds, STEP_SECONDS);
    assert_eq!(s.snapshot().active_burn, V::new(20.0, 0.0));
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
fn a_new_interval_repeats_until_explicitly_changed_to_coasting() {
    let mut s = Session::default();
    s.dispatch(C::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
        .unwrap();
    s.dispatch(C::Commit).unwrap();
    finish(&mut s);
    let end = s.world().ships[0].clone();
    assert_eq!(s.snapshot().planned_burn, V::new(MAX_ACCELERATION, 0.0));
    s.dispatch(C::PlanBurn(V::ZERO)).unwrap();
    s.dispatch(C::Commit).unwrap();
    finish(&mut s);
    assert_eq!(s.world().ships[0].velocity, end.velocity);
    assert_eq!(s.world().ships[0].remaining_dv, end.remaining_dv);
    assert_eq!(s.world().tick, 2 * INTERVAL_TICKS);
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
    base.dispatch(C::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
        .unwrap();
    base.dispatch(C::Commit).unwrap();
    let before = base.world().clone();
    assert!(base.dispatch(C::StepForward).is_err());
    assert_eq!(base.world(), &before);
    base.dispatch(C::Pause).unwrap();
    for elapsed in [0, 1, INTERVAL_TICKS - 1] {
        let mut stepped = base.clone();
        for _ in 0..elapsed {
            stepped.dispatch(C::Step).unwrap();
        }
        let mut reference = stepped.clone();
        for _ in 0..STEP_FORWARD_TICKS.min(INTERVAL_TICKS - elapsed) {
            reference.dispatch(C::Step).unwrap();
        }
        stepped.dispatch(C::StepForward).unwrap();
        assert_eq!(stepped.world(), reference.world());
        assert_eq!(stepped.snapshot().phase, reference.snapshot().phase);
        assert!(stepped.snapshot().paused);
        assert_eq!(stepped.snapshot().speed, DEFAULT_SPEED);
        if elapsed == INTERVAL_TICKS - 1 {
            assert_eq!(stepped.world().tick, INTERVAL_TICKS);
            stepped.dispatch(C::StepForward).unwrap();
            assert_eq!(stepped.world().tick, INTERVAL_TICKS + STEP_FORWARD_TICKS);
        } else {
            assert_eq!(
                stepped.snapshot().time_seconds,
                elapsed as f64 * STEP_SECONDS + STEP_FORWARD_SECONDS
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
    for (committed, paused) in [(false, false), (true, false), (true, true)] {
        let mut skipped = initial.clone();
        skipped
            .dispatch(C::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
            .unwrap();
        if committed {
            skipped.dispatch(C::Commit).unwrap();
            for _ in 0..10 {
                skipped.pulse();
            }
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
        assert_eq!(skipped.snapshot().speed, DEFAULT_SPEED);
        assert_eq!(
            skipped.snapshot().planned_burn,
            V::new(MAX_ACCELERATION, 0.0)
        );
        assert_eq!(skipped.snapshot().command_number, 2);
        for _ in 0..100 {
            skipped.pulse();
        }
        assert_eq!(skipped.world(), reference.world());
        skipped.dispatch(C::PlanBurn(V::ZERO)).unwrap();
        let previous = skipped.world().ships[0].clone();
        skipped.dispatch(C::NextTurn).unwrap();
        assert_eq!(skipped.world().tick, INTERVAL_TICKS * 2);
        assert_eq!(skipped.world().ships[0].velocity, previous.velocity);
        assert_eq!(skipped.world().ships[0].remaining_dv, previous.remaining_dv);
        assert_eq!(skipped.snapshot().command_log.len(), 2);
        assert_eq!(skipped.snapshot().command_log[1].burn.acceleration, V::ZERO);
        assert_eq!(skipped.snapshot().phase, Phase::Planning);
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
        assert_eq!(s.snapshot().speed, DEFAULT_SPEED);
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
    assert_eq!(s.snapshot().speed, DEFAULT_SPEED);
    assert_eq!(s.snapshot().phase, Phase::Planning);
    assert!(s.snapshot().paused);
}

#[test]
fn running_edits_apply_only_at_the_next_boundary_and_repeat_with_replay() {
    let mut s = Session::default();
    let mut replay = s.world().clone();
    let first = V::new(10.0, 0.0);
    let second = V::new(0.0, -20.0);
    s.dispatch(C::PlanBurn(first)).unwrap();
    s.dispatch(C::Resume).unwrap();
    for _ in 0..70 {
        s.pulse();
    }
    let frozen = s.world().clone();
    s.dispatch(C::PlanBurn(V::ZERO)).unwrap();
    s.dispatch(C::PlanBurn(second)).unwrap();
    assert_eq!(s.world(), &frozen);
    assert_eq!(s.snapshot().active_burn, first);
    assert_eq!(s.snapshot().order_start_seconds, INTERVAL_SECONDS);
    // Remaining at 32x: exactly four intervals in 900 pulses, with no pauses.
    for _ in 70..(4 * INTERVAL_TICKS * u64::from(PULSES_PER_TICK) / u64::from(DEFAULT_SPEED)) {
        s.pulse();
    }
    assert_eq!(s.world().tick, 4 * INTERVAL_TICKS);
    assert!(!s.snapshot().paused);
    for (index, record) in s.snapshot().command_log.iter().enumerate() {
        assert_eq!(record.start_tick, index as u64 * INTERVAL_TICKS);
        assert_eq!(
            record.burn.acceleration,
            if index == 0 { first } else { second }
        );
        for _ in 0..INTERVAL_TICKS {
            replay.step(&[record.burn]).unwrap();
        }
    }
    assert_eq!(s.world(), &replay);
}

#[test]
fn stepping_from_a_boundary_is_paused_and_commit_alias_does_not_restart_an_interval() {
    let mut s = Session::default();
    s.dispatch(C::StepForward).unwrap();
    assert!(s.snapshot().paused);
    assert_eq!(s.world().tick, STEP_FORWARD_TICKS);
    s.dispatch(C::Commit).unwrap();
    s.dispatch(C::Resume).unwrap();
    assert_eq!(s.snapshot().command_log.len(), 1);
    assert_eq!(
        s.snapshot().remaining_seconds,
        INTERVAL_SECONDS - STEP_FORWARD_SECONDS
    );
    assert!(!s.snapshot().paused);
}
