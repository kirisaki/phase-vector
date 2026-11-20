//! Browser bindings and presentation of the renderer-independent WEGO session.

use std::cell::RefCell;

use bevy::{camera::ScalingMode, prelude::*, window::WindowResolution};
use wasm_bindgen::prelude::*;

// Enable the browser entropy backends required by the Lua VM's dependencies.
use getrandom as _;
use getrandom03 as _;

use crate::{
    map::MapView,
    physics::Vector2,
    repl::Host,
    session::{Command, Session},
};

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::with_session(Session::blockade()));
    static MAP: RefCell<MapView> = RefCell::new(MapView::default());
    static FRAMES: RefCell<u32> = const { RefCell::new(0) };
    static ROUTE_EDITING: RefCell<bool> = const { RefCell::new(false) };
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
pub fn plan_arrival(x: f64, y: f64) -> Result<(), JsValue> {
    HOST.with_borrow_mut(|host| host.plan_arrival(x, y).map_err(|e| JsValue::from_str(&e)))
}

#[wasm_bindgen]
pub fn queue_shot(x: f64, y: f64) -> Result<(), JsValue> {
    HOST.with_borrow(|host| {
        host.state
            .borrow_mut()
            .dispatch(Command::QueueShot(Vector2::new(x, y)))
            .map_err(|e| JsValue::from_str(&e))
    })
}

#[wasm_bindgen]
pub fn cancel_shot(id: u32) -> Result<(), JsValue> {
    HOST.with_borrow(|host| {
        host.state
            .borrow_mut()
            .dispatch(Command::CancelQueuedShot(id))
            .map_err(|e| JsValue::from_str(&e))
    })
}

#[wasm_bindgen]
pub fn preview_shot(x: f64, y: f64) -> Result<String, JsValue> {
    HOST.with_borrow(|host| {
        let session = host.state.borrow();
        let solution = session
            .preview_shot(Vector2::new(x, y))
            .map_err(|e| JsValue::from_str(&e))?;
        Ok(serde_json::to_string(&solution).expect("serializable shot"))
    })
}

#[wasm_bindgen]
pub fn preview_arrival(x: f64, y: f64) -> Result<String, JsValue> {
    HOST.with_borrow(|host| {
        let session = host.state.borrow();
        let view = session.snapshot();
        let acceleration = crate::maneuver::solve(&view.planning_ship(), Vector2::new(x, y))
            .map_err(|e| JsValue::from_str(&e))?;
        Ok(serde_json::to_string(&view.planned_route(acceleration)).expect("serializable route"))
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
        MAP.with_borrow(|map| {
            serde_json::to_string(&crate::presentation::State::new(&snapshot, map))
                .expect("serializable browser state")
        })
    })
}

#[wasm_bindgen]
pub fn set_map_view(name: &str, aspect: f64) -> Result<(), JsValue> {
    HOST.with_borrow(|host| {
        let session = host.state.borrow();
        let view = session.snapshot();
        MAP.with_borrow_mut(|map| {
            map.focus(name).map_err(|e| JsValue::from_str(&e))?;
            // Fit before publishing the new view to SVG pointer controls.
            map.update_auto_zoom(&view.presentation_world(), view.player_id, aspect, 0.0);
            Ok(())
        })
    })
}

#[wasm_bindgen]
pub fn set_route_editing(editing: bool) {
    ROUTE_EDITING.with_borrow_mut(|value| *value = editing);
}

#[wasm_bindgen]
pub fn zoom_map(factor: f64) -> Result<(), JsValue> {
    MAP.with_borrow_mut(|map| map.zoom(factor).map_err(|e| JsValue::from_str(&e)))
}

#[wasm_bindgen]
pub fn navigate_map(
    anchor_x: f64,
    anchor_y: f64,
    offset_x: f64,
    offset_y: f64,
    scale: f64,
) -> Result<(), JsValue> {
    MAP.with_borrow_mut(|map| {
        map.navigate(
            Vector2::new(anchor_x, anchor_y),
            Vector2::new(offset_x, offset_y),
            scale,
        )
        .map_err(|e| JsValue::from_str(&e))
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
        .insert_resource(Time::<Fixed>::from_hz(crate::timing::PLAYBACK_HZ as f64))
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

fn draw_world(mut gizmos: Gizmos, windows: Query<&Window>, time: Res<Time>) {
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
        let visible = view.presentation_world();
        let frame = MAP.with_borrow_mut(|map| {
            if !ROUTE_EDITING.with_borrow(|editing| *editing) {
                map.update_auto_zoom(&visible, view.player_id, aspect, time.delta_secs_f64());
            }
            map.frame(&visible, view.player_id)
        });
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
        if !ROUTE_EDITING.with_borrow(|v| *v) {
            let planning_ship = view.planning_ship();
            let route = crate::maneuver::preview(&planning_ship, view.planned_burn);
            let project = |p| {
                let p = frame.project(p);
                Vec2::new(p.x as f32, p.y as f32)
            };
            let coast_start = project(planning_ship.position);
            let coast_end = project(route.coast_endpoint);
            for i in 0..32 {
                gizmos.line_2d(
                    coast_start.lerp(coast_end, i as f32 / 32.0),
                    coast_start.lerp(coast_end, (i as f32 + 0.5) / 32.0),
                    Color::srgb(0.35, 0.43, 0.5),
                );
            }
            // The SVG overlay draws the planned route with continuous pixel-spaced dots.
        }
        for ship in &visible.ships {
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
