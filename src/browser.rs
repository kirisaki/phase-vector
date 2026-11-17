//! Browser bindings and presentation of the renderer-independent WEGO session.

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
pub fn plan_burn(x: f64, y: f64) -> Result<(), JsValue> {
    HOST.with_borrow_mut(|host| {
        host.plan_burn(x, y)
            .map_err(|error| JsValue::from_str(&error))
    })
}

#[wasm_bindgen]
pub fn set_speed(speed: f64) -> Result<(), JsValue> {
    HOST.with_borrow_mut(|host| {
        host.set_speed(speed)
            .map_err(|error| JsValue::from_str(&error))
    })
}

#[wasm_bindgen]
pub fn state_json() -> String {
    HOST.with_borrow(|host| {
        serde_json::to_string(&host.state.borrow().snapshot())
            .expect("serializable session snapshot")
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
                        min_width: 2000.0,
                        min_height: 1200.0,
                    },
                    ..OrthographicProjection::default_2d()
                }),
            ));
        })
        .add_systems(FixedUpdate, || {
            HOST.with_borrow_mut(|host| host.state.borrow_mut().pulse())
        })
        .add_systems(Update, draw_world)
        .run();
}

fn draw_world(mut gizmos: Gizmos) {
    let grid = Color::srgb(0.08, 0.15, 0.19);
    let cyan = Color::srgb(0.3, 0.88, 0.85);
    let amber = Color::srgb(1.0, 0.66, 0.34);
    for x in -12..=12 {
        gizmos.line_2d(
            Vec2::new(x as f32 * 100.0, -1000.0),
            Vec2::new(x as f32 * 100.0, 1000.0),
            grid,
        );
    }
    for y in -10..=10 {
        gizmos.line_2d(
            Vec2::new(-1200.0, y as f32 * 100.0),
            Vec2::new(1200.0, y as f32 * 100.0),
            grid,
        );
    }
    HOST.with_borrow(|host| {
        let session = host.state.borrow();
        let view = session.snapshot();
        for ship in &view.world.ships {
            let position = Vec2::new(ship.position.x as f32, ship.position.y as f32);
            let velocity = Vec2::new(ship.velocity.x as f32, ship.velocity.y as f32);
            let friendly = ship.id == view.player_id;
            let color = if friendly { cyan } else { amber };
            gizmos.circle_2d(
                Isometry2d::from_translation(position),
                ship.radius as f32,
                color,
            );
            if friendly {
                gizmos.linestrip_2d(
                    [
                        position + Vec2::new(28.0, 0.0),
                        position + Vec2::new(-22.0, 18.0),
                        position + Vec2::new(-12.0, 0.0),
                        position + Vec2::new(-22.0, -18.0),
                        position + Vec2::new(28.0, 0.0),
                    ],
                    color,
                );
            } else {
                gizmos.cross_2d(position, 30.0, color);
            }
            if velocity.length_squared() > 0.0 {
                gizmos.arrow_2d(position, position + velocity * 50.0, color);
            }
        }
        for projectile in &view.world.projectiles {
            let position = Vec2::new(projectile.position.x as f32, projectile.position.y as f32);
            gizmos.circle_2d(
                Isometry2d::from_translation(position),
                (projectile.radius as f32).max(3.0),
                Color::WHITE,
            );
        }
    });
    FRAMES.with_borrow_mut(|frames| *frames = frames.saturating_add(1));
}
