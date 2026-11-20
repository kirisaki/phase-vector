use phase_vector::{
    map::MapView,
    mission::solve_shot,
    physics::{STEP_SECONDS, Vector2 as V},
    presentation::State,
    session::{Command as C, Session},
};
use serde_json::to_value;

#[test]
fn sandbox_view_preserves_world_and_session_fields() {
    let mut session = Session::default();
    session.dispatch(C::PlanBurn(V::new(5.0, 0.0))).unwrap();
    session.dispatch(C::StepForward).unwrap();
    let snapshot = session.snapshot();
    let json = to_value(State::new(&snapshot, &MapView::default())).unwrap();
    assert_eq!(json["world"], to_value(session.world()).unwrap());
    assert!(json["mission"].is_null());
    assert_eq!(json["phase"], "executing");
    assert_eq!(json["paused"], true);
    assert_eq!(json["time_seconds"], 10.0);
    assert_eq!(json["order_start_seconds"], 120.0);
    assert_eq!(json["remaining_seconds"], 110.0);
    assert_eq!(json["command_log"], to_value(snapshot.command_log).unwrap());
    assert_eq!(
        json["maneuver"],
        to_value(snapshot.planned_route(snapshot.planned_burn)).unwrap()
    );
}

#[test]
fn hidden_enemy_changes_cannot_change_normal_ui_json_or_camera() {
    let mut session = Session::blockade();
    session.dispatch(C::StepForward).unwrap();
    let mut snapshot = session.snapshot();
    let map = MapView::default();
    let baseline = to_value(State::new(&snapshot, &map)).unwrap();
    let own = &baseline["world"]["ships"][0];
    assert!(own.get("remaining_dv").is_some());
    let enemy = &baseline["world"]["ships"][1];
    assert!(enemy.get("remaining_dv").is_none());
    assert!(enemy.get("spent_dv").is_none());

    let mut hidden_world = session.world().clone();
    hidden_world.ships[1].position = V::new(9e9, -9e9);
    hidden_world.ships[1].velocity = V::new(200_000.0, 200_000.0);
    hidden_world.ships[1].remaining_dv = 123.0;
    hidden_world.ships[1].spent_dv = 456.0;
    let mut hidden_mission = session.mission().unwrap().clone();
    hidden_mission.decision.reason = "Privileged decision".into();
    hidden_mission.decision.acceleration = V::new(12.0, 34.0);
    snapshot.world = &hidden_world;
    snapshot.mission = Some(&hidden_mission);
    assert_eq!(baseline, to_value(State::new(&snapshot, &map)).unwrap());

    // A delivered observation, unlike a change in hidden truth, must affect the view.
    let mut observed = hidden_mission.clone();
    observed.sensors.contact.position.x += 1_000_000.0;
    observed.sensors.contact.velocity.x += 1000.0;
    snapshot.mission = Some(&observed);
    let changed = to_value(State::new(&snapshot, &map)).unwrap();
    assert_ne!(baseline["map"]["center"], changed["map"]["center"]);
    assert_ne!(baseline["mission"]["aim"], changed["mission"]["aim"]);
    let contact = &observed.sensors.contact;
    let estimated = contact.position_at(hidden_world.tick);
    assert_eq!(
        changed["world"]["ships"][1]["position"],
        to_value(estimated).unwrap()
    );
    assert_eq!(
        changed["mission"]["prediction"][0],
        to_value(estimated).unwrap()
    );
    assert_eq!(
        changed["mission"]["prediction"][1],
        to_value(estimated + contact.velocity * 360.0).unwrap()
    );
}

#[test]
fn presentation_keeps_reservations_and_flight_separate_without_advancing_time() {
    let mut session = Session::blockade();
    session
        .dispatch(C::QueueShot(session.aim().unwrap().target))
        .unwrap();
    session.dispatch(C::PlanBurn(V::new(20.0, 0.0))).unwrap();
    session.dispatch(C::StepForward).unwrap();
    let target = session.aim().unwrap().target;
    session.dispatch(C::QueueShot(target)).unwrap();
    let before = session.world().clone();
    let mission = session.mission().unwrap().clone();
    let snapshot = session.snapshot();
    let state = State::new(&snapshot, &MapView::default());
    let shown = state.mission.as_ref().unwrap();
    assert_eq!(state.world.projectiles.len(), 1);
    assert_eq!(shown.shots.len(), 1);
    assert_eq!(shown.queued.unwrap().launch_tick, 480);
    assert_eq!(shown.queued.unwrap().target, target);
    let solution = solve_shot(&snapshot.planning_ship(), target, 480).unwrap();
    assert_eq!(shown.queued_solution.as_ref().unwrap(), &solution);
    assert_eq!(shown.aim.as_ref().unwrap(), &session.aim().unwrap());
    assert_eq!(
        shown.contact_age,
        (before.tick - shown.contact.sampled_tick) as f64 * STEP_SECONDS
    );
    assert_eq!(session.world(), &before);
    assert_eq!(session.mission().unwrap(), &mission);
}
