use phase_vector::{
    ai::{self, Decision, Policy},
    mission::{Config, Mission, Outcome},
    observation::Track,
    physics::{Asteroid, Burn, STEP_SECONDS, Ship, Vector2 as V, World},
    scenario,
    session::{Command as C, Session},
};

fn rock(position: V, velocity: V, radius: f64) -> Asteroid {
    Asteroid::new(1, position, velocity, radius).unwrap()
}
fn ship(id: u32, position: V, velocity: V) -> Ship {
    Ship::new(id, position, velocity, 1.0, 10.0, 100.0).unwrap()
}
fn end(s: &mut Session) {
    while !s.finished() {
        s.dispatch(C::NextTurn).unwrap();
    }
}

#[test]
fn fast_round_hits_moving_rock_and_does_not_tunnel_or_damage_it() {
    let mut w = World::new(vec![]).unwrap();
    w.asteroids
        .push(rock(V::new(0.0, -50.0), V::new(0.0, 400.0), 2.0));
    w.launch(1, V::new(-100.0, 0.0), V::new(800.0, 0.0), 0.0)
        .unwrap();
    w.step(&[]).unwrap();
    assert!(w.projectiles.is_empty());
    assert!(w.impacts.is_empty());
    let hit = &w.asteroid_impacts[0];
    assert!(hit.fraction > 0.48 && hit.fraction < 0.5);
    assert_eq!(w.asteroids[0].position, V::new(0.0, 50.0));
    assert_eq!(w.asteroids[0].radius, 2.0);
    assert_eq!(w.asteroids[0].velocity, V::new(0.0, 400.0));
}

#[test]
fn spatial_crossing_at_different_times_is_not_a_hit() {
    let mut w = World::new(vec![]).unwrap();
    w.asteroids
        .push(rock(V::new(0.0, -50.0), V::new(0.0, 200.0), 2.0));
    w.launch(1, V::new(-100.0, 0.0), V::new(800.0, 0.0), 0.0)
        .unwrap();
    w.step(&[]).unwrap();
    assert_eq!(w.projectiles.len(), 1);
    assert!(w.asteroid_impacts.is_empty());
}

#[test]
fn earliest_object_stops_rounds_for_both_factions_and_terrain_wins_ties() {
    for owner in [1, 2] {
        for rock_x in [-20.0, 0.0, 20.0] {
            let target = if owner == 1 { 2 } else { 1 };
            let mut w = World::new(vec![ship(target, V::ZERO, V::ZERO)]).unwrap();
            w.asteroids.push(rock(V::new(rock_x, 0.0), V::ZERO, 1.0));
            w.launch(owner, V::new(-100.0, 0.0), V::new(800.0, 0.0), 0.0)
                .unwrap();
            w.step(&[]).unwrap();
            assert!(w.projectiles.is_empty());
            assert_eq!(w.impacts.len(), usize::from(rock_x > 0.0));
            assert_eq!(
                w.asteroid_impacts
                    .iter()
                    .filter(|h| h.projectile_id.is_some())
                    .count(),
                usize::from(rock_x <= 0.0)
            );
        }
    }
}

#[test]
fn swept_ship_contact_disables_thrust_once_but_retains_inertia() {
    let mut w = World::new(vec![ship(1, V::new(-100.0, 0.0), V::new(800.0, 0.0))]).unwrap();
    w.asteroids.push(rock(V::ZERO, V::ZERO, 2.0));
    w.step(&[]).unwrap();
    assert!(w.ships[0].disabled);
    assert!(w.asteroid_impacts[0].fraction < 0.5);
    let before = w.ships[0].clone();
    w.step(&[Burn {
        ship_id: 1,
        acceleration: V::new(10.0, 0.0),
    }])
    .unwrap();
    assert_eq!(w.ships[0].velocity, before.velocity);
    assert_eq!(w.ships[0].remaining_dv, before.remaining_dv);
    assert_eq!(w.asteroid_impacts.len(), 1);
    assert!(phase_vector::mission::solve_shot(&w.ships[0], V::ZERO, w.tick).is_err());
}

#[test]
fn collision_outcomes_are_ordered_against_goal_and_deadline() {
    for (own, rock_x, expected) in [
        (true, -8.0, Outcome::OwnDisabled),
        (false, -8.0, Outcome::Disabled),
        (false, 8.0, Outcome::Breakthrough),
    ] {
        let moving_id = if own { 1 } else { 2 };
        let mut w = World::new(vec![
            ship(moving_id, V::new(-10.0, 0.0), V::new(80.0, 0.0)),
            ship(if own { 2 } else { 1 }, V::new(0.0, 100.0), V::ZERO),
        ])
        .unwrap();
        w.asteroids.push(rock(V::new(rock_x, 0.0), V::ZERO, 1.0));
        let mut m = Mission::new(
            &w,
            1,
            Config {
                goal: V::ZERO,
                goal_radius: 1.0,
                deadline_tick: 1,
                ..Config::default()
            },
        )
        .unwrap();
        m.step(&mut w, 1, V::ZERO);
        assert_eq!(m.outcome, expected);
        assert!(m.ended_seconds.unwrap() <= STEP_SECONDS);
    }
}

