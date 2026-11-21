use phase_vector::{
    ai::{self, Decision, Policy},
    mission::{Config, MAX_QUEUED_SHOTS, Mission, Outcome, solve_shot},
    observation::Track,
    physics::{STEP_SECONDS, Ship, Vector2 as V, World},
    scenario,
    session::{Command as C, Session},
};

fn run_to_end(s: &mut Session) {
    for _ in 0..20 {
        if s.finished() {
            return;
        }
        s.dispatch(C::NextTurn).unwrap();
    }
    panic!("mission did not terminate");
}

#[test]
fn no_attack_breaks_through_and_zero_fuel_does_not_award_victory() {
    for fuel in [0.0, scenario::INITIAL_DELTA_V] {
        let mut world = scenario::initial_world();
        world.ships[1].remaining_dv = fuel;
        let mut s = Session::with_mission(world, 1, Config::default()).unwrap();
        s.dispatch(C::NextTurn).unwrap();
        assert_eq!(s.mission().unwrap().outcome, Outcome::Running);
        run_to_end(&mut s);
        let m = s.mission().unwrap();
        assert_eq!(m.outcome, Outcome::Breakthrough);
        assert!((m.ended_seconds.unwrap() - 1173.3333333333333).abs() < 1e-6);
        assert!(m.shots.is_empty());
    }
}

#[test]
fn off_route_is_not_a_win_before_the_deadline() {
    let mut world = scenario::initial_world();
    world.ships[1].position.x += 10_000_000.0;
    world.ships[1].remaining_dv = 0.0;
    let mut s = Session::with_mission(world, 1, Config::default()).unwrap();
    for _ in 0..9 {
        s.dispatch(C::NextTurn).unwrap();
        assert!(!s.finished());
    }
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.mission().unwrap().outcome, Outcome::Timeout);
    assert_eq!(
        s.mission().unwrap().ended_seconds,
        Some(Config::default().deadline_tick as f64 * STEP_SECONDS)
    );
}

fn event_fixture(goal: V, radius: f64, projectile_x: Option<f64>) -> Mission {
    let mut world = World::new(vec![
        Ship::new(1, V::new(-100.0, 100.0), V::ZERO, 1.0, 49.0, 1000.0).unwrap(),
        Ship::new(2, V::ZERO, V::new(10.0, 0.0), 1.0, 49.0, 1000.0).unwrap(),
    ])
    .unwrap();
    if let Some(x) = projectile_x {
        world.launch(1, V::new(x, 0.0), V::ZERO, 0.0).unwrap();
    }
    let config = Config {
        goal,
        goal_radius: radius,
        deadline_tick: 1,
        cruise_speed: 10.0,
        ..Config::default()
    };
    let mut mission = Mission::new(&world, 1, config).unwrap();
    mission.step(&mut world, 1, V::ZERO);
    mission
}

#[test]
fn swept_goal_entry_and_impact_ordering_include_exact_deadline() {
    let early_hit = event_fixture(V::new(2.0, 0.0), 1.0, Some(1.0));
    assert_eq!(early_hit.outcome, Outcome::Disabled);
    let late_hit = event_fixture(V::new(2.0, 0.0), 1.0, Some(3.0));
    assert_eq!(late_hit.outcome, Outcome::Breakthrough);
    let tie = event_fixture(V::new(2.0, 0.0), 1.0, Some(2.0));
    assert_eq!(tie.outcome, Outcome::Disabled);
    assert_eq!(
        event_fixture(V::new(1.5, 0.0), 0.1, None).outcome,
        Outcome::Breakthrough
    );
    let exact = event_fixture(V::new(3.5, 0.0), 1.0, None);
    assert_eq!(exact.outcome, Outcome::Breakthrough);
    assert_eq!(exact.ended_seconds, Some(STEP_SECONDS));
    let exact_hit = event_fixture(V::new(3.6, 0.0), 1.0, Some(3.5));
    assert_eq!(exact_hit.outcome, Outcome::Disabled);
    assert_eq!(exact_hit.ended_seconds, Some(STEP_SECONDS));
    assert_eq!(
        event_fixture(V::new(3.6, 0.0), 1.0, None).outcome,
        Outcome::Timeout
    );
    assert_eq!(
        event_fixture(V::new(3.6, 0.0), 1.0, Some(3.6)).outcome,
        Outcome::Timeout
    );
}

