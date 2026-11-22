use phase_vector::{
    parameters::{Parameter as P, RunKind},
    repl::Host,
    session::{Command as C, Session},
};

#[test]
fn invalid_and_running_edits_are_atomic() {
    let mut s = Session::blockade();
    for parameter in P::ALL {
        for value in [f64::NAN, f64::INFINITY, -1.0, f64::MAX] {
            let before = format!("{s:?}");
            assert!(s.dispatch(C::Configure(parameter, value)).is_err());
            assert_eq!(format!("{s:?}"), before);
        }
    }
    for parameter in [
        P::DeadlineSeconds,
        P::ReloadSeconds,
        P::SensorProcessingSeconds,
    ] {
        let before = format!("{s:?}");
        assert!(s.dispatch(C::Configure(parameter, 1.1)).is_err());
        assert_eq!(format!("{s:?}"), before);
    }
    s.dispatch(C::Resume).unwrap();
    let before = format!("{s:?}");
    assert!(s.dispatch(C::Configure(P::ReactionSeconds, 60.0)).is_err());
    assert_eq!(format!("{s:?}"), before);
    let mut sandbox = Session::default();
    assert!(
        sandbox
            .dispatch(C::Configure(P::ReactionSeconds, 60.0))
            .is_err()
    );
}

#[test]
fn initial_edits_require_an_empty_queue_and_retry_preserves_the_baseline() {
    let mut s = Session::blockade();
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    let before = format!("{s:?}");
    assert!(s.dispatch(C::Configure(P::ReloadSeconds, 60.0)).is_err());
    assert_eq!(format!("{s:?}"), before);
    s.dispatch(C::CancelShot).unwrap();
    s.dispatch(C::Configure(P::ReloadSeconds, 60.0)).unwrap();
    s.dispatch(C::Configure(P::DeadlineSeconds, 960.0)).unwrap();
    s.dispatch(C::Configure(P::SensorProcessingSeconds, 2.0))
        .unwrap();
    s.dispatch(C::Configure(P::ReactionSeconds, 90.0)).unwrap();
    let baseline = s.mission().unwrap().config.clone();
    assert_eq!(s.snapshot().run_kind, RunKind::Custom);
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
    s.dispatch(C::Step).unwrap();
    let frozen = s.clone();
    s.dispatch(C::Configure(P::ReactionSeconds, 30.0)).unwrap();
    assert_eq!(s.world(), frozen.world());
    assert_eq!(s.snapshot().command_log, frozen.snapshot().command_log);
    assert_eq!(s.snapshot().active_burn, frozen.snapshot().active_burn);
    assert_eq!(
        s.mission().unwrap().queued,
        frozen.mission().unwrap().queued
    );
    assert_eq!(
        s.mission().unwrap().sensors,
        frozen.mission().unwrap().sensors
    );
    assert_eq!(s.snapshot().run_kind, RunKind::Debug);
    assert!(s.snapshot().parameter_edits.last().unwrap().during_play);
    assert!(s.dispatch(C::Configure(P::ReloadSeconds, 30.0)).is_err());
    s.dispatch(C::Reset).unwrap();
    assert_eq!(s.mission().unwrap().config, baseline);
    assert_eq!(s.world(), &phase_vector::scenario::initial_world());
    assert_eq!(s.snapshot().run_kind, RunKind::Custom);
    assert!(s.snapshot().parameter_edits.is_empty());
    assert!(s.snapshot().paused);
    s.dispatch(C::StartMission).unwrap();
    assert_eq!(s.snapshot().run_kind, RunKind::Standard);
    assert_eq!(s.mission().unwrap().config, Default::default());
}

#[test]
fn paused_policy_edits_replay_and_do_not_rewrite_flying_rounds() {
    let mut first = Session::blockade();
    let mut second = first.clone();
    for s in [&mut first, &mut second] {
        s.dispatch(C::QueueShot(s.aim().unwrap().target)).unwrap();
        s.dispatch(C::NextTurn).unwrap();
        let rounds = s.world().projectiles.clone();
        s.dispatch(C::Configure(P::SafetyRadiusM, 100_000.0))
            .unwrap();
        s.dispatch(C::Configure(P::HoldSeconds, 0.0)).unwrap();
        s.dispatch(C::Configure(P::RecoverySeconds, 60.0)).unwrap();
        assert_eq!(s.world().projectiles, rounds);
    }
    first.dispatch(C::NextTurn).unwrap();
    for _ in 0..phase_vector::timing::INTERVAL_TICKS {
        second.dispatch(C::Step).unwrap();
    }
    assert_eq!(first.world(), second.world());
    assert_eq!(first.mission(), second.mission());
    assert_eq!(
        first.snapshot().parameter_edits,
        second.snapshot().parameter_edits
    );
}

#[test]
fn lua_uses_shared_validation_and_exposes_detached_settings_and_history() {
    let mut lua = Host::with_session(Session::blockade());
    let mut direct = Session::blockade();
    for source in [
        "game.configure('typo', 60)",
        "game.configure('reaction_seconds', true)",
        "game.configure('reaction_seconds', 0/0)",
    ] {
        assert!(lua.evaluate(source).is_err());
        assert_eq!(lua.state.borrow().snapshot().run_kind, RunKind::Standard);
    }
    lua.evaluate("game.configure('reaction_seconds', 60)")
        .unwrap();
    direct
        .dispatch(C::Configure(P::ReactionSeconds, 60.0))
        .unwrap();
    assert_eq!(lua.state.borrow().mission(), direct.mission());
    lua.evaluate("local p = game.parameters(); p.reaction_seconds = 0")
        .unwrap();
    assert_eq!(
        lua.evaluate("game.parameters().reaction_seconds").unwrap(),
        "60"
    );
    assert_eq!(
        lua.evaluate("game.state().mission.parameter_edits[1].parameter")
            .unwrap(),
        "reaction_seconds"
    );
    lua.evaluate("game.command('step'); game.configure('reaction_seconds', 0)")
        .unwrap();
    assert_eq!(
        lua.evaluate("game.state().mission.run_kind").unwrap(),
        "debug"
    );
    assert_eq!(
        lua.evaluate("game.state().mission.initial_parameters.reaction_seconds")
            .unwrap(),
        "60"
    );
    lua.evaluate("game.command('retry')").unwrap();
    assert_eq!(
        lua.evaluate("game.parameters().reaction_seconds").unwrap(),
        "60"
    );
    lua.evaluate(
        "game.command('mission'); game.configure('deadline_seconds', 0.25); game.command('step')",
    )
    .unwrap();
    let before = format!("{:?}", lua.state.borrow());
    assert!(
        lua.evaluate("game.configure('reaction_seconds', 20)")
            .is_err()
    );
    assert_eq!(format!("{:?}", lua.state.borrow()), before);
}
