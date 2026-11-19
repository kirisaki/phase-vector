use phase_vector::{
    map::MapView,
    physics::{STEP_SECONDS, Ship, Vector2 as V, World},
    scenario::*,
    session::{Command, INTERVAL_TICKS, Session},
};

#[test]
fn light_delay_and_flight_time_use_the_same_distance() {
    assert!((INITIAL_SEPARATION / LIGHT_SPEED - 324.0).abs() < 1e-12);
    assert!((INITIAL_SEPARATION / RAILGUN_SPEED - 360.0).abs() < 1e-12);
    for flight in [240.0, 360.0, 480.0] {
        let light_delay = RAILGUN_SPEED * flight / LIGHT_SPEED;
        assert!((flight - light_delay - flight * 0.1).abs() < 1e-10);
    }
}

#[test]
fn full_burn_spends_120_km_per_second_in_one_interval() {
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
    assert_eq!(ship.remaining_dv, 1_080_000.0);
    assert_eq!(ship.spent_dv, 120_000.0);
    assert_eq!(ship.velocity, V::new(120_000.0, FRIENDLY_SPEED));
    // The discrete kick/drift model travels 7,215 km laterally in 120 s.
    assert!((ship.position.x + INITIAL_SEPARATION / 2.0 - 7_215_000.0).abs() < 0.001);
    assert!((ship.position.y - 3_597_509_496.0).abs() < 0.001);
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
        .launch(1, V::ZERO, V::new(RAILGUN_SPEED, 0.0), 1.0)
        .unwrap();
    for _ in 0..(REFERENCE_FLIGHT_SECONDS / STEP_SECONDS) as u64 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.impacts.len(), 1);
    let impact = &world.impacts[0];
    let expected = (INITIAL_SEPARATION - SHIP_RADIUS - 1.0) / RAILGUN_SPEED;
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
