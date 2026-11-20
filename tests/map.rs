use phase_vector::{map::MapView, physics::Vector2 as V, scenario::initial_world};

#[test]
fn auto_zoom_fits_both_ships_in_portrait_and_landscape_even_after_a_jump() {
    for aspect in [0.4, 1.0, 16.0 / 9.0, 3.0] {
        let mut world = initial_world();
        world.ships[0].position = V::new(-100_000_000.0, -80_000_000.0);
        world.ships[1].position = V::new(100_000_000.0, 80_000_000.0);
        let original = world.clone();
        let mut map = MapView::default();
        map.update_auto_zoom(&world, 1, aspect, 0.0);
        for jump in [false, true] {
            if jump {
                world.ships[1].position = V::new(800_000_000.0, 600_000_000.0);
                map.update_auto_zoom(&world, 1, aspect, 1.0 / 60.0);
            }
            let frame = map.frame(&world, 1);
            assert!(frame.auto_zoom);
            let margin = if jump { 0.90 } else { 0.65 };
            for ship in &world.ships {
                let p = frame.project(ship.position);
                assert!(p.x.abs() <= 1000_f64.max(600.0 * aspect) * margin + 1e-8);
                assert!(p.y.abs() <= 600_f64.max(1000.0 / aspect) * margin + 1e-8);
            }
            if !jump {
                assert_eq!(world, original);
            }
        }
    }
}

#[test]
fn auto_zoom_eases_in_both_directions_without_overshoot() {
    let mut world = initial_world();
    let mut map = MapView::default();
    map.update_auto_zoom(&world, 1, 5.0 / 3.0, 0.0);
    let initial = map.frame(&world, 1).metres_per_unit;
    for ship in &mut world.ships {
        ship.position = ship.position * 2.0;
    }
    map.update_auto_zoom(&world, 1, 5.0 / 3.0, 1.0 / 60.0);
    let outward = map.frame(&world, 1).metres_per_unit;
    assert!(outward > initial && outward < initial * 2.0);
    for _ in 0..360 {
        map.update_auto_zoom(&world, 1, 5.0 / 3.0, 1.0 / 60.0);
    }
    assert!((map.frame(&world, 1).metres_per_unit / initial - 2.0).abs() < 1e-6);
    for ship in &mut world.ships {
        ship.position = ship.position * 0.25;
    }
    map.update_auto_zoom(&world, 1, 5.0 / 3.0, 1.0 / 60.0);
    let inward = map.frame(&world, 1).metres_per_unit;
    assert!(inward > initial * 0.5 && inward < initial * 2.0);
    for _ in 0..360 {
        map.update_auto_zoom(&world, 1, 5.0 / 3.0, 1.0 / 60.0);
    }
    assert!((map.frame(&world, 1).metres_per_unit / initial - 0.5).abs() < 1e-6);
}

#[test]
fn manual_zoom_and_ship_focus_hold_scale_until_overview_is_reselected() {
    let mut world = initial_world();
    let mut map = MapView::default();
    map.update_auto_zoom(&world, 1, 1.0, 0.0);
    map.zoom(0.5).unwrap();
    let manual = map.frame(&world, 1).metres_per_unit;
    world.ships[1].position = V::new(500_000_000.0, 500_000_000.0);
    map.update_auto_zoom(&world, 1, 3.0, 5.0);
    assert!(!map.frame(&world, 1).auto_zoom);
    assert_eq!(map.frame(&world, 1).metres_per_unit, manual);
    map.focus("overview").unwrap();
    map.update_auto_zoom(&world, 1, 3.0, 0.0);
    assert!(map.frame(&world, 1).auto_zoom);
    assert!(map.frame(&world, 1).metres_per_unit > manual);
    for focus in ["friendly", "contact"] {
        map.focus(focus).unwrap();
        let local = map.frame(&world, 1).metres_per_unit;
        map.update_auto_zoom(&world, 1, 0.4, 5.0);
        assert!(!map.frame(&world, 1).auto_zoom);
        assert_eq!(map.frame(&world, 1).metres_per_unit, local);
    }
}