#[test]
fn queued_gun_uses_inherited_velocity_reload_and_persists_across_turns() {
    let mut s = Session::blockade();
    let aim = s.aim().unwrap();
    assert!((240.0..480.0).contains(&aim.flight_seconds));
    s.dispatch(C::QueueShot(aim.target)).unwrap();
    assert!(s.world().projectiles.is_empty());
    let ship = s.world().ships[0].clone();
    s.dispatch(C::Resume).unwrap();
    let projectile = &s.world().projectiles[0];
    assert!(
        ((projectile.velocity - ship.velocity).length() - scenario::RAILGUN_SPEED).abs() < 1e-6
    );
    assert!(
        (projectile.position + projectile.velocity * aim.flight_seconds - aim.target).length()
            < 0.01
    );
    assert_eq!(
        s.mission().unwrap().ready_tick,
        s.mission().unwrap().config.reload_ticks
    );
    let frozen = s.world().clone();
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    assert_eq!(s.world(), &frozen);
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.world().projectiles.len(), 1);
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.mission().unwrap().shots.len(), 2);
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(
        s.mission().unwrap().shots.len(),
        2,
        "shots must not repeat automatically"
    );
}

#[test]
fn invalid_shots_and_cooldown_do_not_replace_an_existing_reservation() {
    let mut world = scenario::initial_world();
    let mut m = Mission::new(&world, 1, Config::default()).unwrap();
    let ship = world.ships[0].clone();
    let aim = m.aim(&ship, 0).unwrap();
    m.queue(&ship, aim.target, 0).unwrap();
    let before = m.clone();
    for target in [V::new(f64::NAN, 0.0), V::new(1e30, 0.0), ship.position] {
        assert!(m.queue(&ship, target, 0).is_err());
        assert_eq!(m, before);
    }
    m.launch_queued(&mut world, 1).unwrap();
    assert!(m.queue(&ship, aim.target, 0).is_err());
    assert!(m.queue(&ship, aim.target, m.config.deadline_tick).is_err());
    assert!(solve_shot(&ship, ship.position, 0).is_err());
}

#[test]
fn hidden_projectiles_cannot_affect_ai_before_sensor_delivery() {
    let mut world = scenario::initial_world();
    let enemy = world.ships[1].clone();
    world
        .launch(
            1,
            enemy.position + V::new(-scenario::RAILGUN_SPEED * 30.0, 0.0),
            enemy.velocity + V::new(scenario::RAILGUN_SPEED, 0.0),
            1.0,
        )
        .unwrap();
    let mut m = Mission::new(&world, 1, Config::default()).unwrap();
    for _ in 0..5 {
        m.step(&mut world, 1, V::ZERO);
        assert!(m.sensors.threats.is_empty());
        assert_eq!(m.decision.threat_id, None);
    }
    for _ in 0..5 {
        m.step(&mut world, 1, V::ZERO);
    }
    assert_eq!(m.decision.threat_id, Some(0));
    assert!(m.decision.reason.contains("observed"));
    assert!(m.sensors.threats[0].received_tick > m.sensors.threats[0].sampled_tick);
}

#[test]
fn wider_warning_corridor_reacts_to_near_misses_without_inflating_hit_radii() {
    for (offset_km, reacts) in [(100.0, true), (2000.0, false)] {
        let mut session = Session::blockade();
        let target = session.aim().unwrap().target + V::new(0.0, offset_km * 1000.0);
        session.dispatch(C::QueueShot(target)).unwrap();
        run_to_end(&mut session);
        assert_eq!(session.mission().unwrap().decision.evasions > 0, reacts);
        assert!(session.world().impacts.is_empty());
        assert_eq!(session.world().ships[1].radius, scenario::SHIP_RADIUS);
    }
}

