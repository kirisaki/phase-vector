use phase_vector::timing::{FORECAST_INTERVALS, INTERVAL_SECONDS, INTERVAL_TICKS, STEP_SECONDS};
use phase_vector::{
    maneuver::{preview, solve},
    physics::Vector2 as V,
    repl::Host,
    scenario::MAX_ACCELERATION,
    session::{Command as C, Session},
};

fn verify_target(mut session: Session, offset: V) {
    let ship = session.world().ships[0].clone();
    let target = preview(&ship, V::ZERO).coast_endpoint + offset;
    let original = session.world().clone();
    let acceleration = solve(&ship, target).unwrap();
    let predicted = preview(&ship, acceleration);
    assert!((predicted.endpoint - target).length() < 0.01);
    assert_eq!(session.world(), &original);
    session.dispatch(C::PlanArrival(target)).unwrap();
    assert_eq!(session.world(), &original);
    session.dispatch(C::Commit).unwrap();
    session.dispatch(C::NextTurn).unwrap();
    let actual = &session.world().ships[0];
    assert_eq!(actual.position, predicted.endpoint);
    assert_eq!(actual.velocity, predicted.arrival_velocity);
    assert_eq!(actual.remaining_dv, predicted.remaining_dv);
    assert!((actual.spent_dv - ship.spent_dv - predicted.delta_v).abs() < 1e-6);
}

#[test]
fn endpoint_plans_match_execution_at_astronomical_coordinates() {
    let mut world = Session::default().world().clone();
    // Preserve the large-coordinate regression independently of scenario tuning.
    world.ships[0].position = V::new(-50_000_000_000.0, 30_000_000_000.0);
    let session = Session::new(world, 1).unwrap();
    let reach = preview(&session.world().ships[0], V::ZERO).reachable_radius;
    for offset in [
        V::ZERO,
        V::new(reach * 0.2, -reach * 0.4),
        V::new(reach, 0.0),
        V::new(-reach * 0.6, reach * 0.8),
    ] {
        verify_target(session.clone(), offset);
    }
}

#[test]
fn limited_fuel_uses_the_same_partial_burn_and_coasting_as_execution() {
    let mut world = Session::default().world().clone();
    world.ships[0].remaining_dv = 123.45;
    let ship = &world.ships[0];
    let max = preview(ship, V::new(MAX_ACCELERATION, 0.0));
    assert_eq!(max.remaining_dv, 0.0);
    // Exhaustion spans several ticks, including a final partial burn.
    let acceleration_per_tick = MAX_ACCELERATION * STEP_SECONDS;
    let full_ticks = (123.45 / acceleration_per_tick).floor() as u64;
    let remainder = 123.45 - full_ticks as f64 * acceleration_per_tick;
    let expected = (0..full_ticks)
        .map(|tick| acceleration_per_tick * (INTERVAL_SECONDS - tick as f64 * STEP_SECONDS))
        .sum::<f64>()
        + remainder * (INTERVAL_SECONDS - full_ticks as f64 * STEP_SECONDS);
    assert!((max.reachable_radius - expected).abs() < 1e-6);
    let reach = max.reachable_radius;
    let session = Session::new(world, 1).unwrap();
    verify_target(session.clone(), V::new(reach, 0.0));
    verify_target(session, V::new(-reach * 0.4, reach * 0.3));
}

#[test]
fn no_fuel_or_no_thrust_only_allows_the_coasting_endpoint() {
    for no_fuel in [true, false] {
        let mut ship = Session::default().world().ships[0].clone();
        if no_fuel {
            ship.remaining_dv = 0.0;
        } else {
            ship.max_acceleration = 0.0;
        }
        let context = preview(&ship, V::ZERO);
        assert_eq!(context.reachable_radius, 0.0);
        assert_eq!(solve(&ship, context.coast_endpoint).unwrap(), V::ZERO);
        assert!(solve(&ship, context.coast_endpoint + V::new(1.0, 0.0)).is_err());
    }
}

