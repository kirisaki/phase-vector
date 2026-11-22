# Step 4: future preview and scheduled shooting

## Scope

The tactical map now has a future-position slider and explicit firing-time reservations.
The original arrival-constrained API remains available for Lua diagnostics. Existing
Live aiming, friendly burn planning, four pending reservations, 30-second reload,
and 120-second command intervals remain available. There is no new simulation
clock, damage model, guidance, or scoring system.

## Controls and prediction

Live retains the earliest-free-slot gun workflow. Moving the slider selects an
absolute firing time between now and the mission deadline, bounded to a
3,600-second planning horizon. The selected time stays fixed during playback;
when the clock reaches it, the UI returns to Live. Live also restores automatic
shot timing immediately. Scrubbing does not advance the world or edit orders.

The timeline assigns 40% of its width to the remaining time until the next
command boundary (cyan), and 60% to later firing times (amber). If only the
current interval remains, it uses the full width. Tick marks identify command
boundaries; labels show the next boundary and the horizon in absolute seconds.
The mapping is frozen during a pointer/touch drag while simulation continues.
Pointer input snaps to a boundary only within both 5 pixels and 6 seconds.
Keyboard arrows bypass snapping and move one physical tick; Page Up/Down move
one command interval, and Home/End select the range limits. These are UI tuning
assumptions, with no changes to command times or the gun solver.

Green diamonds mark free firing-window starts, with a Ready label for the next
marked opening at/after the selected time (or the last opening if none follows).
`Mission::available_launch_ticks` derives these starts from the same
`next_launch_tick` logic used by Live, respecting actual reload readiness,
reservations, queue capacity, editable command boundary, outcome, and deadline.
Presentation exposes them as `available_launch_seconds`; the UI only maps them
to visible positions. Within 5 screen pixels, readiness takes precedence over a
nearby command tick, including on the compressed far-future scale. Keyboard
nudges bypass this snap. Adding/cancelling orders updates markers without changing
the selected time or accepted orders; full queues and ended missions hide them.
Markers describe gun availability, not guaranteed target reachability or hits.

Press Q while focusing or dragging the timeline to queue the current target and
firing time. Key repeats and Ctrl/Alt/Meta combinations do not queue shots. A second touch
can tap Queue while the first finger continues holding the timeline; only the
owning pointer releases the fixed scale. Releasing the second finger outside
Queue or cancelling that touch does not submit an order. All paths share the
same validated command, and scrubbing afterwards leaves accepted orders fixed.
Queue reports reload conflicts, full capacity, and locked active-interval times.
The deadline is available for preview but cannot be used as a firing tick.

The map overlays a cyan future friendly position/path and velocity arrow, an
amber observed enemy estimate, and numbered future round positions. Reserved
rounds appear only at or after their planned launch. Existing live symbols stay
visible for comparison. Pan and pinch zoom work in either view; future symbols
may fall outside the current framing. The same Burn control edits the next
interval and updates the forecast. There are no intermediate waypoints or
independently authored burns for later turns: the latest draft repeats.

In future preview, Track lead and snap assist compute an intercept from the
friendly ship's predicted firing position and velocity, extrapolating only the
delivered enemy track. AIM is therefore ahead of the enemy-at-fire marker by
projectile flight time. The later enemy arrival estimate remains visible.
Drag Aim or Fine aim to choose a different fixed world point. Queue stores that
point and firing tick, not a required arrival time. Moving the slider afterwards
does not rewrite reservations. Feedback reports firing, flight, and arrival times.

The enemy forecast extrapolates the latest delivered position/velocity. It does
not run enemy AI, read true motion, or assume continued acceleration. Friendly
prediction integrates the frozen active burn until the next boundary and the
current draft afterwards, including acceleration and remaining delta-v limits.
Ballistic previews assume unobstructed flight: they do not promise hits, future
mission outcomes, or survival after crossing another ship.

## Fixed firing-time command

`Snapshot::preview_launch_shot` and `Command::QueueLaunchShot` share validation.
The selected firing time must be on the 0.25-second grid, inside the planning
horizon, before the deadline, and at or after the editable command boundary.
Capacity and reload conflicts are rejected without moving the chosen time.
The same physical solver determines direction and arrival from the predicted
launch state. `Snapshot::aim_at` and `Forecast::aim` provide observation-only lead.
Arrival can exceed the deadline, as with Live shots; no late hit is guaranteed.

Maneuver edits preserve the target and firing tick, recomputing ballistic previews.
A reserved target is not automatically retargeted to the moving enemy. Active
orders and flying rounds retain their existing command semantics.

## Arrival-constrained solver (Lua/API)

This original solver remains available separately from the firing-time slider.

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

Arrival-constrained reservations keep their ID, fixed target, and requested arrival. Changing
the draft maneuver recalculates editable timed launch ticks. Cancellation also
retries these requests because it may free a reload window. Live and explicit firing-time reservations
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

- `forecast_at(seconds)` returns positions and a firing-state lead solution without advancing time.
- `preview_launch_shot(x, y, seconds)` previews firing at the specified tick.
- `queue_launch_shot(x, y, seconds)` submits the shared fixed-launch command.
- `preview_timed_shot(x, y, seconds)` returns the solved firing plan or an error.
- `queue_timed_shot(x, y, seconds)` submits the shared command.

Lua uses metres and absolute simulation seconds:

```lua
local x, y = game.aim_at(120)
local shot = game.preview_launch(x, y, 120)
print(shot.launch_seconds, shot.arrival_seconds)
game.fire_at_launch(x, y, 120)
-- Advanced arrival-constrained requests still use game.fire_at(x, y, arrival).
```

`game.forecast` is observation-only for the enemy, even though the separate
`game.state` diagnostic interface exposes true state. Queue entries include
`requested_arrival_seconds` (null for fixed launches), `error`, and firing time.
`game.fire(x, y)` and `game.aim()` retain the Live workflow. Parameter editing
and formal evaluation scores are covered by [step 5](step-5-evaluation.md).

## Verification

Native planning tests cover fixed muzzle speed and target arrival, timing
quantization, atomic invalid requests and reload conflicts, maneuver re-solving,
invalid-plan recovery/cancellation, locked orders, ballistic persistence,
forecast/execution agreement, hidden-enemy-state independence, Lua parity,
command-log replay, and stopping invalid future plans at the next boundary.

The mission browser test exercises readiness markers, reload snapping ahead of
command ticks, cancellation/gaps/full capacity, and the expanded colored scale,
precise keyboard nudges, scale stability during Auto, fixed-time Queue, reload error,
numeric maneuver edit with unchanged firing ticks, immutable reservations,
active-interval locks, Live restoration, and touch sliders in portrait/landscape layouts.
It also verifies Q while dragging, rejection of key repeats/full
requests and successful early firing, ordinary Lua text entry, and actual two-finger Queue taps during Auto.
Second-finger release/cancellation preserves scale ownership and accepted orders.
Existing mission, navigation, Lua, and sandbox regressions remain applicable.
