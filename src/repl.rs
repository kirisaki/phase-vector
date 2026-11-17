//! A persistent, fuel-limited Lua probe using the same command API as the UI.

use std::{cell::RefCell, rc::Rc};

use piccolo::{Callback, CallbackReturn, Closure, Executor, Fuel, Lua, Table, Value, Variadic};

use crate::probe::ProbeState;

pub struct Host {
    pub state: Rc<RefCell<ProbeState>>,
    lua: Lua,
    output: Rc<RefCell<Vec<String>>>,
}

impl Default for Host {
    fn default() -> Self {
        let state = Rc::new(RefCell::new(ProbeState::default()));
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
                    command_state.borrow_mut().command(&name).map_err(|error| {
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
                    let snapshot = Table::new(&ctx);
                    snapshot.set(ctx, "paused", state.paused)?;
                    snapshot.set(ctx, "ticks", state.ticks)?;
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
        self.state.borrow_mut().command(name)
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
        host.command("step").unwrap();
        assert_eq!(host.evaluate("game.state().ticks").unwrap(), "1");
        host.evaluate("game.command('resume')").unwrap();
        assert!(host.command("step").is_err());
        assert!(host.evaluate("game.command('step')").is_err());
        host.command("pause").unwrap();
        host.evaluate("game.command('step')").unwrap();
        assert_eq!(host.state.borrow().ticks, 2);
        host.command("reset").unwrap();
        assert_eq!(*host.state.borrow(), ProbeState::default());
    }

    #[test]
    fn state_is_a_snapshot_and_errors_are_recoverable() {
        let mut host = Host::default();
        host.evaluate("game.state().ticks = 99").unwrap();
        assert_eq!(host.state.borrow().ticks, 0);
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
    fn paused_probe_does_not_advance() {
        let mut state = ProbeState::default();
        state.advance();
        assert_eq!(state.ticks, 0);
        state.command("resume").unwrap();
        state.advance();
        assert_eq!(state.ticks, 1);
        assert!(state.command("fire").is_err());
    }
}