#[test]
fn invalid_arrival_orders_do_not_modify_the_plan() {
    let mut session = Session::default();
    session.dispatch(C::PlanBurn(V::new(10.0, 20.0))).unwrap();
    let original = session.world().clone();
    let route = preview(&original.ships[0], V::ZERO);
    for target in [
        V::new(f64::NAN, 0.0),
        V::new(f64::INFINITY, 0.0),
        V::new(1e30, 0.0),
        route.coast_endpoint + V::new(route.reachable_radius + 1.0, 0.0),
    ] {
        assert!(session.dispatch(C::PlanArrival(target)).is_err());
        assert_eq!(session.snapshot().planned_burn, V::new(10.0, 20.0));
        assert_eq!(session.world(), &original);
    }
    session.dispatch(C::Commit).unwrap();
    for paused in [false, true] {
        if paused {
            session.dispatch(C::Pause).unwrap();
        }
        assert!(
            session
                .dispatch(C::PlanArrival(route.coast_endpoint))
                .is_err()
        );
    }
}

#[test]
fn lua_and_graphical_arrival_commands_share_validation_and_results() {
    let mut ui = Host::default();
    let mut lua = Host::default();
    let coast = preview(&ui.state.borrow().world().ships[0], V::ZERO);
    let target =
        coast.coast_endpoint + V::new(coast.reachable_radius * 0.4, coast.reachable_radius * 0.2);
    ui.plan_arrival(target.x, target.y).unwrap();
    lua.evaluate(&format!("game.arrive({}, {})", target.x, target.y))
        .unwrap();
    assert_eq!(
        ui.state.borrow().snapshot().planned_burn,
        lua.state.borrow().snapshot().planned_burn
    );
    assert!(lua.evaluate("game.arrive(0/0, 0)").is_err());
    lua.evaluate("game.command('commit'); game.command('pause')")
        .unwrap();
    assert!(
        lua.evaluate(&format!("game.arrive({}, {})", target.x, target.y))
            .is_err()
    );
}

#[test]
fn executing_preview_keeps_the_committed_endpoint_and_remaining_cost() {
    for remaining_dv in [1_200_000.0, 123.45] {
        let mut world = Session::default().world().clone();
        world.ships[0].remaining_dv = remaining_dv;
        let mut session = Session::new(world, 1).unwrap();
        session
            .dispatch(C::PlanBurn(V::new(600.0, -800.0)))
            .unwrap();
        let original = preview(&session.world().ships[0], session.snapshot().planned_burn);
        session.dispatch(C::Commit).unwrap();
        session.dispatch(C::Pause).unwrap();
        for _ in 0..37 {
            session.dispatch(C::Step).unwrap();
        }
        let view = session.snapshot();
        let remaining = phase_vector::maneuver::preview_ticks(
            &view.world.ships[0],
            view.active_burn,
            INTERVAL_TICKS - 37,
        );
        assert_eq!(remaining.endpoint, original.endpoint);
        assert_eq!(remaining.points.last(), Some(&original.endpoint));
        assert_eq!(remaining.arrival_velocity, original.arrival_velocity);
        assert_eq!(remaining.remaining_dv, original.remaining_dv);
        assert!((view.world.ships[0].spent_dv + remaining.delta_v - original.delta_v).abs() < 1e-6);
        session.dispatch(C::NextTurn).unwrap();
        assert_eq!(session.world().ships[0].position, remaining.endpoint);
    }
}

