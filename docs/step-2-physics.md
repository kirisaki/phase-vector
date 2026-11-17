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