#[test]
fn unlaunched_reservations_do_not_change_enemy_observations_or_actions() {
    let mut baseline = Session::blockade();
    let mut reserved = Session::blockade();
    for session in [&mut baseline, &mut reserved] {
        session.dispatch(C::Step).unwrap();
    }
    reserved
        .dispatch(C::QueueShot(reserved.aim().unwrap().target))
        .unwrap();
    let launch_tick = reserved.mission().unwrap().queued[0].launch_tick;
    while reserved.world().tick < launch_tick {
        baseline.dispatch(C::Step).unwrap();
        reserved.dispatch(C::Step).unwrap();
        assert_eq!(reserved.world(), baseline.world());
        assert_eq!(
            reserved.mission().unwrap().sensors,
            baseline.mission().unwrap().sensors
        );
        assert_eq!(
            reserved.mission().unwrap().decision,
            baseline.mission().unwrap().decision
        );
    }
    assert!(reserved.mission().unwrap().shots.is_empty());
    reserved.dispatch(C::Step).unwrap();
    assert_eq!(reserved.mission().unwrap().shots.len(), 1);
    assert!(reserved.mission().unwrap().sensors.threats.is_empty());
}

#[test]
fn recovery_reduces_actual_cross_track_displacement_after_an_evasion() {
    let mut world = scenario::initial_world();
    // An oblique attack forces lateral evasion, not just braking along the route.
    world.ships[0].position.y += scenario::INITIAL_SEPARATION / 2.0;
    let mut session = Session::with_mission(world, 1, Config::default()).unwrap();
    session
        .dispatch(C::QueueShot(session.aim().unwrap().target))
        .unwrap();
    let goal_x = session.mission().unwrap().config.goal.x;
    let mut peak_error = 0.0_f64;
    let mut returned = false;
    while !session.finished() {
        session.dispatch(C::StepForward).unwrap();
        let mission = session.mission().unwrap();
        let enemy = &session.world().ships[1];
        peak_error = peak_error.max((enemy.position.x - goal_x).abs());
        returned |= mission.decision.reason.contains("Returning");
        assert!(session.world().projectiles.iter().all(|p| p.owner == 1));
        assert!(enemy.remaining_dv >= 0.0);
        assert!(enemy.spent_dv <= scenario::INITIAL_DELTA_V);
    }
    assert!(returned);
    assert!(peak_error > scenario::SHIP_RADIUS);
    assert!((session.world().ships[1].position.x - goal_x).abs() < peak_error / 2.0);
}

#[test]
fn a_queued_hit_disables_the_enemy_and_stops_pending_launches_until_retry() {
    let mut world = scenario::initial_world();
    // Fuel exhaustion alone is not victory; a real queued projectile must hit.
    world.ships[1].remaining_dv = 0.0;
    let mut session = Session::with_mission(world.clone(), 1, Config::default()).unwrap();
    let initial = session.mission().unwrap().clone();
    let aim = session.aim().unwrap();
    while !session.finished() {
        while session.next_shot_tick().is_ok() {
            session.dispatch(C::QueueShot(aim.target)).unwrap();
        }
        session.dispatch(C::NextTurn).unwrap();
    }
    let mission = session.mission().unwrap();
    assert_eq!(mission.outcome, Outcome::Disabled);
    assert!((mission.ended_seconds.unwrap() - aim.arrival_seconds).abs() < STEP_SECONDS);
    assert_eq!(session.world().impacts.len(), 1);
    assert_eq!(session.world().impacts[0].ship_id, initial.config.enemy_id);
    assert!(!mission.queued.is_empty());
    assert!(
        mission
            .queued
            .iter()
            .all(|q| q.launch_tick > session.world().tick)
    );
    let ended = session.world().clone();
    let mission = mission.clone();
    for _ in 0..100 {
        session.pulse();
    }
    for command in [
        C::Resume,
        C::Commit,
        C::Step,
        C::NextTurn,
        C::QueueShot(aim.target),
    ] {
        assert!(session.dispatch(command).is_err());
    }
    assert!(session.snapshot().paused);
    assert_eq!(session.world(), &ended);
    assert_eq!(session.mission().unwrap(), &mission);
    session.dispatch(C::Reset).unwrap();
    assert_eq!(session.world(), &world);
    assert_eq!(session.mission().unwrap(), &initial);
    assert!(session.snapshot().command_log.is_empty());
}

