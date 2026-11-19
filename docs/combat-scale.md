# Near-light-speed combat scale

## Adopted baseline

The railgun is a fictional near-light-speed weapon. Until a more specific fraction
is requested, its reference muzzle speed is **0.90c**. This is a tuning assumption,
not a claim about present-day railguns. Constants live in `src/scenario.rs`.

| Quantity | Baseline |
| --- | --- |
| Speed of light | 299,792,458 m/s exactly |
| Reference railgun muzzle speed | 269,813.2122 km/s (0.90c) |
| Reference flight time | 360 s; tuning band 240–480 s |
| Initial ship separation | 97,132,756.392 km (324 light-seconds) |
| Friendly velocity | +29,979.2458 km/s along Y (0.10c) |
| Passive contact velocity | −14,989.6229 km/s along Y (0.05c) |
| Ship collision radius | 0.5 km |
| Maximum acceleration | 1,000 m/s² |
| Initial delta-v, each ship | 1,200 km/s |
| WEGO interval / physics tick | 120 s / 0.25 s |

The light-speed constant follows [NIST](https://physics.nist.gov/cuu/Constants/Value/c.html).
Acceleration and delta-v assume fictional propulsion and tolerance for sustained
high acceleration (about 100 g at full thrust). They are gameplay values to tune,
not realistic crew limits. Crewed ships would require fictional acceleration
protection. Speeds are relative to the shared mission frame, not absolute speeds.
The current speed trial uses 0.10c/0.05c while keeping acceleration at 1,000 m/s²,
delta-v at 1,200 km/s, and the reference engagement range and flight time fixed.
The ships start at opposite X coordinates, with transverse velocities. Their
separation is set by the stationary-target reference shot; it is not a solved
intercept between the moving ships.

A maximum burn spends 120 km/s per command interval. Ten uninterrupted intervals
consume the 1,200 km/s budget; coasting spends none. The semi-implicit integrator
produces 7,215 km of lateral displacement in the first full interval from zero
lateral velocity, plus 3,597,509.496 km of friendly longitudinal drift. This drift
is about 3.7% of the initial engagement range per interval.

## Travel time and observed warning

For a stationary launcher and target separated by D, use `flight = D / (0.9c)`
and `light_delay = D / c`. If a launch flash is the earliest detectable signal,
the available warning is `flight - light_delay`:

| Flight | Range (km, rounded) | Light delay | Warning after seeing launch |
| --- | --- | --- | --- |
| 240 s | 64,755,171 | 216 s | 24 s |
| 360 s | 97,132,756 | 324 s | 36 s |
| 480 s | 129,510,342 | 432 s | 48 s |

These are reference calculations. Actual warning also depends on geometry,
visibility, detection, and processing delay; it may be shorter. At the 360 s
reference range, 36 s at 1,000 m/s² permits approximately 648 km of transverse
motion from rest (continuous-acceleration estimate). Thus a small target can
still evade an accurately placed unguided shot. Evasion balance is not yet proven.
A different near-c fraction changes both the range and the warning: at 0.99c,
the warning for a 360 s shot would be only 3.6 s.

The step-3 observation model must account for signal propagation before adding
sensor/decision delays. It must not give the enemy the entire projectile flight
time as a reaction window or expose unobserved shots. No light-delay model or
observation-limited AI exists in the current sandbox; both displayed ships are
authoritative diagnostic state. This remains explicit future work.

## Reference frames and numerical model

Simulation storage and Lua values remain SI: m, m/s, m/s², and seconds. Display
positions use km, velocity and delta-v use km/s, and acceleration remains m/s².
Simulation time is the shared mission frame's coordinate time. Ships use the
existing nonrelativistic kick/drift approximation; ballistic projectiles drift
along a straight world-frame trajectory. At 0.10c the Lorentz factor is about
1.005, so relativistic effects are no longer assumed negligible for all purposes.
This is a speed and presentation trial, not a relativistic propulsion simulation:
thrust is world-frame coordinate acceleration, delta-v is summed coordinate
velocity change, and proper acceleration and time dilation are not modeled.

`World::launch` accepts a world-frame velocity, not a muzzle velocity. The new
near-c tests use a stationary firing frame. Once the weapon and shot solver are
implemented, transform the 0.90c muzzle velocity from the ship frame using
relativistic velocity addition; ordinary vector addition is not the intended
near-c firing rule. See [OpenStax's velocity-addition explanation](https://openstax.org/books/college-physics-ap-courses/pages/28-4-relativistic-addition-of-velocities).
Relativistic energy, recoil, proper-time clocks, and detailed damage remain
outside this kinematic PoC. Weapon reservations, firing controls, and the
intercept solver are still later milestones.

Contacts use relative closest approach and a chord intersection. This avoids
subtracting enormous nearly equal squared quantities in a quadratic discriminant.
Tests distinguish a 500 m hit from a 501 m miss over an astronomical segment,
including a large translated origin. A real 0.90c projectile travels 67,453.3 km
per physics tick, so endpoint-only collision checks would be unusable.

## Map scale

Overview frames the initial engagement. Own ship and Contact use a local frame
that follows the chosen ship, initially 100,000 m per render unit. Plus/minus zoom
changes only presentation. Each preset resets its zoom; neither focus nor zoom
changes commands, simulation time, or delta-v. At extreme zoom-out or after long
unbounded execution, a ship can leave the viewport; the focus presets recover it.

The renderer subtracts the camera center in `f64`, divides by the map scale, and
only then converts to `f32`. This retains local detail at astronomical coordinates.
The grid is anchored to world coordinates with 1/2/5 decade spacing, labeled in
km. Ship symbols and velocity-direction arrows are schematic and enlarged;
physical radius circles appear only when large enough to see. Symbol overlap
is not a collision. The large overview intentionally makes short ship maneuvers
look tiny; use a local view to inspect them rather than altering the physics.
