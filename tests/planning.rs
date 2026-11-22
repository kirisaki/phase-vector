use phase_vector::{
    mission::{Config, Mission},
    physics::{STEP_SECONDS, Vector2 as V},
    planning::ARRIVAL_TOLERANCE_SECONDS,
    scenario,
    session::{Command as C, Session, Snapshot},
};

fn request(s: &Session, tick: u64, flight: f64) -> (V, f64) {
    let ship = s.snapshot().planning_ship_at(tick);
    (
        ship.position + (ship.velocity + V::new(scenario::RAILGUN_SPEED, 0.0)) * flight,
        tick as f64 * STEP_SECONDS + flight,
    )
}

#[test]
fn timed_solution_preserves_muzzle_speed_and_reaches_the_fixed_point() {
    let mut s = Session::blockade();
    s.dispatch(C::PlanBurn(V::new(scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    let (target, arrival) = request(&s, 600, 300.0);
    let solution = s.snapshot().preview_timed_shot(target, arrival).unwrap();
    assert_eq!(solution.launch_tick, 600);
    let ship = s.snapshot().planning_ship_at(solution.launch_tick);
    let velocity = ship.velocity + solution.direction * scenario::RAILGUN_SPEED;
    assert!(((velocity - ship.velocity).length() - scenario::RAILGUN_SPEED).abs() < 1e-6);
    assert!((solution.origin + velocity * solution.flight_seconds - target).length() < 1e-5);
    assert!((solution.arrival_seconds - arrival).abs() <= ARRIVAL_TOLERANCE_SECONDS);
    let nearby = s
        .snapshot()
        .preview_timed_shot(target, arrival + 0.08)
        .unwrap();
    assert!((nearby.arrival_seconds - arrival - 0.08).abs() <= ARRIVAL_TOLERANCE_SECONDS);
}

#[test]
fn requests_reject_invalid_times_and_reload_conflicts_atomically() {
    let mut s = Session::blockade();
    let (target, arrival) = request(&s, 600, 300.0);
    for time in [
        f64::NAN,
        f64::INFINITY,
        -1.0,
        0.0,
        1.0,
        Config::default().deadline_tick as f64 * STEP_SECONDS + 1.0,
    ] {
        assert!(s.dispatch(C::QueueTimedShot(target, time)).is_err());
        assert!(s.mission().unwrap().queued.is_empty());
    }
    s.dispatch(C::QueueTimedShot(target, arrival)).unwrap();
    let before = s.mission().unwrap().clone();
    assert!(
        s.dispatch(C::QueueTimedShot(target, arrival))
            .unwrap_err()
            .contains("reload")
    );
    assert_eq!(s.mission().unwrap(), &before);
    assert_eq!(s.world().tick, 0);
}

#[test]
fn maneuver_changes_recompute_timed_launches_without_changing_targets() {
    let mut s = Session::blockade();
    let (target, arrival) = request(&s, 600, 300.0);
    s.dispatch(C::QueueTimedShot(target, arrival)).unwrap();
    let before = s.mission().unwrap().queued[0].clone();
    s.dispatch(C::PlanBurn(V::new(scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    let after = &s.mission().unwrap().queued[0];
    assert_eq!(after.id, before.id);
    assert_eq!(after.target, target);
    assert_eq!(after.requested_arrival_seconds, Some(arrival));
    assert_ne!(after.launch_tick, before.launch_tick);
    assert!(after.planning_error.is_none());
    assert!(
        (s.snapshot().preview_order(after).unwrap().arrival_seconds - arrival).abs()
            <= ARRIVAL_TOLERANCE_SECONDS
    );
}

#[test]
fn invalid_replans_remain_visible_block_execution_and_can_recover_or_cancel() {
    let mut s = Session::blockade();
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    let (target, arrival) = request(&s, Config::default().reload_ticks, 300.0);
    s.dispatch(C::QueueTimedShot(target, arrival)).unwrap();
    let id = s.mission().unwrap().queued[1].id;
    s.dispatch(C::PlanBurn(V::new(-scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    assert!(
        s.mission()
            .unwrap()
            .queued
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .planning_error
            .is_some()
    );
    let before = s.world().clone();
    assert!(s.dispatch(C::Commit).is_err());
    assert_eq!(s.world(), &before);
    s.dispatch(C::PlanBurn(V::ZERO)).unwrap();
    assert!(
        s.mission()
            .unwrap()
            .queued
            .iter()
            .all(|o| o.planning_error.is_none())
    );
    s.dispatch(C::PlanBurn(V::new(-scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    s.dispatch(C::CancelQueuedShot(id)).unwrap();
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.mission().unwrap().shots.len(), 1);
}

#[test]
fn locked_timed_orders_and_flying_rounds_ignore_future_maneuvers() {
    let mut s = Session::blockade();
    let (target, arrival) = request(&s, 120, 300.0);
    s.dispatch(C::QueueTimedShot(target, arrival)).unwrap();
    s.dispatch(C::StepForward).unwrap();
    let locked = s.mission().unwrap().queued.clone();
    s.dispatch(C::PlanBurn(V::new(scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    assert_eq!(s.mission().unwrap().queued, locked);
    assert!(s.dispatch(C::CancelQueuedShot(locked[0].id)).is_err());
    s.dispatch(C::NextTurn).unwrap();
    let shot = s.mission().unwrap().shots[0].clone();
    assert!((shot.arrival_seconds - arrival).abs() <= ARRIVAL_TOLERANCE_SECONDS);
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.mission().unwrap().shots[0], shot);
    assert_eq!(s.world().projectiles[0].velocity, shot.velocity);
}

#[test]
fn forecast_is_read_only_and_matches_friendly_and_ballistic_execution() {
    let mut s = Session::blockade();
    s.dispatch(C::PlanBurn(V::new(scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    let (target, arrival) = request(&s, 600, 300.0);
    s.dispatch(C::QueueTimedShot(target, arrival)).unwrap();
    let before = s.world().clone();
    let forecast = s.snapshot().forecast(240.0).unwrap();
    assert_eq!(s.world(), &before);
    assert_eq!(forecast.rounds.len(), 1);
    assert!(forecast.rounds[0].reserved);
    assert_eq!(forecast.own_path.first(), Some(&before.ships[0].position));
    assert_eq!(forecast.own_path.last(), Some(&forecast.own.position));
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.mission().unwrap().queued.len(), 1);
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.world().ships[0], forecast.own);
    assert!((s.world().projectiles[0].position - forecast.rounds[0].position).length() < 1e-4);
    assert_eq!(
        s.world().projectiles[0].velocity,
        forecast.rounds[0].velocity
    );
    assert!(!s.snapshot().forecast(300.0).unwrap().rounds[0].reserved);
}

#[test]
fn enemy_forecast_never_consults_hidden_motion_or_ai() {
    let s = Session::blockade();
    let expected = s.snapshot().forecast(480.0).unwrap();
    let mut hidden = s.world().clone();
    hidden.ships[1].position = V::ZERO;
    hidden.ships[1].velocity = V::new(900000.0, 900000.0);
    let view = Snapshot {
        world: &hidden,
        ..s.snapshot()
    };
    let actual = view.forecast(480.0).unwrap();
    assert_eq!(actual.enemy_position, expected.enemy_position);
    assert_eq!(actual.enemy_velocity, expected.enemy_velocity);
}

#[test]
fn lua_timed_commands_share_validation_and_replay_preserves_results() {
    let mut s = Session::blockade();
    let (target, arrival) = request(&s, 600, 300.0);
    let mut lua = phase_vector::repl::Host::with_session(s.clone());
    s.dispatch(C::QueueTimedShot(target, arrival)).unwrap();
    lua.evaluate(&format!(
        "game.fire_at({}, {}, {})",
        target.x, target.y, arrival
    ))
    .unwrap();
    assert_eq!(s.mission(), lua.state.borrow().mission());
    assert!(
        lua.evaluate(&format!(
            "game.fire_at({}, {}, {})",
            target.x, target.y, arrival
        ))
        .is_err()
    );
    assert_eq!(
        lua.evaluate("game.forecast(240).time_seconds").unwrap(),
        "240"
    );
    for _ in 0..3 {
        s.dispatch(C::NextTurn).unwrap();
    }
    let mut world = scenario::initial_world();
    let mut mission = Mission::new(&world, 1, Config::default()).unwrap();
    for record in s.snapshot().command_log {
        mission.queued = record.shots.clone();
        let end = record.start_tick + phase_vector::timing::INTERVAL_TICKS;
        while world.tick < end {
            mission.launch_queued(&mut world, 1).unwrap();
            mission.step(&mut world, 1, record.burn.acceleration);
        }
    }
    assert_eq!(s.world(), &world);
    assert_eq!(s.mission().unwrap().shots, mission.shots);
}

#[test]
fn invalid_future_order_allows_active_interval_to_finish_then_blocks_the_boundary() {
    let mut s = Session::blockade();
    s.dispatch(C::StepForward).unwrap();
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    let (target, arrival) = request(&s, 600, 300.0);
    s.dispatch(C::QueueTimedShot(target, arrival)).unwrap();
    let id = s.mission().unwrap().queued[1].id;
    s.dispatch(C::PlanBurn(V::new(-scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    assert!(
        s.mission()
            .unwrap()
            .queued
            .iter()
            .any(|o| o.planning_error.is_some())
    );
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.world().tick, phase_vector::timing::INTERVAL_TICKS);
    assert!(s.dispatch(C::Commit).is_err());
    assert!(s.mission().unwrap().shots.is_empty());
    s.dispatch(C::CancelQueuedShot(id)).unwrap();
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.mission().unwrap().shots.len(), 1);
}

#[test]
fn selected_launch_leads_beyond_the_preview_and_fires_on_that_tick() {
    let mut s = Session::blockade();
    let firing = 120.0;
    let forecast = s.snapshot().forecast(firing).unwrap();
    let lead = s.snapshot().aim_at(firing).unwrap();
    assert!(lead.flight_seconds > 300.0);
    assert!((lead.target - forecast.enemy_position).length() > 10_000_000.0);
    assert!(
        (lead.target - forecast.enemy_position - forecast.enemy_velocity * lead.flight_seconds)
            .length()
            < 1e-5
    );
    assert_eq!(forecast.aim.unwrap(), lead);
    s.dispatch(C::QueueLaunchShot(lead.target, firing)).unwrap();
    s.dispatch(C::NextTurn).unwrap();
    assert!(s.mission().unwrap().shots.is_empty());
    s.dispatch(C::NextTurn).unwrap();
    let shot = &s.mission().unwrap().shots[0];
    assert_eq!(shot.launch_tick, (firing / STEP_SECONDS) as u64);
    assert_eq!(shot.origin, forecast.own.position);
    assert_eq!(shot.target, lead.target);
    assert!((shot.arrival_seconds - lead.arrival_seconds).abs() < 1e-9);
}

#[test]
fn fixed_launch_validation_and_four_rounds_preserve_exact_schedule() {
    let mut s = Session::blockade();
    let target = s.aim().unwrap().target;
    for seconds in [
        f64::NAN,
        f64::INFINITY,
        -1.0,
        0.1,
        Config::default().deadline_tick as f64 * STEP_SECONDS,
    ] {
        let before = s.mission().unwrap().clone();
        assert!(s.dispatch(C::QueueLaunchShot(target, seconds)).is_err());
        assert_eq!(s.mission().unwrap(), &before);
    }
    for seconds in [0.0, 30.0, 60.0, 90.0] {
        let target = s.snapshot().aim_at(seconds).unwrap().target;
        s.dispatch(C::QueueLaunchShot(target, seconds)).unwrap();
        assert!(
            s.dispatch(C::QueueLaunchShot(target, seconds + STEP_SECONDS))
                .is_err()
        );
    }
    assert!(
        s.dispatch(C::QueueLaunchShot(target, 120.0))
            .unwrap_err()
            .contains("full")
    );
    s.dispatch(C::NextTurn).unwrap();
    let times: Vec<_> = s
        .mission()
        .unwrap()
        .shots
        .iter()
        .map(|shot| shot.launch_tick as f64 * STEP_SECONDS)
        .collect();
    assert_eq!(times, [0.0, 30.0, 60.0, 90.0]);
}

#[test]
fn launch_order_keeps_time_and_target_while_maneuver_recomputes_ballistics() {
    let mut s = Session::blockade();
    let lead = s.snapshot().aim_at(150.0).unwrap();
    s.dispatch(C::QueueLaunchShot(lead.target, 150.0)).unwrap();
    let order = s.mission().unwrap().queued[0].clone();
    s.dispatch(C::PlanBurn(V::new(scenario::MAX_ACCELERATION, 0.0)))
        .unwrap();
    assert_eq!(s.mission().unwrap().queued[0], order);
    let after = s.snapshot().preview_order(&order).unwrap();
    assert_eq!(after.launch_tick, lead.launch_tick);
    assert_eq!(after.target, lead.target);
    assert_ne!(after.origin, lead.origin);
    assert_ne!(after.arrival_seconds, lead.arrival_seconds);
    s.dispatch(C::StepForward).unwrap();
    let before = s.mission().unwrap().clone();
    assert!(
        s.dispatch(C::QueueLaunchShot(lead.target, 60.0))
            .unwrap_err()
            .contains("locked")
    );
    assert_eq!(s.mission().unwrap(), &before);
}

#[test]
fn timed_lead_uses_observation_and_lua_shares_fixed_launch_validation() {
    let mut s = Session::blockade();
    let lead = s.snapshot().aim_at(120.0).unwrap();
    let mut hidden = s.world().clone();
    hidden.ships[1].position = V::ZERO;
    hidden.ships[1].velocity = V::new(900000.0, 900000.0);
    assert_eq!(
        Snapshot {
            world: &hidden,
            ..s.snapshot()
        }
        .aim_at(120.0)
        .unwrap(),
        lead
    );
    let mut lua = phase_vector::repl::Host::with_session(s.clone());
    lua.evaluate("local x,y = game.aim_at(120); game.fire_at_launch(x,y,120)")
        .unwrap();
    s.dispatch(C::QueueLaunchShot(lead.target, 120.0)).unwrap();
    assert_eq!(s.mission(), lua.state.borrow().mission());
    assert!(
        lua.evaluate("local x,y = game.aim_at(120); game.fire_at_launch(x,y,120)")
            .is_err()
    );
    assert_eq!(
        lua.evaluate(
            "local x,y = game.aim_at(150); return game.preview_launch(x,y,150).launch_seconds"
        )
        .unwrap(),
        "150"
    );
}

#[test]
fn firing_window_markers_follow_reservations_cancellation_and_capacity() {
    let mut s = Session::blockade();
    let reload = Config::default().reload_ticks;
    let windows = |s: &Session| {
        s.mission()
            .unwrap()
            .available_launch_ticks(s.snapshot().order_start_tick)
    };
    assert_eq!(windows(&s), [0]);
    for time in [0.0, 120.0] {
        let target = s.snapshot().aim_at(time).unwrap().target;
        s.dispatch(C::QueueLaunchShot(target, time)).unwrap();
    }
    assert_eq!(windows(&s), [reload, 5 * reload]);
    let id = s.mission().unwrap().queued[0].id;
    s.dispatch(C::CancelQueuedShot(id)).unwrap();
    assert_eq!(windows(&s), [0, 5 * reload]);
    for time in [30.0, 60.0, 90.0] {
        let target = s.snapshot().aim_at(time).unwrap().target;
        s.dispatch(C::QueueLaunchShot(target, time)).unwrap();
    }
    assert!(windows(&s).is_empty());
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(windows(&s), [5 * reload]);
}

#[test]
fn firing_window_markers_respect_reload_deadline_and_locked_intervals() {
    let mut s = Session::blockade();
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    s.dispatch(C::StepForward).unwrap();
    let mission = s.mission().unwrap();
    assert_eq!(
        mission.available_launch_ticks(0),
        [mission.config.reload_ticks]
    );
    assert_eq!(
        mission.available_launch_ticks(s.snapshot().order_start_tick),
        [phase_vector::timing::INTERVAL_TICKS]
    );
    assert!(
        mission
            .available_launch_ticks(mission.config.deadline_tick)
            .is_empty()
    );
    let mut ended = mission.clone();
    ended.outcome = phase_vector::mission::Outcome::Timeout;
    assert!(ended.available_launch_ticks(0).is_empty());
}