#[test]
fn delivered_observation_and_presentation_do_not_read_hidden_enemy_changes() {
    let world = scenario::initial_world();
    let m = Mission::new(&world, 1, Config::default()).unwrap();
    let mut changed = world.clone();
    changed.tick = 4;
    changed.ships[1].position = V::new(123.0, 456.0);
    changed.ships[1].velocity = V::new(999.0, 888.0);
    let visible = m.presentation_world(&changed);
    assert_eq!(
        visible.ships[1].position,
        world.ships[1].position + world.ships[1].velocity
    );
    assert_eq!(visible.ships[1].velocity, world.ships[1].velocity);
    assert_eq!(visible.ships[0], changed.ships[0]);
}

#[test]
fn ai_returns_to_goal_after_evasion_and_ignores_out_of_window_tracks() {
    let own = scenario::initial_world().ships[1].clone();
    let goal = Config::default().goal;
    let track = Track {
        id: 3,
        sampled_tick: 0,
        received_tick: 0,
        position: own.position + V::new(-scenario::RAILGUN_SPEED * 90.0, 0.0),
        velocity: own.velocity + V::new(scenario::RAILGUN_SPEED, 0.0),
        radius: 1.0,
    };
    let policy = Policy::default();
    let d = ai::decide(
        &own,
        goal,
        scenario::CONTACT_SPEED,
        std::slice::from_ref(&track),
        0,
        policy,
        &Decision::default(),
    );
    assert_eq!(d.threat_id, Some(3));
    let mut displaced = own.clone();
    displaced.position.x += 500_000.0;
    displaced.velocity.x = 1000.0;
    let recovered = ai::decide(
        &displaced,
        goal,
        scenario::CONTACT_SPEED,
        &[],
        d.hold_until,
        policy,
        &d,
    );
    assert!(recovered.acceleration.x < 0.0);
    assert!(recovered.reason.contains("Returning"));
    let short = Policy {
        reaction_seconds: 10.0,
        ..policy
    };
    assert_eq!(
        ai::decide(
            &own,
            goal,
            scenario::CONTACT_SPEED,
            &[track],
            0,
            short,
            &Decision::default()
        )
        .threat_id,
        None
    );
}

#[test]
fn results_freeze_playback_and_retry_restores_the_entire_mission() {
    let mut s = Session::blockade();
    let initial = s.world().clone();
    run_to_end(&mut s);
    let ended = s.world().clone();
    for _ in 0..100 {
        s.pulse();
    }
    assert_eq!(s.world(), &ended);
    assert!(s.snapshot().paused);
    for command in [
        C::Resume,
        C::Step,
        C::NextTurn,
        C::QueueShot(V::ZERO),
        C::PlanBurn(V::ZERO),
    ] {
        assert!(s.dispatch(command).is_err());
        assert_eq!(s.world(), &ended);
    }
    s.dispatch(C::Reset).unwrap();
    assert_eq!(s.world(), &initial);
    assert_eq!(s.mission().unwrap().outcome, Outcome::Running);
    assert!(s.mission().unwrap().shots.is_empty());
    assert_eq!(s.mission().unwrap().decision, Decision::default());
    assert!(s.snapshot().command_log.is_empty());
}

#[test]
fn recorded_commands_replay_shots_observations_ai_and_outcome() {
    let mut s = Session::blockade();
    for i in 0..10 {
        if s.finished() {
            break;
        }
        if i < 7 {
            for _ in 0..4 {
                s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
            }
            if i == 0 {
                let id = s.mission().unwrap().queued[1].id;
                s.dispatch(C::CancelQueuedShot(id)).unwrap();
            }
        }
        s.dispatch(C::NextTurn).unwrap();
    }
    let mut world = scenario::initial_world();
    let mut replay = Mission::new(&world, 1, Config::default()).unwrap();
    for record in s.snapshot().command_log {
        assert_eq!(world.tick, record.start_tick);
        for order in &record.shots {
            replay
                .queue(&world.ships[0], order.target, order.launch_tick)
                .unwrap();
            // Restore recorded identifiers, including gaps left by cancelled orders.
            replay
                .queued
                .iter_mut()
                .find(|queued| queued.launch_tick == order.launch_tick)
                .unwrap()
                .id = order.id;
        }
        let end = record.start_tick + phase_vector::timing::INTERVAL_TICKS;
        while world.tick < end && replay.running() {
            replay.launch_queued(&mut world, 1).unwrap();
            replay.step(&mut world, 1, record.burn.acceleration);
        }
    }
    assert_eq!(s.world(), &world);
    let actual = s.mission().unwrap();
    assert_eq!(actual.shots, replay.shots);
    assert_eq!(actual.sensors, replay.sensors);
    assert_eq!(actual.decision, replay.decision);
    assert_eq!(actual.outcome, replay.outcome);
    assert_eq!(actual.ended_seconds, replay.ended_seconds);
    assert!(s.mission().unwrap().ended_seconds.unwrap() <= s.world().tick as f64 * STEP_SECONDS);
}

