//! A persistent, fuel-limited Lua REPL using the same WEGO command API as the UI.

use std::{cell::RefCell, rc::Rc};

use piccolo::{
    Callback, CallbackReturn, Closure, Context, Executor, Fuel, Lua, Table, Value, Variadic,
};

use crate::{
    physics::{Ship, Vector2},
    session::{Command, Session},
};

pub struct Host {
    pub state: Rc<RefCell<Session>>,
    lua: Lua,
    output: Rc<RefCell<Vec<String>>>,
}

impl Default for Host {
    fn default() -> Self {
        Self::with_session(Session::default())
    }
}

impl Host {
    pub fn with_session(session: Session) -> Self {
        let state = Rc::new(RefCell::new(session));
        let output = Rc::new(RefCell::new(Vec::new()));
        let mut lua = Lua::core();
        lua.enter(|ctx| {
            let game = Table::new(&ctx);
            let command_state = state.clone();
            game.set(
                ctx,
                "command",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let name: String = stack.consume(ctx)?;
                    Command::named(&name)
                        .and_then(|command| command_state.borrow_mut().dispatch(command))
                        .map_err(|error| {
                            piccolo::Error::from_value(Value::String(ctx.intern(error.as_bytes())))
                        })?;
                    stack.replace(ctx, true);
                    Ok(CallbackReturn::Return)
                }),
            )
            .expect("static table key");
            let burn_state = state.clone();
            game.set(
                ctx,
                "burn",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let (x, y): (f64, f64) = stack.consume(ctx)?;
                    burn_state
                        .borrow_mut()
                        .dispatch(Command::PlanBurn(Vector2::new(x, y)))
                        .map_err(|error| {
                            piccolo::Error::from_value(Value::String(ctx.intern(error.as_bytes())))
                        })?;
                    stack.replace(ctx, true);
                    Ok(CallbackReturn::Return)
                }),
            )
            .expect("static table key");
            let fire_state = state.clone();
            game.set(
                ctx,
                "fire",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let (x, y): (f64, f64) = stack.consume(ctx)?;
                    fire_state
                        .borrow_mut()
                        .dispatch(Command::QueueShot(Vector2::new(x, y)))
                        .map_err(|e| {
                            piccolo::Error::from_value(Value::String(ctx.intern(e.as_bytes())))
                        })?;
                    stack.replace(ctx, true);
                    Ok(CallbackReturn::Return)
                }),
            )
            .expect("static key");
            let cancel_state = state.clone();
            game.set(
                ctx,
                "cancel_shot",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let id: u32 = stack.consume(ctx)?;
                    cancel_state
                        .borrow_mut()
                        .dispatch(Command::CancelQueuedShot(id))
                        .map_err(|e| {
                            piccolo::Error::from_value(Value::String(ctx.intern(e.as_bytes())))
                        })?;
                    stack.replace(ctx, true);
                    Ok(CallbackReturn::Return)
                }),
            )
            .expect("static key");
            let aim_state = state.clone();
            game.set(
                ctx,
                "aim",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let solution = aim_state.borrow().aim().map_err(|e| {
                        piccolo::Error::from_value(Value::String(ctx.intern(e.as_bytes())))
                    })?;
                    stack.replace(ctx, (solution.target.x, solution.target.y));
                    Ok(CallbackReturn::Return)
                }),
            )
            .expect("static key");
            let speed_state = state.clone();
            let arrival_state = state.clone();
            game.set(
                ctx,
                "arrive",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let (x, y): (f64, f64) = stack.consume(ctx)?;
                    arrival_state
                        .borrow_mut()
                        .dispatch(Command::PlanArrival(Vector2::new(x, y)))
                        .map_err(|error| {
                            piccolo::Error::from_value(Value::String(ctx.intern(error.as_bytes())))
                        })?;
                    stack.replace(ctx, true);
                    Ok(CallbackReturn::Return)
                }),
            )
            .expect("static table key");
            game.set(
                ctx,
                "speed",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let speed: f64 = stack.consume(ctx)?;
                    speed_state
                        .borrow_mut()
                        .dispatch(Command::SetSpeed(speed))
                        .map_err(|error| {
                            piccolo::Error::from_value(Value::String(ctx.intern(error.as_bytes())))
                        })?;
                    stack.replace(ctx, true);
                    Ok(CallbackReturn::Return)
                }),
            )
            .expect("static table key");
            let read_state = state.clone();
            game.set(
                ctx,
                "state",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let state = read_state.borrow();
                    let view = state.snapshot();
                    let snapshot = Table::new(&ctx);
                    let scale = Table::new(&ctx);
                    scale.set(ctx, "light_speed_mps", view.scale.light_speed_mps)?;
                    scale.set(ctx, "railgun_speed_mps", view.scale.railgun_speed_mps)?;
                    scale.set(ctx, "reference_range_m", view.scale.reference_range_m)?;
                    scale.set(
                        ctx,
                        "reference_flight_seconds",
                        view.scale.reference_flight_seconds,
                    )?;
                    snapshot.set(ctx, "scale", scale)?;
                    let timing = Table::new(&ctx);
                    let settings = crate::timing::SETTINGS;
                    for (key, value) in [
                        ("step_seconds", settings.step_seconds),
                        ("interval_ticks", settings.interval_ticks as f64),
                        ("interval_seconds", settings.interval_seconds),
                        ("step_forward_ticks", settings.step_forward_ticks as f64),
                        ("step_forward_seconds", settings.step_forward_seconds),
                        ("forecast_intervals", settings.forecast_intervals as f64),
                        ("forecast_seconds", settings.forecast_seconds),
                    ] {
                        timing.set(ctx, key, value)?;
                    }
                    snapshot.set(ctx, "timing", timing)?;
                    snapshot.set(ctx, "paused", view.paused)?;
                    snapshot.set(ctx, "phase", view.phase.label())?;
                    snapshot.set(ctx, "ticks", view.world.tick as f64)?;
                    snapshot.set(ctx, "time_seconds", view.time_seconds)?;
                    snapshot.set(ctx, "remaining_seconds", view.remaining_seconds)?;
                    snapshot.set(ctx, "order_start_seconds", view.order_start_seconds)?;
                    snapshot.set(ctx, "speed", view.speed)?;
                    snapshot.set(ctx, "command_number", view.command_number as f64)?;
                    snapshot.set(ctx, "command_count", view.command_log.len() as f64)?;
                    let plan = Table::new(&ctx);
                    plan.set(ctx, "x", view.planned_burn.x)?;
                    plan.set(ctx, "y", view.planned_burn.y)?;
                    snapshot.set(ctx, "planned_burn", plan)?;
                    let active = Table::new(&ctx);
                    active.set(ctx, "x", view.active_burn.x)?;
                    active.set(ctx, "y", view.active_burn.y)?;
                    snapshot.set(ctx, "active_burn", active)?;
                    if let Some(mission) = view.mission {
                        let table = Table::new(&ctx);
                        table.set(ctx, "outcome", mission.outcome.label())?;
                        table.set(ctx, "shots", mission.shots.len() as f64)?;
                        table.set(
                            ctx,
                            "deadline_seconds",
                            mission.config.deadline_tick as f64 * crate::timing::STEP_SECONDS,
                        )?;
                        table.set(
                            ctx,
                            "ai_reason",
                            ctx.intern(mission.decision.reason.as_bytes()),
                        )?;
                        table.set(ctx, "evasions", mission.decision.evasions)?;
                        table.set(
                            ctx,
                            "observed_attacks",
                            mission.sensors.threats.len() as f64,
                        )?;
                        table.set(
                            ctx,
                            "contact_sample_tick",
                            mission.sensors.contact.sampled_tick as f64,
                        )?;
                        table.set(
                            ctx,
                            "contact_received_tick",
                            mission.sensors.contact.received_tick as f64,
                        )?;
                        table.set(ctx, "contact_x", mission.sensors.contact.position.x)?;
                        table.set(ctx, "contact_y", mission.sensors.contact.position.y)?;
                        table.set(ctx, "contact", track_table(ctx, &mission.sensors.contact)?)?;
                        let threats = Table::new(&ctx);
                        for (i, track) in mission.sensors.threats.iter().enumerate() {
                            threats.set(ctx, (i + 1) as i64, track_table(ctx, track)?)?;
                        }
                        table.set(ctx, "threats", threats)?;
                        table.set(ctx, "ai_ax", mission.decision.acceleration.x)?;
                        table.set(ctx, "ai_ay", mission.decision.acceleration.y)?;
                        table.set(
                            ctx,
                            "ai_hold_until_tick",
                            mission.decision.hold_until as f64,
                        )?;
                        table.set(
                            ctx,
                            "reload_seconds",
                            mission.config.reload_ticks as f64 * crate::physics::STEP_SECONDS,
                        )?;
                        table.set(ctx, "queued", !mission.queued.is_empty())?;
                        let queue = Table::new(&ctx);
                        for (i, order) in mission.queued.iter().enumerate() {
                            let item = Table::new(&ctx);
                            item.set(ctx, "id", order.id)?;
                            item.set(
                                ctx,
                                "locked",
                                order.launch_tick
                                    < (view.order_start_seconds / crate::physics::STEP_SECONDS)
                                        as u64,
                            )?;
                            item.set(ctx, "target_x", order.target.x)?;
                            item.set(ctx, "target_y", order.target.y)?;
                            item.set(ctx, "launch_tick", order.launch_tick as f64)?;
                            item.set(
                                ctx,
                                "launch_seconds",
                                order.launch_tick as f64 * crate::physics::STEP_SECONDS,
                            )?;
                            queue.set(ctx, (i + 1) as i64, item)?;
                        }
                        table.set(ctx, "queue", queue)?;
                        snapshot.set(ctx, "mission", table)?;
                    }
                    let ships = Table::new(&ctx);
                    for (index, ship) in view.world.ships.iter().enumerate() {
                        ships.set(ctx, (index + 1) as i64, ship_table(ctx, ship)?)?;
                        if ship.id == view.player_id {
                            snapshot.set(ctx, "ship", ship_table(ctx, ship)?)?;
                        }
                    }
                    snapshot.set(ctx, "ships", ships)?;
                    snapshot.set(ctx, "projectile_count", view.world.projectiles.len() as f64)?;
                    snapshot.set(ctx, "impact_count", view.world.impacts.len() as f64)?;
                    stack.replace(ctx, snapshot);
                    Ok(CallbackReturn::Return)
                }),
            )
            .expect("static table key");
            ctx.set_global("game", game).expect("static global key");

            let printed = output.clone();
            ctx.set_global(
                "print",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let values: Variadic<Vec<Value>> = stack.consume(ctx)?;
                    let mut lines = printed.borrow_mut();
                    if lines.len() < 64 {
                        lines.push(
                            values
                                .0
                                .iter()
                                .map(format_value)
                                .collect::<Vec<_>>()
                                .join("\t"),
                        );
                    }
                    Ok(CallbackReturn::Return)
                }),
            )
            .expect("static global key");
        });
        Self { state, lua, output }
    }
}

