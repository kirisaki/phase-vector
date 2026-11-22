# Step 4: future preview and arrival-time shooting

## Scope

The tactical map now has a read-only time slider and a timed shot command. Existing
Live aiming, friendly burn planning, four pending reservations, 30-second reload,
and 120-second command intervals remain available. There is no new simulation
clock, damage model, guidance, or scoring system.

## Controls and prediction

Live retains the earliest-free-slot gun workflow. Moving the slider selects an
absolute simulation time between now and the mission deadline, bounded to a
3,600-second planning horizon. The selected time stays fixed during playback;
when the clock reaches it, the UI returns to Live. Live also restores automatic
shot timing immediately. Scrubbing does not advance the world or edit orders.

The map overlays a cyan future friendly position/path and velocity arrow, an
amber observed enemy estimate, and numbered future round positions. Reserved
rounds appear only at or after their planned launch. Existing live symbols stay
visible for comparison. Pan and pinch zoom work in either view; future symbols
may fall outside the current framing. The same Burn control edits the next
interval and updates the forecast. There are no intermediate waypoints or
independently authored burns for later turns: the latest draft repeats.

In future preview, Track lead and the snap assist use the observed enemy position
at the selected time. Drag Aim or Fine aim to choose a different fixed world point.
Queue reserves that point and requested arrival time. Moving the slider or Aim
later does not rewrite that reservation. The feedback reports both the requested
and solved arrival times; the queue retains the requested time for reference.

The enemy forecast extrapolates the latest delivered position/velocity. It does
not run enemy AI, read true motion, or assume continued acceleration. Friendly
prediction integrates the frozen active burn until the next boundary and the
current draft afterwards, including acceleration and remaining delta-v limits.
Ballistic previews assume unobstructed flight: they do not promise hits, future
mission outcomes, or survival after crossing another ship.

## Timed gun solver

`Snapshot::preview_timed_shot` and `Command::QueueTimedShot` share one solver.
It advances a copy of the friendly ship once through the bounded planning window,
evaluating each possible launch tick at or after the editable command boundary.
Each candidate uses the same fixed-speed gun solver as actual firing, including
inherited ship velocity. It chooses the reload-compatible candidate with the
smallest absolute arrival-time error; exact ties favor the earlier launch.

Physical launches stay on the **0.25-second tick grid**. Requested arrival times
need not lie exactly on that grid, but the computed arrival must differ by no
more than **0.25 seconds** and may not exceed the mission deadline. The actual
trajectory reaches the specified point at the displayed computed time. Neither
muzzle speed nor target position is altered to fake an exact requested time.
This tolerance is a PoC assumption; it can matter for a moving target and is not
a hit guarantee. The UI exposes the tolerance and horizon from Rust settings.

Errors distinguish invalid/past/deadline-exceeding times, unreachable arrivals,
reload conflicts, and a full queue. Rejected additions do not change existing
reservations, IDs, world state, or ammunition statistics. Live reservations still
choose the earliest reload slot and solve their arrival without a requested time.

## Replanning and command boundaries

Timed reservations keep their ID, fixed target, and requested arrival. Changing
the draft maneuver recalculates editable timed launch ticks. Cancellation also
retries these requests because it may free a reload window. Live reservations
retain their launch slots; locked active-interval shots are also fixed anchors.
Editable timed requests are processed in ascending reservation-ID order, so
older requests have priority if a maneuver creates competition for a slot.

An infeasible reservation remains in the queue with an error and consumes one of
the four slots. It does not occupy a reload window or launch silently. Its old
launch tick is only retained metadata, not a valid firing plan. The user can
restore a feasible maneuver or cancel the reservation. The current interval
finishes normally, but the next interval cannot begin until all retained planning
errors are resolved. Normal UI feedback and the queue show the reason.

At the boundary, the complete firing plan is validated before any launch. Orders
in the active interval then freeze; later draft edits cannot move them. Flying
rounds retain their launch velocity. Reservations and rounds persist across
intervals. Command logs include resolved launch ticks and requested arrival times
so replay executes the same frozen orders without another planning search.

## Shared APIs

Browser bindings:

- `forecast_at(seconds)` returns a JSON forecast without advancing time.
- `preview_timed_shot(x, y, seconds)` returns the solved firing plan or an error.
- `queue_timed_shot(x, y, seconds)` submits the shared command.

Lua uses metres and absolute simulation seconds:

```lua
local f = game.forecast(600)
local shot = game.preview_shot_at(f.enemy_x, f.enemy_y, 600)
print(shot.launch_seconds, shot.arrival_seconds)
game.fire_at(f.enemy_x, f.enemy_y, 600)
```

`game.forecast` is observation-only for the enemy, even though the separate
`game.state` diagnostic interface exposes true state. Queue entries include
`requested_arrival_seconds`, `error`, and the existing resolved firing time.
`game.fire(x, y)` and `game.aim()` retain the Live workflow. Parameter editing
and formal evaluation scores remain step 5.

## Verification

Native planning tests cover fixed muzzle speed and target arrival, timing
quantization, atomic invalid requests and reload conflicts, maneuver re-solving,
invalid-plan recovery/cancellation, locked orders, ballistic persistence,
forecast/execution agreement, hidden-enemy-state independence, Lua parity,
command-log replay, and stopping invalid future plans at the next boundary.

The mission browser test exercises the actual slider, timed Queue, reload error,
numeric maneuver edit, immutable reservations during scrubbing, impossible early
arrival, Live restoration, and touch sliders in portrait/landscape layouts.
Existing mission, navigation, Lua, and sandbox regressions remain applicable.