#[test]
fn equal_shot_counts_have_different_outcomes_and_blockade_need_not_hit() {
    let mut outcomes = Vec::new();
    for offset in [0.0, 10_000_000.0] {
        let mut s = Session::blockade();
        for turn in 0..10 {
            if s.finished() {
                break;
            }
            if turn < 7 {
                let target = s.aim().unwrap().target + V::new(0.0, offset);
                s.dispatch(C::QueueShot(target)).unwrap();
            }
            s.dispatch(C::NextTurn).unwrap();
        }
        let m = s.mission().unwrap();
        assert_eq!(m.shots.len(), 7);
        assert!(s.world().impacts.is_empty());
        assert!(s.world().ships[1].remaining_dv > 0.0);
        outcomes.push(m.outcome);
    }
    assert_eq!(outcomes, [Outcome::Timeout, Outcome::Breakthrough]);
}

#[test]
fn lua_and_ui_share_shot_validation_and_mission_restart() {
    let mut ui = phase_vector::repl::Host::with_session(Session::blockade());
    let mut lua = phase_vector::repl::Host::default();
    lua.command("mission").unwrap();
    let target = ui.state.borrow().aim().unwrap().target;
    ui.state
        .borrow_mut()
        .dispatch(C::QueueShot(target))
        .unwrap();
    lua.evaluate("game.fire(game.aim())").unwrap();
    ui.command("next_turn").unwrap();
    lua.evaluate("game.command('next_turn')").unwrap();
    assert_eq!(ui.state.borrow().world(), lua.state.borrow().world());
    assert_eq!(ui.state.borrow().mission(), lua.state.borrow().mission());
    assert!(lua.evaluate("game.fire(0/0, 0)").is_err());
    lua.evaluate("game.command('retry')").unwrap();
    assert_eq!(lua.evaluate("game.state().mission.shots").unwrap(), "0");
    assert_eq!(
        lua.evaluate("game.state().mission.outcome").unwrap(),
        "running"
    );
}

#[test]
fn mission_playback_speeds_and_steps_agree_through_terminal_events() {
    let mut reference = Session::blockade();
    let target = reference.aim().unwrap().target;
    for _ in 0..4 {
        reference.dispatch(C::QueueShot(target)).unwrap();
    }
    while !reference.finished() {
        reference.dispatch(C::NextTurn).unwrap();
    }
    for speed in [1.0, 32.0, 64.0, 128.0] {
        let mut candidate = Session::blockade();
        for _ in 0..4 {
            candidate.dispatch(C::QueueShot(target)).unwrap();
        }
        candidate.dispatch(C::SetSpeed(speed)).unwrap();
        candidate.dispatch(C::Resume).unwrap();
        for _ in 0..100_000 {
            if candidate.finished() {
                break;
            }
            candidate.pulse();
        }
        assert!(candidate.finished());
        assert_eq!(candidate.world(), reference.world());
        assert_eq!(candidate.mission(), reference.mission());
        assert_eq!(
            candidate.snapshot().command_log,
            reference.snapshot().command_log
        );
    }
}

#[test]
fn lua_exposes_detached_delivered_observations_and_ai_explanations() {
    let mut host = phase_vector::repl::Host::with_session(Session::blockade());
    host.evaluate("game.fire(game.aim()); game.command('step_forward')")
        .unwrap();
    assert_eq!(
        host.evaluate("return #game.state().mission.threats, game.state().mission.contact.vy")
            .unwrap(),
        "1\t-37500"
    );
    assert_eq!(host.evaluate("local m = game.state().mission; return m.threats[1].sampled_tick < m.threats[1].received_tick, m.contact.sampled_tick < m.contact.received_tick, m.ai_reason ~= nil").unwrap(), "true\ttrue\ttrue");
    host.evaluate("game.state().mission.threats[1].x = 0")
        .unwrap();
    assert_ne!(
        host.evaluate("game.state().mission.threats[1].x").unwrap(),
        "0"
    );
}