#[test]
fn collision_ties_favor_own_loss_and_impact_at_deadline_precedes_timeout() {
    let mut w = World::new(vec![ship(1, V::ZERO, V::ZERO), ship(2, V::ZERO, V::ZERO)]).unwrap();
    w.asteroids.push(rock(V::ZERO, V::ZERO, 1.0));
    let mut m = Mission::new(
        &w,
        1,
        Config {
            goal: V::ZERO,
            ..Config::default()
        },
    )
    .unwrap();
    m.step(&mut w, 1, V::ZERO);
    assert_eq!(m.outcome, Outcome::OwnDisabled);
    assert_eq!(m.ended_seconds, Some(0.0));
    let mut w = World::new(vec![
        ship(1, V::new(0.0, 100.0), V::ZERO),
        ship(2, V::new(-22.0, 0.0), V::new(80.0, 0.0)),
    ])
    .unwrap();
    w.asteroids.push(rock(V::ZERO, V::ZERO, 1.0));
    let mut m = Mission::new(
        &w,
        1,
        Config {
            deadline_tick: 1,
            goal: V::new(1000.0, 0.0),
            goal_radius: 1.0,
            ..Config::default()
        },
    )
    .unwrap();
    m.step(&mut w, 1, V::ZERO);
    assert_eq!(m.outcome, Outcome::Disabled);
    assert_eq!(m.ended_seconds, Some(STEP_SECONDS));
}

#[test]
fn preview_cover_time_matches_actual_contact_and_retry_restores_ephemeris() {
    let mut s = Session::blockade();
    let original = s.world().clone();
    let solution = s.aim().unwrap();
    let hit = solution.obstruction.unwrap();
    s.dispatch(C::QueueShot(solution.target)).unwrap();
    let forecast = s.snapshot().forecast(120.0).unwrap();
    assert!(forecast.rounds.is_empty());
    s.dispatch(C::NextTurn).unwrap();
    let actual = &s.world().asteroid_impacts[0];
    assert!(((actual.tick as f64 + actual.fraction) * STEP_SECONDS - hit.seconds).abs() < 1e-8);
    assert!((actual.position - hit.position).length() < 1e-6);
    assert_eq!(s.world().asteroids, forecast.asteroids);
    s.dispatch(C::Reset).unwrap();
    assert_eq!(s.world(), &original);
}

#[test]
fn queued_cover_recalculates_with_maneuver_but_flying_rounds_do_not() {
    let mut s = Session::blockade();
    let target = s.snapshot().aim_at(240.0).unwrap().target;
    s.dispatch(C::QueueLaunchShot(target, 240.0)).unwrap();
    let order = s.mission().unwrap().queued[0].clone();
    assert!(
        s.snapshot()
            .preview_order(&order)
            .unwrap()
            .obstruction
            .is_some()
    );
    s.dispatch(C::PlanBurn(V::new(0.0, -scenario::MAX_ACCELERATION)))
        .unwrap();
    assert_eq!(s.mission().unwrap().queued[0], order);
    assert!(
        s.snapshot()
            .preview_order(&order)
            .unwrap()
            .obstruction
            .is_none()
    );
    s.dispatch(C::NextTurn).unwrap();
    s.dispatch(C::NextTurn).unwrap();
    s.dispatch(C::StepForward).unwrap();
    let projectile = s.world().projectiles[0].clone();
    s.dispatch(C::PlanBurn(V::ZERO)).unwrap();
    assert_eq!(s.world().projectiles[0], projectile);
}

#[test]
fn own_collision_is_forecast_and_prevents_future_launch() {
    let mut w = scenario::initial_world();
    let own = &w.ships[0];
    w.asteroids
        .push(rock(own.position + own.velocity * 1.0, V::ZERO, 1000.0));
    let s = Session::with_mission(w, 1, Config::default()).unwrap();
    let f = s.snapshot().forecast(10.0).unwrap();
    assert!(f.own.disabled);
    assert!(f.own_collision.is_some());
    assert!(
        s.snapshot()
            .planned_route(V::ZERO)
            .forecast
            .unwrap()
            .collision
            .is_some()
    );
    assert!(s.snapshot().preview_launch_shot(V::ZERO, 10.0).is_err());
}

#[test]
fn ai_uses_cover_without_hidden_rounds_and_ignores_stale_tracks_past_cover() {
    let own = ship(2, V::ZERO, V::ZERO);
    let track = Track {
        id: 7,
        position: V::new(-1000.0, 0.0),
        velocity: V::new(10.0, 0.0),
        radius: 0.0,
        sampled_tick: 0,
        received_tick: 0,
    };
    let terrain = vec![rock(V::new(-500.0, 0.0), V::ZERO, 50.0)];
    let policy = Policy {
        safety_radius: 1.0,
        ..Policy::default()
    };
    let normal = ai::decide(
        &own,
        V::ZERO,
        0.0,
        std::slice::from_ref(&track),
        0,
        policy,
        &Decision::default(),
    );
    assert!(normal.threat_id.is_some());
    for now in [0, 240] {
        let covered = ai::decide_with_terrain(
            &own,
            V::ZERO,
            0.0,
            (std::slice::from_ref(&track), &terrain),
            now,
            policy,
            &Decision::default(),
        );
        assert_eq!(covered.acceleration, V::ZERO);
        assert_eq!(covered.threat_id, None);
    }
    let approaching = ship(2, V::new(-1000.0, 0.0), V::new(10.0, 0.0));
    let avoid = ai::decide_with_terrain(
        &approaching,
        V::new(1000.0, 0.0),
        10.0,
        (&[], &terrain),
        0,
        policy,
        &Decision::default(),
    );
    assert!(avoid.reason.contains("asteroid"));
    assert!(avoid.acceleration.length() > 0.0);
}

