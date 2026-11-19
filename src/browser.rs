//! Browser bindings and presentation of the renderer-independent WEGO session.

use std::cell::RefCell;

use bevy::{camera::ScalingMode, prelude::*, window::WindowResolution};
use wasm_bindgen::prelude::*;

// Enable the browser entropy backends required by the Lua VM's dependencies.
use getrandom as _;
use getrandom03 as _;

use crate::{map::MapView, physics::Vector2, repl::Host};

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
    static MAP: RefCell<MapView> = RefCell::new(MapView::default());
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
        let session = host.state.borrow();
        let snapshot = session.snapshot();
        let mut value = serde_json::to_value(&snapshot).expect("serializable session");
        value["map"] = MAP.with_borrow(|map| {
            serde_json::to_value(map.frame(snapshot.world, snapshot.player_id))
                .expect("serializable map")
        });
        serde_json::to_string(&value).expect("serializable browser state")
    })
}

#[wasm_bindgen]
pub fn set_map_view(name: &str) -> Result<(), JsValue> {
    MAP.with_borrow_mut(|map| map.focus(name).map_err(|e| JsValue::from_str(&e)))
}

#[wasm_bindgen]
pub fn zoom_map(factor: f64) -> Result<(), JsValue> {
    MAP.with_borrow_mut(|map| map.zoom(factor).map_err(|e| JsValue::from_str(&e)))
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

fn draw_world(mut gizmos: Gizmos, windows: Query<&Window>) {
    let grid = Color::srgb(0.08, 0.15, 0.19);
    let cyan = Color::srgb(0.3, 0.88, 0.85);
    let amber = Color::srgb(1.0, 0.66, 0.34);
    let aspect = windows
        .iter()
        .next()
        .map_or(1.6, |w| (w.width() / w.height().max(1.0)) as f64);
    let half_width = 1000_f64.max(600.0 * aspect);
    let half_height = 600_f64.max(1000.0 / aspect);
    HOST.with_borrow(|host| {
        let session = host.state.borrow();
        let view = session.snapshot();
        let frame = MAP.with_borrow(|map| map.frame(view.world, view.player_id));
        let spacing = frame.grid_metres / frame.metres_per_unit;
        let offset_x = (-frame.center.x).rem_euclid(frame.grid_metres) / frame.metres_per_unit;
        let offset_y = (-frame.center.y).rem_euclid(frame.grid_metres) / frame.metres_per_unit;
        for i in -(half_width / spacing).ceil() as i32 - 1..=(half_width / spacing).ceil() as i32 {
            let x = (offset_x + i as f64 * spacing) as f32;
            gizmos.line_2d(
                Vec2::new(x, -half_height as f32),
                Vec2::new(x, half_height as f32),
                grid,
            );
        }
        for i in -(half_height / spacing).ceil() as i32 - 1..=(half_height / spacing).ceil() as i32
        {
            let y = (offset_y + i as f64 * spacing) as f32;
            gizmos.line_2d(
                Vec2::new(-half_width as f32, y),
                Vec2::new(half_width as f32, y),
                grid,
            );
        }
        let on_screen =
            |p: Vector2| p.x.abs() <= half_width + 100.0 && p.y.abs() <= half_height + 100.0;
        for ship in &view.world.ships {
            let local = frame.project(ship.position);
            if !on_screen(local) {
                continue;
            }
            let position = Vec2::new(local.x as f32, local.y as f32);
            let velocity = Vec2::new(ship.velocity.x as f32, ship.velocity.y as f32);
            let friendly = ship.id == view.player_id;
            let color = if friendly { cyan } else { amber };
            let radius = (ship.radius / frame.metres_per_unit) as f32;
            if radius >= 3.0 {
                gizmos.circle_2d(Isometry2d::from_translation(position), radius, color);
            }
            // Symbols and direction arrows stay legible independently of physical size.
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
                gizmos.arrow_2d(position, position + velocity.normalize() * 70.0, color);
            }
        }
        for projectile in &view.world.projectiles {
            let local = frame.project(projectile.position);
            if !on_screen(local) {
                continue;
            }
            let position = Vec2::new(local.x as f32, local.y as f32);
            gizmos.circle_2d(
                Isometry2d::from_translation(position),
                (projectile.radius / frame.metres_per_unit).max(3.0) as f32,
                Color::WHITE,
            );
        }
    });
    FRAMES.with_borrow_mut(|frames| *frames = frames.saturating_add(1));
}