impl Host {
    pub fn command(&mut self, name: &str) -> Result<(), String> {
        self.state.borrow_mut().dispatch(Command::named(name)?)
    }

    pub fn plan_burn(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.state
            .borrow_mut()
            .dispatch(Command::PlanBurn(Vector2::new(x, y)))
    }

    pub fn set_speed(&mut self, speed: f64) -> Result<(), String> {
        self.state.borrow_mut().dispatch(Command::SetSpeed(speed))
    }

    pub fn plan_arrival(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.state
            .borrow_mut()
            .dispatch(Command::PlanArrival(Vector2::new(x, y)))
    }

    pub fn evaluate(&mut self, source: &str) -> Result<String, String> {
        if source.len() > 4096 {
            return Err("Input exceeds 4096 bytes.".into());
        }
        self.output.borrow_mut().clear();
        let expression = format!("return {source}");
        let executor = self
            .lua
            .try_enter(|ctx| {
                let closure = Closure::load(ctx, None, expression.as_bytes())
                    .or_else(|_| Closure::load(ctx, None, source.as_bytes()))?;
                Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
            })
            .map_err(|error| error.to_string())?;

        // Stop an accidental infinite loop without blocking the browser event loop.
        // This is a developer convenience, not a hardened resource sandbox.
        let finished = self
            .lua
            .enter(|ctx| ctx.fetch(&executor).step(ctx, &mut Fuel::with(20_000)));
        if !finished {
            return Err(
                "Lua instruction budget exceeded; earlier side effects are retained.".into(),
            );
        }
        let result = self
            .lua
            .try_enter(|ctx| {
                let values: Variadic<Vec<Value>> = ctx.fetch(&executor).take_result(ctx)??;
                Ok(values
                    .0
                    .iter()
                    .map(format_value)
                    .collect::<Vec<_>>()
                    .join("\t"))
            })
            .map_err(|error| error.to_string())?;
        let mut lines = self.output.borrow_mut().clone();
        if !result.is_empty() {
            lines.push(result);
        }
        Ok(if lines.is_empty() {
            "ok".into()
        } else {
            lines.join("\n")
        })
    }
}