#[test]
fn same_twelve_shots_need_both_maneuver_and_placement_to_deny_without_hits() {
    for (burn, spread, expected) in [
        (0.0, 900_000.0, Outcome::Breakthrough),
        (-scenario::MAX_ACCELERATION, 900_000.0, Outcome::Timeout),
        (-scenario::MAX_ACCELERATION, 0.0, Outcome::Breakthrough),
    ] {
        let mut s = Session::blockade();
        s.dispatch(C::PlanBurn(V::new(0.0, burn))).unwrap();
        for _ in 0..2 {
            s.dispatch(C::NextTurn).unwrap();
        }
        s.dispatch(C::PlanBurn(V::ZERO)).unwrap();
        for _ in 0..3 {
            for i in 0..4 {
                s.dispatch(C::QueueShot(
                    s.aim().unwrap().target + V::new(0.0, i as f64 * spread),
                ))
                .unwrap();
            }
            s.dispatch(C::NextTurn).unwrap();
        }
        end(&mut s);
        assert_eq!(s.mission().unwrap().outcome, expected);
        assert_eq!(s.mission().unwrap().shots.len(), 12);
        assert!(s.world().impacts.is_empty());
        assert_eq!(
            s.world().asteroid_impacts.len(),
            if burn == 0.0 { 12 } else { 0 }
        );
        assert!(s.world().ships[1].remaining_dv > 0.0);
        let first = s.world().clone();
        let records = s.snapshot().command_log.to_vec();
        let mut replay = Session::blockade();
        // Replay exact fixed targets and launch ticks rather than recomputing lead.
        for order in s.mission().unwrap().shots.iter().take(4) {
            replay
                .dispatch(C::QueueLaunchShot(
                    order.target,
                    order.launch_tick as f64 * STEP_SECONDS,
                ))
                .unwrap();
        }
        for record in records {
            replay
                .dispatch(C::PlanBurn(record.burn.acceleration))
                .unwrap();
            // Queue later salvos only when their command interval becomes editable.
            if record.start_tick > 0 {
                for order in s.mission().unwrap().shots.iter().skip(4).filter(|o| {
                    o.launch_tick / phase_vector::timing::INTERVAL_TICKS
                        == record.start_tick / phase_vector::timing::INTERVAL_TICKS
                }) {
                    replay
                        .dispatch(C::QueueLaunchShot(
                            order.target,
                            order.launch_tick as f64 * STEP_SECONDS,
                        ))
                        .unwrap();
                }
            }
            replay.dispatch(C::NextTurn).unwrap();
        }
        assert_eq!(replay.world(), &first);
    }
    let mut idle = Session::blockade();
    end(&mut idle);
    assert_eq!(idle.mission().unwrap().outcome, Outcome::Breakthrough);
    assert!(idle.world().asteroid_impacts.is_empty());
}

#[test]
fn lua_exposes_the_same_cover_and_forecast_and_custom_retry_restores_terrain() {
    let mut host = phase_vector::repl::Host::with_session(Session::blockade());
    assert_eq!(
        host.evaluate("return #game.state().asteroids, game.state().ship.disabled")
            .unwrap(),
        "1\tfalse"
    );
    assert_eq!(host.evaluate("local x,y=game.aim_at(240); local s=game.preview_launch(x,y,240); return s.blocked_by, s.blocked_seconds > 240").unwrap(),"1\ttrue");
    let value = host
        .evaluate("return game.forecast(240).asteroids[1].y")
        .unwrap()
        .parse::<f64>()
        .unwrap();
    let initial = scenario::default_asteroid();
    assert_eq!(value, initial.position.y + initial.velocity.y * 240.0);
    let mut world = scenario::blockade_world();
    world.asteroids[0] = Asteroid::new(1, V::new(1e8, 2e7), V::new(100.0, -50.0), 1000.0).unwrap();
    let mut s = Session::with_mission(world.clone(), 1, Config::default()).unwrap();
    assert_eq!(
        s.snapshot().run_kind,
        phase_vector::parameters::RunKind::Custom
    );
    s.dispatch(C::NextTurn).unwrap();
    s.dispatch(C::Reset).unwrap();
    assert_eq!(s.world(), &world);
    assert!(Asteroid::new(1, V::ZERO, V::ZERO, 0.0).is_err());
    assert!(Asteroid::new(1, V::new(f64::NAN, 0.0), V::ZERO, 1.0).is_err());
}