#[test]
fn multiple_reservations_fill_gaps_without_moving_other_shots() {
    let reload = Config::default().reload_ticks;
    let mut s = Session::blockade();
    for _ in 0..4 {
        s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    }
    let orders = s.mission().unwrap().queued.clone();
    assert_eq!(
        orders.iter().map(|q| q.launch_tick).collect::<Vec<_>>(),
        vec![0, reload, 2 * reload, 3 * reload]
    );
    s.dispatch(C::CancelQueuedShot(orders[1].id)).unwrap();
    assert_eq!(s.mission().unwrap().queued[1], orders[2]);
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    let queue = &s.mission().unwrap().queued;
    assert_eq!(queue[1].launch_tick, reload);
    assert!(queue[1].id > orders[3].id);
    let before = s.mission().unwrap().clone();
    assert!(s.dispatch(C::CancelQueuedShot(orders[1].id)).is_err());
    assert!(s.dispatch(C::QueueShot(V::new(f64::NAN, 0.0))).is_err());
    assert_eq!(s.mission().unwrap(), &before);
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.mission().unwrap().shots.len(), 4);
    assert!(s.mission().unwrap().queued.is_empty());
    assert_eq!(
        s.mission()
            .unwrap()
            .shots
            .iter()
            .map(|shot| shot.order_id)
            .collect::<Vec<_>>(),
        before
            .queued
            .iter()
            .map(|order| order.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        s.mission()
            .unwrap()
            .shots
            .iter()
            .map(|q| q.launch_tick)
            .collect::<Vec<_>>(),
        vec![0, reload, 2 * reload, 3 * reload]
    );
}

#[test]
fn queue_respects_long_reload_and_deadline_and_reset_clears_all() {
    use phase_vector::timing::INTERVAL_TICKS;
    let world = scenario::initial_world();
    let mut m = Mission::new(
        &world,
        1,
        Config {
            reload_ticks: INTERVAL_TICKS + 1,
            deadline_tick: 3 * (INTERVAL_TICKS + 1),
            ..Config::default()
        },
    )
    .unwrap();
    for slot in 0..3 {
        let tick = m.next_launch_tick(0).unwrap();
        assert_eq!(tick, slot * m.config.reload_ticks);
        m.queue(&world.ships[0], V::ZERO, tick).unwrap();
    }
    let before = m.clone();
    assert!(m.next_launch_tick(0).is_err());
    assert!(m.queue(&world.ships[0], V::ZERO, INTERVAL_TICKS).is_err());
    assert_eq!(m, before);
    m.cancel(m.queued[2].id, 0).unwrap();
    assert_eq!(m.next_launch_tick(0).unwrap(), 2 * m.config.reload_ticks);
    let mut s = Session::blockade();
    while let Ok(aim) = s.aim() {
        s.dispatch(C::QueueShot(aim.target)).unwrap();
    }
    assert_eq!(s.mission().unwrap().queued.len(), MAX_QUEUED_SHOTS);
    let before = s.mission().unwrap().clone();
    assert!(s.dispatch(C::QueueShot(V::ZERO)).is_err());
    assert_eq!(s.mission().unwrap(), &before);
    s.dispatch(C::Reset).unwrap();
    assert!(s.mission().unwrap().queued.is_empty());
    assert_eq!(s.next_shot_tick().unwrap(), 0);
}