#[test]
fn smoothing_uses_elapsed_time_instead_of_frame_count() {
    let mut world = initial_world();
    let mut fast = MapView::default();
    fast.update_auto_zoom(&world, 1, 1.0, 0.0);
    let mut slow = fast;
    for ship in &mut world.ships {
        ship.position = ship.position * 0.5;
    }
    for _ in 0..60 {
        fast.update_auto_zoom(&world, 1, 1.0, 1.0 / 60.0);
    }
    for _ in 0..30 {
        slow.update_auto_zoom(&world, 1, 1.0, 1.0 / 30.0);
    }
    assert!(
        (fast.frame(&world, 1).metres_per_unit / slow.frame(&world, 1).metres_per_unit - 1.0).abs()
            < 1e-12
    );
}

#[test]
fn navigation_keeps_the_world_anchor_under_the_gesture_even_at_zoom_limits() {
    let world = initial_world();
    let anchor = V::new(120_000_000.0, -45_000_000.0);
    let offset = V::new(250.0, -120.0);
    for requested in [1e-30, 100_000.0, 1e30] {
        let mut map = MapView::default();
        map.navigate(anchor, offset, requested).unwrap();
        let frame = map.frame(&world, 1);
        assert!((frame.project(anchor) - offset).length() < 1e-7);
        assert!(!frame.auto_zoom);
        assert_eq!(serde_json::to_value(frame.focus).unwrap(), "free");
        assert_eq!(frame.metres_per_unit, requested.clamp(1.0, 200_000_000.0));
    }
}

#[test]
fn panning_freezes_the_center_until_a_view_preset_is_selected() {
    let mut world = initial_world();
    let mut map = MapView::default();
    map.update_auto_zoom(&world, 1, 1.0, 0.0);
    let before = map.frame(&world, 1);
    let drag = V::new(100.0, -50.0);
    map.navigate(before.center, drag, before.metres_per_unit)
        .unwrap();
    let free = map.frame(&world, 1);
    assert_eq!(free.center, before.center - drag * before.metres_per_unit);
    for ship in &mut world.ships {
        ship.position = ship.position + V::new(90_000_000.0, 50_000_000.0);
    }
    map.update_auto_zoom(&world, 1, 0.4, 10.0);
    assert_eq!(map.frame(&world, 1).center, free.center);
    assert_eq!(map.frame(&world, 1).metres_per_unit, free.metres_per_unit);
    map.zoom(0.5).unwrap();
    assert_eq!(map.frame(&world, 1).center, free.center);
    for (name, expected) in [
        ("friendly", world.ships[0].position),
        ("contact", world.ships[1].position),
    ] {
        map.focus(name).unwrap();
        assert_eq!(map.frame(&world, 1).center, expected);
    }
    map.focus("overview").unwrap();
    map.update_auto_zoom(&world, 1, 0.4, 0.0);
    assert!(map.frame(&world, 1).auto_zoom);
    assert_eq!(
        map.frame(&world, 1).center,
        (world.ships[0].position + world.ships[1].position) * 0.5
    );
}

#[test]
fn invalid_navigation_does_not_modify_camera_or_simulation() {
    let world = initial_world();
    let mut map = MapView::default();
    let before = serde_json::to_value(map.frame(&world, 1)).unwrap();
    for (anchor, offset, scale) in [
        (V::new(f64::NAN, 0.0), V::ZERO, 1.0),
        (V::ZERO, V::new(0.0, f64::INFINITY), 1.0),
        (V::ZERO, V::ZERO, -1.0),
        (V::ZERO, V::ZERO, 0.0),
        (V::ZERO, V::ZERO, f64::INFINITY),
        (V::ZERO, V::new(1e12, 0.0), 1e30),
    ] {
        assert!(map.navigate(anchor, offset, scale).is_err());
        assert_eq!(serde_json::to_value(map.frame(&world, 1)).unwrap(), before);
    }
    assert_eq!(world, initial_world());
}
