use phase_vector::{
    evaluation::evaluate,
    map::MapView,
    mission::{Config, Outcome},
    parameters::{Parameter as P, RunKind},
    physics::Vector2 as V,
    presentation::State,
    repl::Host,
    scenario,
    session::{Command as C, Session},
    timing::seconds_to_ticks,
};

fn finish(s: &mut Session) {
    while !s.finished() {
        s.dispatch(C::NextTurn).unwrap();
    }
}

#[test]
fn no_final_score_before_result_and_defeat_gives_no_economy_points() {
    let mut s = Session::blockade();
    assert!(evaluate(&s.snapshot()).is_none());
    finish(&mut s);
    assert_eq!(s.mission().unwrap().outcome, Outcome::Breakthrough);
    let score = evaluate(&s.snapshot()).unwrap();
    assert_eq!(
        (
            score.total,
            score.victory_points,
            score.shot_points,
            score.fuel_points
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(score.shots_fired, 0);
    assert_eq!(score.spent_delta_v, 0.0);
    s.dispatch(C::Reset).unwrap();
    assert!(evaluate(&s.snapshot()).is_none());
}

#[test]
fn timeout_counts_actual_launches_and_normalizes_fuel_to_initial_budget() {
    let mut world = scenario::initial_world();
    world.ships[0].remaining_dv = 1200.0;
    let mut s = Session::with_mission(
        world,
        1,
        Config {
            deadline_tick: seconds_to_ticks(60),
            ..Config::default()
        },
    )
    .unwrap();
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    s.dispatch(C::PlanBurn(V::new(10.0, 0.0))).unwrap();
    let mut stepped = s.clone();
    finish(&mut s);
    while !stepped.finished() {
        stepped.dispatch(C::Step).unwrap();
    }
    assert_eq!(evaluate(&stepped.snapshot()), evaluate(&s.snapshot()));
    assert_eq!(stepped.world(), s.world());
    assert_eq!(s.mission().unwrap().outcome, Outcome::Timeout);
    let score = evaluate(&s.snapshot()).unwrap();
    assert_eq!(
        (
            score.total,
            score.victory_points,
            score.shot_points,
            score.fuel_points
        ),
        (1710, 1000, 460, 250)
    );
    assert_eq!(score.shots_fired, 2);
    assert_eq!(score.spent_delta_v, 600.0);
    let ended = s.world().clone();
    for speed in [1.0, 32.0, 64.0, 128.0] {
        s.dispatch(C::SetSpeed(speed)).unwrap();
        for _ in 0..10 {
            s.pulse();
        }
        assert_eq!(evaluate(&s.snapshot()).unwrap(), score);
    }
    assert_eq!(s.world(), &ended);
}

#[test]
fn early_hit_earns_fixed_disablement_points_without_charging_unfired_reservations() {
    let mut world = scenario::initial_world();
    world.ships[0].velocity = V::ZERO;
    world.ships[1].position = world.ships[0].position + V::new(1000.0, 0.0);
    world.ships[1].velocity = V::ZERO;
    world.ships[1].remaining_dv = 0.0;
    let mut s = Session::with_mission(world, 1, Config::default()).unwrap();
    for _ in 0..4 {
        s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    }
    s.dispatch(C::Step).unwrap();
    assert_eq!(s.mission().unwrap().outcome, Outcome::Disabled);
    assert_eq!(s.mission().unwrap().queued.len(), 3);
    let score = evaluate(&s.snapshot()).unwrap();
    assert_eq!(
        (
            score.total,
            score.victory_points,
            score.shot_points,
            score.fuel_points
        ),
        (2980, 2000, 480, 500)
    );
    assert_eq!(score.shots_fired, 1);
}

#[test]
fn shot_bonus_saturates_and_zero_initial_fuel_has_no_fuel_bonus() {
    let mut world = scenario::initial_world();
    world.ships[0].remaining_dv = 0.0;
    let mut s = Session::with_mission(
        world,
        1,
        Config {
            deadline_tick: seconds_to_ticks(840),
            ..Config::default()
        },
    )
    .unwrap();
    while !s.finished() {
        while s.next_shot_tick().is_ok() {
            s.dispatch(C::QueueShot(V::new(-1e9, 1e9))).unwrap();
        }
        s.dispatch(C::NextTurn).unwrap();
    }
    let score = evaluate(&s.snapshot()).unwrap();
    assert!(score.shots_fired > 25);
    assert_eq!(
        (score.total, score.shot_points, score.fuel_points),
        (1000, 0, 0)
    );
}

#[test]
fn enemy_depletion_uses_the_custom_initial_budget_and_only_rewards_timeout() {
    let mut world = scenario::initial_world();
    world.ships[1].remaining_dv = 1200.0;
    let mut s = Session::with_mission(
        world,
        1,
        Config {
            deadline_tick: seconds_to_ticks(60),
            ..Config::default()
        },
    )
    .unwrap();
    finish(&mut s);
    assert_eq!(s.snapshot().initial_enemy_delta_v, 1200.0);
    // Controlled terminal budgets isolate scoring from AI tuning and sensor delays.
    for (remaining, bonus) in [
        (1200.0, 0),
        (900.0, 125),
        (600.0, 250),
        (300.0, 375),
        (0.0, 500),
        (1199.0, 0),
        (1197.0, 1),
    ] {
        let mut terminal = s.world().clone();
        terminal.ships[1].remaining_dv = remaining;
        for (outcome, points, depletion) in [
            (Outcome::Timeout, 1000 + bonus, bonus),
            (Outcome::Disabled, 2000, 0),
            (Outcome::Breakthrough, 0, 0),
            (Outcome::OwnDisabled, 0, 0),
        ] {
            let mut mission = s.mission().unwrap().clone();
            mission.outcome = outcome;
            let mut snapshot = s.snapshot();
            snapshot.world = &terminal;
            snapshot.mission = Some(&mission);
            let score = evaluate(&snapshot).unwrap();
            assert_eq!(score.victory_points, points);
            assert_eq!(score.enemy_depletion_points, depletion);
            assert_eq!(score.enemy_initial_delta_v, 1200.0);
            assert_eq!(score.enemy_remaining_delta_v, remaining);
            assert_eq!(score.total, if points > 0 { points + 1000 } else { 0 });
        }
    }
    s.dispatch(C::Reset).unwrap();
    assert_eq!(s.snapshot().initial_enemy_delta_v, 1200.0);
    assert!(evaluate(&s.snapshot()).is_none());
    finish(&mut s);
    assert_eq!(evaluate(&s.snapshot()).unwrap().enemy_depletion_points, 0);
}

#[test]
fn zero_initial_enemy_budget_has_no_depletion_bonus_and_does_not_end_the_mission() {
    let mut world = scenario::initial_world();
    world.ships[1].remaining_dv = 0.0;
    let mut s = Session::with_mission(
        world,
        1,
        Config {
            deadline_tick: seconds_to_ticks(60),
            ..Config::default()
        },
    )
    .unwrap();
    s.dispatch(C::Step).unwrap();
    assert!(!s.finished());
    assert!(evaluate(&s.snapshot()).is_none());
    finish(&mut s);
    let score = evaluate(&s.snapshot()).unwrap();
    assert_eq!(score.enemy_initial_delta_v, 0.0);
    assert_eq!(score.enemy_remaining_delta_v, 0.0);
    assert_eq!(score.enemy_depletion_points, 0);
    assert_eq!(score.victory_points, 1000);
    assert_eq!(score.total, 2000);
}

#[test]
fn terrain_disablement_earns_the_same_fixed_victory_points_as_a_hit() {
    let mut world = scenario::initial_world();
    let enemy = &world.ships[1];
    world.asteroids.push(
        phase_vector::physics::Asteroid::new(1, enemy.position, enemy.velocity, 1000.0).unwrap(),
    );
    let mut s = Session::with_mission(world, 1, Config::default()).unwrap();
    s.dispatch(C::Step).unwrap();
    assert_eq!(s.mission().unwrap().outcome, Outcome::Disabled);
    let score = evaluate(&s.snapshot()).unwrap();
    assert_eq!(score.victory_points, 2000);
    assert_eq!(score.enemy_depletion_points, 0);
    assert_eq!(score.total, 3000);
}

#[test]
fn lua_and_ui_expose_identical_scores_and_debug_runs_keep_their_label() {
    let mut s = Session::blockade();
    s.dispatch(C::Configure(P::DeadlineSeconds, 120.0)).unwrap();
    s.dispatch(C::Step).unwrap();
    s.dispatch(C::Configure(P::ReactionSeconds, 60.0)).unwrap();
    s.dispatch(C::Configure(P::ReactionSeconds, 120.0)).unwrap();
    finish(&mut s);
    assert_eq!(s.snapshot().run_kind, RunKind::Debug);
    let json = serde_json::to_value(State::new(&s.snapshot(), &MapView::default())).unwrap();
    assert_eq!(json["mission"]["run_kind"], "debug");
    let mut host = Host::with_session(s);
    for (name, value) in json["mission"]["evaluation"].as_object().unwrap() {
        let from_lua: f64 = host
            .evaluate(&format!("game.state().mission.evaluation.{name}"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(from_lua, value.as_f64().unwrap(), "{name}");
    }
    host.evaluate("game.command('retry')").unwrap();
    assert_eq!(
        host.evaluate("game.state().mission.run_kind").unwrap(),
        "custom"
    );
    assert_eq!(
        host.evaluate("game.state().mission.evaluation").unwrap(),
        "nil"
    );
}
