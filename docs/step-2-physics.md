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

The session begins paused at a command boundary. Play applies the draft and
starts simultaneous execution. The current vector is frozen for 480 physical
ticks. Playback continues across boundaries and repeats the latest draft.
Drafts can be edited while running or paused; they never modify the active
interval. A changed vector replaces the next order and then repeats until
changed again. The passive second ship coasts.

`play`, `resume`, and the compatibility command `commit` start or resume playback.
Repeated calls during execution do not restart the interval or duplicate its
log record. At a paused boundary, the next Play or step applies the current draft.

The UI's +10 s action (`step_forward`) runs up to 40 fixed ticks while paused,
clipped to the current interval's end. The Lua `step` command advances one tick.
Both can start a new interval without unpausing. Next turn (`next_turn`) finishes
the current interval, or starts one at a boundary, and always stops paused at its
end. These actions process all physical ticks, including contacts and delta-v.

The browser supplies 60 fixed presentation pulses per second. An integer
accumulator converts these to 0.25-second physical steps. Playback speeds are
1/32/64/128 (default 32). Fractional time is preserved across command boundaries;
a single pulse can finish one interval and start another. Pause discards only
a fraction of an unexecuted playback tick. Position, velocity, delta-v,
projectiles, impact history, and the queued vector persist across boundaries.

Each committed vector and start tick is recorded. Replaying the log directly
through the physics core reproduces the world without any presentation clock.
This is a same-runtime guarantee. The mission's ten-opportunity target, richer
maneuver timelines, and enemy decisions remain later milestones.

## Friendly endpoint planning

The current GUI plans one constant thrust vector by specifying an absolute
position at the end of the next complete 120-second order interval. This is an early step-4 subset; it does not implement
weapons, enemy forecasts, variable arrival times, or intermediate waypoints.

`maneuver::preview` clones the friendly ship and uses the same kick/drift routine
as execution, including partial fuel exhaustion. It samples the route every two
seconds and reports final velocity and delta-v use. Previews do not mutate the world or evaluate mission
outcomes. Enemy state is not part of the friendly prediction.

The inverse solver measures the requested endpoint relative to the coast
endpoint. Isotropic thrust bounds make the reachable endpoints a disk. Its
radius is the displacement from maximum thrust with the actual fuel budget.
The solver evaluates this displacement in a local stationary frame and bisects
acceleration magnitude along the requested offset direction. It rejects targets
outside the disk (apart from a 1 mm numerical tolerance) and non-finite inputs. A target within 1 mm of the coast endpoint selects
zero thrust. Tests compare the preview and execution exactly and require solved
endpoints to agree with requested positions within 1 cm at the current scale.

`Command::PlanArrival`, the browser, and Lua `game.arrive(x, y)` share this solver.
During a drag the GUI previews a temporary candidate; releasing the pointer
queues it immediately without changing the active order or playback. Pointer positions outside the
reachable disk are projected radially onto its boundary. The command API itself
still rejects unreachable targets. Escape, pointer cancellation, or focus loss
leaves the previously applied plan and numeric input unchanged. Numeric thrust
remains available in a collapsed section and retains its magnitude-clamping
behavior. Running and paused edits share the same command API.

The arrival handle is available during playback and pause, directly in Overview or
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
Play from pause, stepping, and Next turn are disabled during a drag; Pause is
still available while running. Movement inside an interval does not cancel a
gesture. At a command boundary the chosen direction and reachable-disk fraction
are rebased onto the next interval. An external order change cancels a stale
gesture. Releasing always uses the current boundary state.

`Snapshot::planning_ship` predicts the friendly state at the next boundary using
the frozen active vector and the same fixed-step integration. Arrival solving
and the next interval's reachable disk use this predicted state, including its
remaining fuel. The editing preview prefixes the active remainder to the queued
120-second trajectory. A separate forecast continues to the next three command
boundaries, using the active remainder first and repeating the queued vector
afterward. Its points and boundary positions, velocities, and remaining delta-v
use the same fixed-step integration, including fuel exhaustion. The SVG overlay
draws this forecast as cyan dots and labels the relative time at each boundary.
Delta-v cost refers to the queued interval; arrival velocity and remaining fuel
include both portions. A repeated order keeps its
acceleration vector, not its absolute target position.

The browser observes new command-log start ticks and expands a pale ring around
the actual friendly position. The 0.9-second animation uses presentation time,
so it remains readable at every playback speed. It signals applied orders even
when the vector is zero or fuel is exhausted; it is not a thrust effect. It does
not modify simulation time, physics, or command timing.

Tests compare running edits and repeated playback with fixed-step replay,
including command boundaries, partial fuel exhaustion, and predicted arrivals.
Browser checks exercise running drags, boundary pulses, pause, stepping, touch,
and stable canvas dimensions across playback states.