#[test]
fn a_running_arrival_edit_uses_the_next_boundary_and_keeps_the_current_burn() {
    let mut s = Session::default();
    s.dispatch(C::PlanBurn(V::new(10.0, 0.0))).unwrap();
    s.dispatch(C::Resume).unwrap();
    for _ in 0..30 {
        s.pulse();
    }
    let next_ship = s.snapshot().planning_ship();
    let expected = preview(&next_ship, V::new(0.0, 20.0));
    let original = s.world().clone();
    s.dispatch(C::PlanArrival(expected.endpoint)).unwrap();
    let queued = s.snapshot().planned_route(s.snapshot().planned_burn);
    assert_eq!(s.world(), &original);
    assert_eq!(s.snapshot().active_burn, V::new(10.0, 0.0));
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.world().ships[0], next_ship);
    s.dispatch(C::NextTurn).unwrap();
    let actual = &s.world().ships[0];
    assert_eq!(actual.position, queued.endpoint);
    assert_eq!(actual.velocity, queued.arrival_velocity);
    assert_eq!(actual.remaining_dv, queued.remaining_dv);
}

#[test]
fn queued_arrival_uses_the_fuel_left_after_the_active_interval() {
    let mut world = Session::default().world().clone();
    world.ships[0].remaining_dv = 10.0;
    let mut s = Session::new(world, 1).unwrap();
    s.dispatch(C::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
        .unwrap();
    s.dispatch(C::Resume).unwrap();
    let next_ship = s.snapshot().planning_ship();
    assert_eq!(next_ship.remaining_dv, 0.0);
    let route = preview(&next_ship, V::ZERO);
    assert!(
        s.dispatch(C::PlanArrival(route.endpoint + V::new(1.0, 0.0)))
            .is_err()
    );
    s.dispatch(C::PlanArrival(route.endpoint)).unwrap();
    assert_eq!(s.snapshot().planned_burn, V::ZERO);
    s.dispatch(C::NextTurn).unwrap();
    s.dispatch(C::NextTurn).unwrap();
    assert_eq!(s.world().ships[0].position, route.endpoint);
}

#[test]
fn three_turn_forecast_matches_execution_and_fuel_exhaustion() {
    for remaining_dv in [0.0, 6_000.0, phase_vector::scenario::INITIAL_DELTA_V] {
        for elapsed in [0, 37] {
            let mut world = Session::default().world().clone();
            world.ships[0].remaining_dv = remaining_dv;
            let mut session = Session::new(world, 1).unwrap();
            session.dispatch(C::PlanBurn(V::new(10.0, 0.0))).unwrap();
            if elapsed > 0 {
                for _ in 0..elapsed {
                    session.dispatch(C::Step).unwrap();
                }
                session.dispatch(C::Resume).unwrap();
            }
            let queued = V::new(0.0, -MAX_ACCELERATION);
            session.dispatch(C::PlanBurn(queued)).unwrap();
            let original = session.world().clone();
            let log = session.snapshot().command_log.to_vec();
            let preview = session.snapshot().planned_route(queued);
            let forecast = preview.forecast.unwrap();
            assert_eq!(session.world(), &original);
            assert_eq!(session.snapshot().command_log, log);
            assert_eq!(forecast.boundaries.len(), FORECAST_INTERVALS);
            assert_eq!(forecast.points.first(), Some(&original.ships[0].position));
            for (index, boundary) in forecast.boundaries.iter().enumerate() {
                session.dispatch(C::NextTurn).unwrap();
                let actual = &session.world().ships[0];
                assert_eq!(
                    boundary.seconds_ahead,
                    (index + 1) as f64 * INTERVAL_SECONDS - elapsed as f64 * STEP_SECONDS
                );
                assert_eq!(boundary.position, actual.position);
                assert_eq!(boundary.velocity, actual.velocity);
                assert_eq!(boundary.remaining_dv, actual.remaining_dv);
            }
            let editable = usize::from(elapsed > 0);
            assert_eq!(preview.endpoint, forecast.boundaries[editable].position);
            assert_eq!(
                preview.remaining_dv,
                forecast.boundaries[editable].remaining_dv
            );
            assert_eq!(
                forecast.points.last(),
                Some(&session.world().ships[0].position)
            );
            if remaining_dv <= 6_000.0 {
                assert_eq!(session.world().ships[0].remaining_dv, 0.0);
            }
        }
    }
}
