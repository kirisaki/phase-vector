//! Browser bindings and the Bevy renderer for the step-1 integration probe.

use std::cell::RefCell;

use bevy::{camera::ScalingMode, prelude::*, window::WindowResolution};
use wasm_bindgen::prelude::*;

// Enable the browser entropy backends required by the Lua VM's dependencies.
use getrandom as _;
use getrandom03 as _;

use crate::repl::Host;

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
    static FRAMES: RefCell<u32> = const { RefCell::new(0) };
}

#[wasm_bindgen]
pub fn run_lua(source: &str) -> Result<String, JsValue> {
    HOST.with_borrow_mut(|host| {
        host.evaluate(source)
            .map_err(|error| JsValue::from_str(&error))
    })
}

#[wasm_bindgen]
pub fn command(name: &str) -> Result<(), JsValue> {
    HOST.with_borrow_mut(|host| {
        host.command(name)
            .map_err(|error| JsValue::from_str(&error))
    })
}

#[wasm_bindgen]
pub fn state_json() -> String {
    HOST.with_borrow(|host| {
        serde_json::to_string(&*host.state.borrow()).expect("serializable probe state")
    })
}

#[wasm_bindgen]
pub fn rendered_frames() -> u32 {
    FRAMES.with_borrow(|frames| *frames)
}

#[wasm_bindgen]
pub fn start() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.045, 0.065)))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "PHASE VECTOR".into(),
                canvas: Some("#tactical-map".into()),
                resolution: WindowResolution::new(960, 600),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn((
                Camera2d,
                Projection::Orthographic(OrthographicProjection {
                    scaling_mode: ScalingMode::AutoMin {
                        min_width: 760.0,
                        min_height: 480.0,
                    },
                    ..OrthographicProjection::default_2d()
                }),
            ));
        })
        .add_systems(FixedUpdate, || {
            HOST.with_borrow_mut(|host| host.state.borrow_mut().advance())
        })
        .add_systems(Update, draw_probe)
        .run();
}

fn draw_probe(mut gizmos: Gizmos) {
    let ticks = HOST.with_borrow(|host| host.state.borrow().ticks);
    let grid = Color::srgb(0.08, 0.15, 0.19);
    let cyan = Color::srgb(0.3, 0.88, 0.85);
    let amber = Color::srgb(1.0, 0.66, 0.34);
    for x in -12..=12 {
        gizmos.line_2d(
            Vec2::new(x as f32 * 40.0, -400.0),
            Vec2::new(x as f32 * 40.0, 400.0),
            grid,
        );
    }
    for y in -10..=10 {
        gizmos.line_2d(
            Vec2::new(-600.0, y as f32 * 40.0),
            Vec2::new(600.0, y as f32 * 40.0),
            grid,
        );
    }
    gizmos
        .circle_2d(Isometry2d::IDENTITY, 160.0, grid)
        .resolution(96);
    gizmos
        .circle_2d(Isometry2d::IDENTITY, 80.0, grid)
        .resolution(64);
    gizmos.rect_2d(
        Isometry2d::from_translation(Vec2::new(260.0, 0.0)),
        Vec2::new(70.0, 240.0),
        amber,
    );
    let ship = Vec2::new(-200.0, -40.0);
    gizmos.linestrip_2d(
        [
            ship + Vec2::new(18.0, 0.0),
            ship + Vec2::new(-12.0, 10.0),
            ship + Vec2::new(-6.0, 0.0),
            ship + Vec2::new(-12.0, -10.0),
            ship + Vec2::new(18.0, 0.0),
        ],
        cyan,
    );
    gizmos.arrow_2d(
        ship + Vec2::new(26.0, 0.0),
        ship + Vec2::new(95.0, 0.0),
        cyan,
    );
    let marker = Vec2::new(40.0 + (ticks as f32 * 0.025).sin() * 90.0, 65.0);
    gizmos.circle_2d(Isometry2d::from_translation(marker), 9.0, amber);
    gizmos.line_2d(
        marker + Vec2::new(-22.0, 0.0),
        marker + Vec2::new(22.0, 0.0),
        amber,
    );
    gizmos.line_2d(
        marker + Vec2::new(0.0, -22.0),
        marker + Vec2::new(0.0, 22.0),
        amber,
    );
    FRAMES.with_borrow_mut(|frames| *frames = frames.saturating_add(1));
}
