# Step 2: physics and execution time

## Physical model

Use metres, seconds, m/s, and m/s². The fixed step is 0.25 seconds; a 120-second
command interval consists of exactly 480 integer ticks. Time is derived from the
tick index rather than accumulated floating-point time. Positions, velocities,
and delta-v use `f64`; conversion to Bevy's `f32` occurs only when drawing.

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

Ships and projectiles are circles. Solve the relative-motion quadratic for the
earliest time of contact in each tick. This catches fast projectiles and moving
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
steps while paused, and 1×/4×/16×/64× playback are presentation controls. Reset is
an explicit diagnostic restart, not an in-mission order change.

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
