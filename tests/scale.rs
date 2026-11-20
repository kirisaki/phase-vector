use phase_vector::{
    map::MapView,
    physics::{STEP_SECONDS, Ship, Vector2 as V, World},
    scenario::*,
    session::{Command, INTERVAL_TICKS, Session},
};

#[test]
fn light_delay_and_flight_time_use_the_same_distance() {
    // Independent regression anchors for the selected trial, not production formulas.
    assert_eq!(INITIAL_SEPARATION, 240_000_000.0);
    assert_eq!(RAILGUN_SPEED, 660_000.0);
    assert!((INITIAL_SEPARATION / LIGHT_SPEED - 0.800553828475565).abs() < 1e-12);
    assert!((REFERENCE_FLIGHT_SECONDS - 363.6363636363636).abs() < 1e-10);
    assert!((REFERENCE_FLIGHT_SECONDS * RAILGUN_SPEED - INITIAL_SEPARATION).abs() < 1e-6);
}

#[test]
fn five_g_burn_matches_the_trial_and_exhausts_fuel_in_the_eleventh_interval() {
    let mut session = Session::default();
    session
        .dispatch(Command::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
        .unwrap();
    session.dispatch(Command::Commit).unwrap();
    session.dispatch(Command::Pause).unwrap();
    for _ in 0..INTERVAL_TICKS {
        session.dispatch(Command::Step).unwrap();
    }
    let ship = &session.world().ships[0];
    assert!((MAX_ACCELERATION - 49.03325).abs() < 1e-12);
    assert!((ship.remaining_dv - 54_116.01).abs() < 1e-6);
    assert!((ship.spent_dv - 5_883.99).abs() < 1e-6);
    assert!((ship.velocity.x - 5_883.99).abs() < 1e-6);
    assert_eq!(ship.velocity.y, 75_000.0);
    // The discrete kick/drift model travels 353.77489875 km laterally in 120 s.
    assert!((ship.position.x + INITIAL_SEPARATION / 2.0 - 353_774.898_75).abs() < 0.001);
    assert!((ship.position.y - 9_000_000.0).abs() < 0.001);
    let deflection = ship.velocity.x.atan2(ship.velocity.y).to_degrees();
    assert!((4.48..4.49).contains(&deflection));
    for _ in 1..10 {
        session
            .dispatch(Command::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
            .unwrap();
        session.dispatch(Command::NextTurn).unwrap();
    }
    assert!((session.world().ships[0].remaining_dv - 1_160.1).abs() < 1e-6);
    session
        .dispatch(Command::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
        .unwrap();
    session.dispatch(Command::NextTurn).unwrap();
    let exhausted = session.world().ships[0].clone();
    assert_eq!(exhausted.remaining_dv, 0.0);
    assert!((exhausted.spent_dv - INITIAL_DELTA_V).abs() < 1e-6);
    session
        .dispatch(Command::PlanBurn(V::new(MAX_ACCELERATION, 0.0)))
        .unwrap();
    session.dispatch(Command::NextTurn).unwrap();
    assert_eq!(session.world().ships[0].velocity, exhausted.velocity);
    assert_eq!(session.world().ships[0].spent_dv, exhausted.spent_dv);
}

#[test]
fn railgun_hits_a_small_target_after_three_command_intervals() {
    let target = Ship::new(
        2,
        V::new(INITIAL_SEPARATION, 0.0),
        V::ZERO,
        SHIP_RADIUS,
        0.0,
        0.0,
    )
    .unwrap();
    let mut world = World::new(vec![target]).unwrap();
    world
        .launch(1, V::ZERO, V::new(RAILGUN_SPEED, 0.0), PROJECTILE_RADIUS)
        .unwrap();
    for _ in 0..3 * INTERVAL_TICKS {
        world.step(&[]).unwrap();
    }
    assert!(world.impacts.is_empty());
    assert_eq!(world.projectiles.len(), 1);
    while world.tick < (REFERENCE_FLIGHT_SECONDS / STEP_SECONDS).ceil() as u64 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.impacts.len(), 1);
    let impact = &world.impacts[0];
    let expected = (INITIAL_SEPARATION - SHIP_RADIUS - PROJECTILE_RADIUS) / RAILGUN_SPEED;
    assert!(((impact.tick as f64 + impact.fraction) * STEP_SECONDS - expected).abs() < 1e-7);
}

#[test]
fn camera_preserves_local_precision_and_does_not_change_the_world() {
    let world = initial_world();
    let original = world.clone();
    let mut map = MapView::default();
    let overview = map.frame(&world, 1);
    assert!((overview.project(world.ships[0].position).x + 650.0).abs() < 1e-10);
    map.focus("friendly").unwrap();
    for _ in 0..30 {
        map.zoom(0.5).unwrap();
    }
    let frame = map.frame(&world, 1);
    assert!(!frame.can_zoom_in);
    assert_eq!(
        frame.project(world.ships[0].position + V::new(0.5, 0.0)).x,
        0.5
    );
    for _ in 0..60 {
        map.zoom(2.0).unwrap();
    }
    assert!(!map.frame(&world, 1).can_zoom_out);
    assert!(map.zoom(f64::NAN).is_err());
    assert!(map.focus("invalid").is_err());
    assert_eq!(world, original);
}
