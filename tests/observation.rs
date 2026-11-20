use phase_vector::{
    observation::{SAMPLE_TICKS, Sensors, TRACK_RETENTION_TICKS},
    physics::{STEP_SECONDS, Vector2 as V},
    scenario,
    timing::seconds_to_ticks,
    units::LIGHT_SPEED,
};

#[test]
fn contact_delivery_includes_processing_and_quantized_light_travel() {
    let mut world = scenario::initial_world();
    let mut sensors = Sensors::new(&world.ships[1], world.tick);
    let briefing = sensors.contact.clone();
    let processing = seconds_to_ticks(1);
    let delay =
        (scenario::INITIAL_SEPARATION / LIGHT_SPEED / STEP_SECONDS).ceil() as u64 + processing;
    world.ships[1].velocity.x = 1000.0;
    let sampled = world.ships[1].clone();
    sensors.update(&world, 1, 2, processing);
    world.ships[1].velocity.x = -2000.0;
    for tick in 1..delay {
        world.tick = tick;
        sensors.update(&world, 1, 2, processing);
        assert_eq!(sensors.contact, briefing);
    }
    world.tick = delay;
    sensors.update(&world, 1, 2, processing);
    assert_eq!(sensors.contact.sampled_tick, 0);
    assert_eq!(sensors.contact.received_tick, delay);
    assert_eq!(sensors.contact.velocity, sampled.velocity);
    assert_eq!(
        sensors.contact.position_at(delay),
        sampled.position + sampled.velocity * (delay as f64 * STEP_SECONDS)
    );
}

#[test]
fn late_older_contact_does_not_overwrite_a_newer_delivered_fix() {
    let mut world = scenario::initial_world();
    // Move between samples to isolate packet ordering, without integrating physics.
    world.ships[1].position = world.ships[0].position + V::new(5.0 * LIGHT_SPEED, 0.0);
    let mut sensors = Sensors::new(&world.ships[1], 0);
    let processing = seconds_to_ticks(1);
    sensors.update(&world, 1, 2, processing);
    world.tick = SAMPLE_TICKS;
    world.ships[1].position = world.ships[0].position + V::new(LIGHT_SPEED * STEP_SECONDS, 0.0);
    world.ships[1].velocity.x = 1000.0;
    sensors.update(&world, 1, 2, processing);
    world.tick += processing + 1;
    sensors.update(&world, 1, 2, processing);
    let newer = sensors.contact.clone();
    assert_eq!(newer.sampled_tick, SAMPLE_TICKS);
    assert_eq!(newer.velocity.x, 1000.0);
    world.tick = seconds_to_ticks(5) + processing;
    sensors.update(&world, 1, 2, processing);
    assert_eq!(sensors.contact, newer);
}

#[test]
fn projectile_knowledge_survives_hidden_removal_and_expires_by_sample_age() {
    let mut world = scenario::initial_world();
    let enemy = world.ships[1].clone();
    let mut sensors = Sensors::new(&enemy, 0);
    let processing = seconds_to_ticks(1);
    for distance in [5.0 * LIGHT_SPEED, LIGHT_SPEED * STEP_SECONDS] {
        world
            .launch(
                1,
                enemy.position + V::new(distance, 0.0),
                V::ZERO,
                scenario::PROJECTILE_RADIUS,
            )
            .unwrap();
    }
    sensors.update(&world, 1, 2, processing);
    assert!(sensors.threats.is_empty());
    // A hidden disappearance cannot cancel an already sampled packet or track.
    world.projectiles.clear();
    world.tick = processing + 1;
    sensors.update(&world, 1, 2, processing);
    assert_eq!(
        sensors.threats.iter().map(|t| t.id).collect::<Vec<_>>(),
        [1]
    );
    world.tick = seconds_to_ticks(5) + processing;
    sensors.update(&world, 1, 2, processing);
    assert_eq!(
        sensors.threats.iter().map(|t| t.id).collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(sensors.threats.iter().all(|t| t.sampled_tick == 0));
    world.tick = TRACK_RETENTION_TICKS;
    sensors.update(&world, 1, 2, processing);
    assert_eq!(sensors.threats.len(), 2);
    world.tick += 1;
    sensors.update(&world, 1, 2, processing);
    assert!(sensors.threats.is_empty());
}