#[test]
fn future_launch_preview_tracks_maneuvers_and_matches_actual_launch() {
    use phase_vector::timing::INTERVAL_TICKS;
    let mut s = Session::with_mission(
        scenario::initial_world(),
        1,
        Config {
            reload_ticks: INTERVAL_TICKS,
            ..Config::default()
        },
    )
    .unwrap();
    for _ in 0..3 {
        s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    }
    let order = s.mission().unwrap().queued[2].clone();
    s.dispatch(C::StepForward).unwrap();
    let active = s.snapshot().active_burn;
    let coast = s.snapshot().planning_ship_at(order.launch_tick);
    s.dispatch(C::PlanBurn(V::new(scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    assert_eq!(s.snapshot().active_burn, active);
    let predicted = s.snapshot().planning_ship_at(order.launch_tick);
    assert_ne!(predicted.position, coast.position);
    assert_eq!(s.mission().unwrap().queued[1], order);
    let solution = solve_shot(&predicted, order.target, order.launch_tick).unwrap();
    s.dispatch(C::NextTurn).unwrap();
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.world().tick, 2 * INTERVAL_TICKS);
    assert_eq!(s.world().ships[0], predicted);
    s.dispatch(C::Resume).unwrap();
    let shot = &s.mission().unwrap().shots[2];
    assert_eq!(shot.origin, solution.origin);
    assert_eq!(
        shot.velocity,
        predicted.velocity + solution.direction * scenario::RAILGUN_SPEED
    );
    s.dispatch(C::CancelShot).unwrap();
    assert_eq!(s.mission().unwrap().shots.len(), 3);
}

#[test]
fn lua_multiple_queue_and_individual_cancel_match_the_command_api() {
    let mut lua = phase_vector::repl::Host::with_session(Session::blockade());
    let mut ui = Session::blockade();
    for _ in 0..3 {
        ui.dispatch(C::QueueShot(ui.aim().unwrap().target)).unwrap();
        lua.evaluate("game.fire(game.aim())").unwrap();
    }
    assert_eq!(lua.evaluate("#game.state().mission.queue").unwrap(), "3");
    let id = ui.mission().unwrap().queued[1].id;
    ui.dispatch(C::CancelQueuedShot(id)).unwrap();
    lua.evaluate("game.cancel_shot(game.state().mission.queue[2].id)")
        .unwrap();
    assert_eq!(ui.mission(), lua.state.borrow().mission());
    for _ in 0..3 {
        ui.dispatch(C::NextTurn).unwrap();
        lua.evaluate("game.command('next_turn')").unwrap();
    }
    assert_eq!(ui.world(), lua.state.borrow().world());
    assert_eq!(ui.mission(), lua.state.borrow().mission());
}

#[test]
fn four_pending_shots_is_a_shared_atomic_limit_and_launches_free_capacity() {
    let mut ui = Session::blockade();
    let mut lua = phase_vector::repl::Host::with_session(Session::blockade());
    assert_eq!(
        lua.evaluate("game.state().mission.queue_capacity").unwrap(),
        MAX_QUEUED_SHOTS.to_string()
    );
    for _ in 0..MAX_QUEUED_SHOTS {
        ui.dispatch(C::QueueShot(ui.aim().unwrap().target)).unwrap();
        lua.evaluate("game.fire(game.aim())").unwrap();
    }
    let full = ui.mission().unwrap().clone();
    assert!(
        ui.dispatch(C::QueueShot(V::ZERO))
            .unwrap_err()
            .contains("full")
    );
    assert!(
        lua.evaluate("game.fire(0, 0)")
            .unwrap_err()
            .contains("full")
    );
    let mut direct = full.clone();
    assert!(
        direct
            .queue(
                &ui.world().ships[0],
                V::ZERO,
                full.config.reload_ticks * MAX_QUEUED_SHOTS as u64
            )
            .unwrap_err()
            .contains("full")
    );
    assert_eq!(direct, full);
    assert_eq!(ui.mission().unwrap(), &full);
    assert_eq!(lua.state.borrow().mission().unwrap(), &full);
    ui.dispatch(C::Step).unwrap();
    lua.evaluate("game.command('step')").unwrap();
    assert_eq!(ui.mission().unwrap().queued.len(), MAX_QUEUED_SHOTS - 1);
    assert_eq!(ui.world().projectiles.len(), 1);
    ui.dispatch(C::QueueShot(ui.aim().unwrap().target)).unwrap();
    lua.evaluate("game.fire(game.aim())").unwrap();
    assert_eq!(ui.mission().unwrap().queued.len(), MAX_QUEUED_SHOTS);
    assert_eq!(ui.mission(), lua.state.borrow().mission());
    assert_eq!(ui.world(), lua.state.borrow().world());
}

#[test]
fn active_salvo_is_frozen_and_running_edits_start_at_the_next_command_boundary() {
    use phase_vector::timing::{INTERVAL_TICKS, seconds_to_ticks};
    let mut s = Session::blockade();
    for _ in 0..MAX_QUEUED_SHOTS {
        s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    }
    let orders = s.mission().unwrap().queued.clone();
    assert_eq!(
        orders.iter().map(|o| o.launch_tick).collect::<Vec<_>>(),
        [0, 30, 60, 90].map(seconds_to_ticks)
    );
    s.dispatch(C::StepForward).unwrap();
    assert_eq!(s.mission().unwrap().shots.len(), 1);
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    let frozen = s.snapshot().planning_ship_at(orders[1].launch_tick);
    assert!(s.dispatch(C::CancelQueuedShot(orders[1].id)).is_err());
    s.dispatch(C::PlanBurn(V::new(scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    assert_eq!(s.snapshot().planning_ship_at(orders[1].launch_tick), frozen);
    s.dispatch(C::CancelShot).unwrap();
    assert_eq!(s.mission().unwrap().queued, orders[1..4]);
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    assert_eq!(s.mission().unwrap().queued[3].launch_tick, INTERVAL_TICKS);
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.mission().unwrap().shots.len(), 4);
    assert_eq!(s.mission().unwrap().queued.len(), 1);
    assert_eq!(s.snapshot().command_log[0].shots, orders[..4]);
    s.dispatch(C::Step).unwrap();
    assert_eq!(s.mission().unwrap().shots.len(), 5);
}

#[test]
fn each_salvo_round_fires_before_its_ticks_burn_with_the_actual_ship_velocity() {
    use phase_vector::timing::INTERVAL_TICKS;
    let mut s = Session::blockade();
    s.dispatch(C::PlanBurn(V::new(scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    for _ in 0..4 {
        s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    }
    let orders = s.mission().unwrap().queued.clone();
    let predictions: Vec<_> = orders
        .iter()
        .map(|o| s.snapshot().planning_ship_at(o.launch_tick))
        .collect();
    s.dispatch(C::Resume).unwrap();
    s.dispatch(C::Pause).unwrap();
    for (i, order) in orders.iter().enumerate() {
        if i > 0 {
            while s.world().tick < order.launch_tick {
                s.dispatch(C::Step).unwrap();
            }
            assert_eq!(s.mission().unwrap().shots.len(), i);
            assert_eq!(s.world().ships[0], predictions[i]);
            s.dispatch(C::Step).unwrap();
        }
        let shot = &s.mission().unwrap().shots[i];
        let solution = solve_shot(&predictions[i], order.target, order.launch_tick).unwrap();
        assert_eq!(shot.order_id, order.id);
        assert_eq!(shot.arrival_seconds, solution.arrival_seconds);
        assert_eq!(shot.launch_tick, order.launch_tick);
        assert_eq!(shot.origin, predictions[i].position);
        assert_eq!(
            shot.velocity,
            predictions[i].velocity + solution.direction * scenario::RAILGUN_SPEED
        );
    }
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.world().tick, INTERVAL_TICKS);
    assert_eq!(s.mission().unwrap().shots.len(), 4);
}

#[test]
fn invalid_later_shot_rejects_the_whole_interval_before_any_launch() {
    let mut s = Session::blockade();
    let target = s
        .snapshot()
        .planning_ship_at(Config::default().reload_ticks)
        .position;
    s.dispatch(C::PlanBurn(V::new(scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    s.dispatch(C::QueueShot(target)).unwrap();
    s.dispatch(C::PlanBurn(V::ZERO)).unwrap();
    let world = s.world().clone();
    let mission = s.mission().unwrap().clone();
    assert!(s.dispatch(C::Resume).is_err());
    assert_eq!(s.world(), &world);
    assert_eq!(s.mission().unwrap(), &mission);
    assert!(s.snapshot().command_log.is_empty());
    s.dispatch(C::CancelQueuedShot(mission.queued[1].id))
        .unwrap();
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.mission().unwrap().shots.len(), 1);
}
