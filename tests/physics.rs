use phase_vector::physics::{Burn, STEP_SECONDS, Ship, Vector2 as V, World, swept_contact};

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}
fn ship(id: u32, position: V, velocity: V) -> Ship {
    Ship::new(id, position, velocity, 1.0, 2.0, 10.0).unwrap()
}

#[test]
fn inertia_without_gravity_or_fuel_consumption() {
    let mut world = World::new(vec![ship(1, V::new(3.0, -2.0), V::new(4.0, -8.0))]).unwrap();
    for _ in 0..480 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.ships[0].position, V::new(483.0, -962.0));
    assert_eq!(world.ships[0].velocity, V::new(4.0, -8.0));
    assert_eq!(world.ships[0].remaining_dv, 10.0);
}

#[test]
fn acceleration_is_limited_by_vector_magnitude() {
    let mut world = World::new(vec![ship(1, V::ZERO, V::ZERO)]).unwrap();
    world
        .step(&[Burn {
            ship_id: 1,
            acceleration: V::new(30.0, 40.0),
        }])
        .unwrap();
    near(world.ships[0].velocity.length(), 2.0 * STEP_SECONDS);
    near(world.ships[0].velocity.x, 0.3);
    near(world.ships[0].velocity.y, 0.4);
    near(world.ships[0].remaining_dv, 9.5);
    near(world.ships[0].position.x, 0.075);
}

#[test]
fn partial_last_burn_consumes_only_remaining_delta_v_then_coasts() {
    let s = Ship::new(1, V::ZERO, V::new(3.0, 0.0), 1.0, 2.0, 0.1).unwrap();
    let mut world = World::new(vec![s]).unwrap();
    let burn = [Burn {
        ship_id: 1,
        acceleration: V::new(2.0, 0.0),
    }];
    for _ in 0..100 {
        world.step(&burn).unwrap();
    }
    near(world.ships[0].velocity.x, 3.1);
    near(world.ships[0].position.x, 77.5);
    assert_eq!(world.ships[0].remaining_dv, 0.0);
    near(world.ships[0].spent_dv, 0.1);
}

#[test]
fn opposing_burn_spends_delta_v_even_when_net_velocity_returns_to_zero() {
    let mut world = World::new(vec![ship(1, V::ZERO, V::ZERO)]).unwrap();
    for x in [2.0, -2.0] {
        world
            .step(&[Burn {
                ship_id: 1,
                acceleration: V::new(x, 0.0),
            }])
            .unwrap();
    }
    assert_eq!(world.ships[0].velocity, V::ZERO);
    assert_eq!(world.ships[0].spent_dv, 1.0);
}

#[test]
fn fast_projectile_hits_between_tick_endpoints() {
    let mut world = World::new(vec![ship(2, V::ZERO, V::ZERO)]).unwrap();
    world
        .launch(1, V::new(-100.0, 0.0), V::new(800.0, 0.0), 0.0)
        .unwrap();
    world.step(&[]).unwrap();
    assert!(world.projectiles.is_empty());
    assert_eq!(world.impacts.len(), 1);
    near(world.impacts[0].fraction, 99.0 / 200.0);
    near(world.impacts[0].position.x, -1.0);
}

#[test]
fn moving_target_uses_relative_motion() {
    let mut world = World::new(vec![ship(2, V::new(0.0, -100.0), V::new(0.0, 800.0))]).unwrap();
    world
        .launch(1, V::new(-100.0, 0.0), V::new(800.0, 0.0), 0.0)
        .unwrap();
    world.step(&[]).unwrap();
    assert_eq!(world.impacts.len(), 1);
    assert!(world.impacts[0].fraction < 0.5);
}

#[test]
fn crossing_the_same_place_at_different_times_is_not_a_hit() {
    assert_eq!(
        swept_contact(
            V::new(-10.0, 0.0),
            V::new(10.0, 0.0),
            V::new(0.0, -20.0),
            V::new(0.0, 0.0),
            0.1
        ),
        None
    );
}

#[test]
fn tangent_endpoint_overlap_and_zero_relative_motion() {
    near(
        swept_contact(V::new(-2.0, 1.0), V::new(2.0, 1.0), V::ZERO, V::ZERO, 1.0).unwrap(),
        0.5,
    );
    assert_eq!(
        swept_contact(V::new(-2.0, 0.0), V::new(-1.0, 0.0), V::ZERO, V::ZERO, 1.0),
        Some(1.0)
    );
    assert_eq!(
        swept_contact(V::ZERO, V::ZERO, V::ZERO, V::ZERO, 1.0),
        Some(0.0)
    );
    assert_eq!(
        swept_contact(
            V::new(2.0, 0.0),
            V::new(3.0, 0.0),
            V::ZERO,
            V::new(1.0, 0.0),
            1.0
        ),
        None
    );
}

#[test]
fn first_contact_and_ties_have_stable_id_order() {
    let targets = vec![
        ship(9, V::ZERO, V::ZERO),
        ship(2, V::ZERO, V::ZERO),
        ship(3, V::new(20.0, 0.0), V::ZERO),
    ];
    let mut world = World::new(targets).unwrap();
    for _ in 0..2 {
        world
            .launch(1, V::new(-10.0, 0.0), V::new(200.0, 0.0), 0.0)
            .unwrap();
    }
    world.step(&[]).unwrap();
    assert_eq!(
        world
            .impacts
            .iter()
            .map(|hit| (hit.projectile_id, hit.ship_id))
            .collect::<Vec<_>>(),
        vec![(0, 2), (1, 2)]
    );
}

#[test]
fn projectile_does_not_collide_with_its_owner_and_is_not_culled_at_a_turn_boundary() {
    let mut world = World::new(vec![ship(1, V::ZERO, V::ZERO)]).unwrap();
    world.launch(1, V::ZERO, V::new(100.0, 0.0), 0.1).unwrap();
    for _ in 0..960 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.projectiles.len(), 1);
    assert_eq!(world.projectiles[0].position.x, 24000.0);
}

#[test]
fn invalid_orders_are_atomic() {
    let mut world = World::new(vec![ship(1, V::ZERO, V::ZERO)]).unwrap();
    let original = world.clone();
    for burns in [
        vec![Burn {
            ship_id: 99,
            acceleration: V::ZERO,
        }],
        vec![Burn {
            ship_id: 1,
            acceleration: V::new(f64::NAN, 0.0),
        }],
        vec![
            Burn {
                ship_id: 1,
                acceleration: V::ZERO
            };
            2
        ],
    ] {
        assert!(world.step(&burns).is_err());
        assert_eq!(world, original);
    }
    assert!(
        world
            .launch(1, V::ZERO, V::new(f64::INFINITY, 0.0), 0.0)
            .is_err()
    );
    assert_eq!(world, original);
    assert!(World::new(vec![ship(1, V::ZERO, V::ZERO); 2]).is_err());
}

#[test]
fn identical_initial_conditions_and_orders_reproduce_the_entire_world() {
    let mut a = World::new(vec![
        ship(1, V::ZERO, V::ZERO),
        ship(2, V::new(50.0, 0.0), V::ZERO),
    ])
    .unwrap();
    a.launch(1, V::ZERO, V::new(400.0, 0.0), 0.0).unwrap();
    let mut b = a.clone();
    for tick in 0..1000 {
        let burns = [Burn {
            ship_id: 1,
            acceleration: V::new(if tick < 500 { 0.01 } else { -0.01 }, 0.02),
        }];
        a.step(&burns).unwrap();
        b.step(&burns).unwrap();
        assert_eq!(a, b);
    }
}