fn track_table<'gc>(
    ctx: Context<'gc>,
    track: &crate::observation::Track,
) -> Result<Table<'gc>, piccolo::Error<'gc>> {
    let table = Table::new(&ctx);
    for (key, value) in [
        ("id", track.id as f64),
        ("sampled_tick", track.sampled_tick as f64),
        ("received_tick", track.received_tick as f64),
        ("x", track.position.x),
        ("y", track.position.y),
        ("vx", track.velocity.x),
        ("vy", track.velocity.y),
        ("radius", track.radius),
    ] {
        table.set(ctx, key, value)?;
    }
    Ok(table)
}

fn ship_table<'gc>(ctx: Context<'gc>, ship: &Ship) -> Result<Table<'gc>, piccolo::Error<'gc>> {
    let table = Table::new(&ctx);
    for (key, value) in [
        ("id", ship.id as f64),
        ("x", ship.position.x),
        ("y", ship.position.y),
        ("vx", ship.velocity.x),
        ("vy", ship.velocity.y),
        ("remaining_dv", ship.remaining_dv),
        ("spent_dv", ship.spent_dv),
        ("max_acceleration", ship.max_acceleration),
    ] {
        table.set(ctx, key, value)?;
    }
    Ok(table)
}

fn format_value(value: &Value<'_>) -> String {
    let text = value.to_string();
    text.chars().take(1024).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_lua_and_preserves_globals() {
        let mut host = Host::default();
        assert_eq!(host.evaluate("1 + 2").unwrap(), "3");
        host.evaluate("answer = 6 * 7").unwrap();
        assert_eq!(host.evaluate("answer").unwrap(), "42");
        assert_eq!(
            host.evaluate("print('hello', answer)").unwrap(),
            "hello\t42"
        );
    }

    #[test]
    fn ui_and_lua_share_state_and_validation() {
        let mut host = Host::default();
        host.plan_burn(crate::scenario::MAX_ACCELERATION, 0.0)
            .unwrap();
        host.command("commit").unwrap();
        host.command("pause").unwrap();
        host.command("step").unwrap();
        assert_eq!(host.evaluate("game.state().ticks").unwrap(), "1");
        host.evaluate("game.command('resume')").unwrap();
        assert!(host.command("step").is_err());
        assert!(host.evaluate("game.command('step')").is_err());
        host.command("pause").unwrap();
        host.evaluate("game.burn(0, 0)").unwrap();
        host.plan_burn(0.0, 0.0).unwrap();
        assert_eq!(
            host.state.borrow().snapshot().active_burn.x,
            crate::scenario::MAX_ACCELERATION
        );
        host.evaluate("game.command('step')").unwrap();
        assert_eq!(host.state.borrow().world().tick, 2);
        assert_eq!(host.evaluate("game.state().ship.vx > 1").unwrap(), "true");
        host.command("reset").unwrap();
        assert_eq!(host.state.borrow().world(), Session::default().world());
    }

    #[test]
    fn state_is_a_snapshot_and_errors_are_recoverable() {
        let mut host = Host::default();
        host.evaluate("game.state().ticks = 99").unwrap();
        host.evaluate("game.state().ship.remaining_dv = 999")
            .unwrap();
        assert_eq!(host.state.borrow().world().tick, 0);
        assert_eq!(
            host.state.borrow().world().ships[0].remaining_dv,
            crate::scenario::INITIAL_DELTA_V
        );
        assert!(host.evaluate("function (").is_err());
        assert!(host.evaluate("error('probe error')").is_err());
        assert!(
            host.evaluate("while true do end")
                .unwrap_err()
                .contains("budget")
        );
        assert_eq!(host.evaluate("2 + 3").unwrap(), "5");
    }

    #[test]
    fn lua_validation_and_interval_boundary_match_the_native_api() {
        let mut host = Host::default();
        assert!(host.evaluate("game.burn(0/0, 0)").is_err());
        assert!(host.evaluate("game.speed(1.5)").is_err());
        host.evaluate(
            "game.burn(1000, 0); game.speed(64); game.command('commit'); game.command('pause')",
        )
        .unwrap();
        host.evaluate("for i=1,game.state().timing.interval_ticks do game.command('step') end")
            .unwrap();
        assert_eq!(host.evaluate("game.state().phase").unwrap(), "planning");
        assert_eq!(
            host.evaluate("game.state().time_seconds").unwrap(),
            crate::timing::INTERVAL_SECONDS.to_string()
        );
        assert_eq!(host.evaluate("game.state().command_count").unwrap(), "1");
        host.evaluate("game.command('next_turn')").unwrap();
        assert_eq!(
            host.evaluate("game.state().time_seconds").unwrap(),
            (2.0 * crate::timing::INTERVAL_SECONDS).to_string()
        );
        assert_eq!(host.evaluate("game.state().phase").unwrap(), "planning");
        assert_eq!(host.evaluate("game.state().command_count").unwrap(), "2");
    }
}
