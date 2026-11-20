use phase_vector::{
    ai::{self, Decision, Policy},
    mission::{Config, Mission, Outcome, solve_shot},
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
    assert_eq!(
        event_fixture(V::new(3.6, 0.0), 1.0, None).outcome,
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
            s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
        }
        s.dispatch(C::NextTurn).unwrap();
    }
    let mut replay = Session::blockade();
    for record in s.snapshot().command_log {
        assert_eq!(replay.world().tick, record.start_tick);
        replay
            .dispatch(C::PlanBurn(record.burn.acceleration))
            .unwrap();
        if let Some(target) = record.shot_target {
            replay.dispatch(C::QueueShot(target)).unwrap();
        }
        replay.dispatch(C::NextTurn).unwrap();
    }
    assert_eq!(s.world(), replay.world());
    assert_eq!(s.mission(), replay.mission());
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
    reference.dispatch(C::QueueShot(target)).unwrap();
    while !reference.finished() {
        reference.dispatch(C::NextTurn).unwrap();
    }
    for speed in [1.0, 32.0, 64.0, 128.0] {
        let mut candidate = Session::blockade();
        candidate.dispatch(C::QueueShot(target)).unwrap();
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
