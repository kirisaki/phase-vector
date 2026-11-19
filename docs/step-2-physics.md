# Step 2: physics and execution time

## Physical model

Use metres, seconds, m/s, and m/s². The fixed step is 0.25 seconds; a 120-second
command interval consists of exactly 480 integer ticks. Time is derived from the
tick index rather than accumulated floating-point time. Positions, velocities,
and delta-v use `f64`; the renderer subtracts a camera origin and rescales in `f64` before converting
to Bevy's `f32`. Current scenario values are documented in
[the combat-scale revision](combat-scale.md).

There is no gravity or drag. Each ship receives an acceleration vector whose
magnitude is capped by its acceleration limit. Delta-v consumption is the
magnitude of the requested velocity change, including braking. The last burn is
clipped to remaining delta-v. Empty tanks do not remove existing velocity.

Integration is semi-implicit Euler: apply the bounded velocity increment at the
start of each tick, then drift at that velocity for the full tick. Exhaustion
clips that tick's velocity increment; this model does not resolve a sub-tick
engine cutoff or a parabolic path. The chosen discrete motion is piecewise
linear, and contacts are continuous along those segments.

## Contacts

Ships and projectiles are circles. Solve the relative-motion closest approach and chord entry for the
earliest time of contact in each tick, avoiding a cancellation-prone quadratic
discriminant at astronomical distances. This catches fast projectiles and moving
targets, and rejects paths that cross at different times. Tangency, starting
overlap, and contact exactly at the tick end count as contact. The projectile's
owner is excluded. A projectile is consumed by its earliest contact; ties use
the target's numeric ID. Events are ordered by contact time, projectile ID, then
target ID. All ship endpoints come from the same starting tick.

The physics layer emits impact events and does not disable ships or decide a
winner. Mission rules are step 3. There is no projectile expiry or map-edge
culling, and ships do not collide with one another in this PoC.

## Reproducibility

The physics module has no rendering, wall-clock, randomness, or Lua dependencies.
Identical initial state and ordered commands produce the same sequence on the
same build/runtime. Cross-architecture or cross-browser bit-identical floating
point results are not claimed. Native tests compare complete worlds and impact
histories. Input validation rejects non-finite and extreme values before changing
state; trusted Rust callers remain responsible for not modifying public state
fields to violate constructor invariants.

## WEGO and playback

Planning is unlimited and never advances physics. One constant friendly thrust
vector is drafted for the next interval; the passive second ship coasts. Commit
freezes that vector and starts simultaneous 120-second execution. Commands cannot
be edited during execution, including while paused. Pause/resume, single physical
steps while paused, and 1×/32×/64×/128× playback (default 32×) are presentation controls. Reset is
an explicit diagnostic restart, not an in-mission order change.

The UI's +10 s action (`step_forward`) runs up to 40 fixed ticks while paused,
clipped to the current interval's end. It stays paused and never enters the next
execution interval. The diagnostic Lua `step` command still advances one tick.

Next turn (`next_turn`) is available during planning and execution, both running
and paused. In planning it commits the current draft, records the order, and runs
all 480 ticks. In execution it finishes only the remaining ticks with the frozen
orders. Both paths stop at the next planning boundary, without entering another
interval. Playback speed and normal impact, projectile, and delta-v processing
are preserved. The UI disables Next turn during a drag or unapplied numeric edits,
just like Commit; Lua uses the same command API with its already-applied draft.

The browser supplies 60 fixed presentation pulses per second. An integer
accumulator converts those pulses into whole 0.25-second physical steps; speed
only changes the number of steps executed. Every speed stops exactly at the
command boundary. Pausing discards a fraction of one unexecuted playback tick,
never physical progress. Catch-up pulses or steps cannot spill into planning.
The next interval defaults to coasting, while position, velocity, delta-v,
projectiles, and impact history persist.

Each committed vector and start tick is recorded. Replaying the log directly
through the physics core reproduces the world without any presentation clock.
This is a same-runtime guarantee. The mission's ten-opportunity target, richer
maneuver timelines, and enemy decisions remain later milestones.

## Friendly endpoint planning

The current GUI plans one constant thrust vector by specifying an absolute
position 120 seconds ahead. This is an early step-4 subset; it does not implement
weapons, enemy forecasts, variable arrival times, or intermediate waypoints.

`maneuver::preview` clones the friendly ship and uses the same kick/drift routine
as execution, including partial fuel exhaustion. It samples the route every two
seconds, reports final velocity and delta-v use, and draws another 30 seconds of
coasting after the burn. Previews do not mutate the world or evaluate mission
outcomes. Enemy state is not part of the friendly prediction.

The inverse solver measures the requested endpoint relative to the coast
endpoint. Isotropic thrust bounds make the reachable endpoints a disk. Its
radius is the displacement from maximum thrust with the actual fuel budget.
The solver evaluates this displacement in a local stationary frame and bisects
acceleration magnitude along the requested offset direction. It rejects targets
outside the disk (apart from a 1 mm numerical tolerance), non-finite inputs, and
commands outside planning. A target within 1 mm of the coast endpoint selects
zero thrust. Tests compare the preview and execution exactly and require solved
endpoints to agree with requested positions within 1 cm at the current scale.

`Command::PlanArrival`, the browser, and Lua `game.arrive(x, y)` share this solver.
During a drag the GUI previews a temporary candidate; releasing the pointer
applies it immediately without advancing physics. Pointer positions outside the
reachable disk are projected radially onto its boundary. The command API itself
still rejects unreachable targets. Escape, pointer cancellation, or focus loss
leaves the previously applied plan and numeric input unchanged. Numeric thrust
remains available in a collapsed section and retains its magnitude-clamping
behavior. Planning edits remain prohibited during paused execution.

The arrival handle is always available during planning, directly in Overview or
the selected map view. There is no route mode, confirmation button, or panel
transition. Dragging, application, and cancellation preserve map focus and hold zoom
during the gesture. Overview automatically fits both ships as they move; manual
zoom suspends this until Overview is selected again.

The SVG layer uses the same origin, metres per render unit, and AutoMin(2000,
1200) projection as Bevy for physical trajectories and arrival annotations.
The burn control is centered on the ship's current position. Its radius fits
available screen space up to 90 pixels. Handle offsets map to the reachable
maneuver displacement relative to coasting, and the inverse solver supplies the
constant thrust. The ring is a control range, not an arrival region at map scale.
There is no filled or outlined circle at the physical arrival point. An amber
arrow from the ship indicates the burn, separately from the cyan velocity arrow.

Candidate trajectories replace the applied preview during a drag. Pointer
capture supports mouse/touch dragging outside the circle or canvas. Keyboard
arrows immediately apply adjustments, Shift makes them finer, and Home plans
coasting. Telemetry and preview metrics remain in the normal flight controls.
Commit is disabled during an active drag; releasing applies the draft and enables
Commit. Changing the world or applied order through Lua cancels a stale gesture.

During execution, `preview_ticks` uses the active burn and the remaining ticks
until the command boundary. The remaining trajectory, arrival annotation, burn
arrow, and remaining delta-v cost stay visible with editing locked. Tests check
that partial execution preserves the predicted endpoint, including fuel exhaustion.
A browser test drags the handle, clicks Commit, and compares the final physical
position, velocity, and cost to the plan. The next interval defaults to coasting;
it does not discard the velocity acquired during execution.
