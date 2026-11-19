# Provisional combat scale

## Adopted baseline

This trial adopts the user-provided 240,000 km engagement and 660 km/s railgun.
It supersedes the earlier near-light-speed trial. Constants live in
`src/scenario.rs`; distances and speeds are stored in SI units.

| Quantity | Baseline |
| --- | --- |
| Initial ship separation | 240,000 km |
| Friendly velocity | +75 km/s along Y |
| Passive contact velocity | −37.5 km/s along Y |
| Maximum acceleration, each ship | 5 G = 49.03325 m/s² |
| Initial delta-v, each ship | 60 km/s |
| Reference railgun muzzle speed | 660 km/s |
| Reference flight time | 240,000 / 660 = 363.636363… s |
| Ship collision radius | 0.5 km, retained from the previous trial |
| WEGO interval / physics tick | 120 s / 0.25 s |

G means standard gravity, 9.80665 m/s². Initial headings are retained: the ships
start at opposite X coordinates and drift in opposite Y directions. Acceleration
and delta-v limits apply to both ships, although the contact still coasts without
AI. The separation and muzzle speed are the independent parameters; reference
flight time is derived from their ratio, not rounded to 364 seconds internally.
This is a stationary-target reference, not a solved intercept for moving ships.

## Maneuver budget

A full 120-second burn changes velocity by 5.88399 km/s and leaves 54.11601 km/s
of delta-v. Ten full burns leave 1.1601 km/s. The budget supports about 1,223.66 s
of maximum thrust (10.197 intervals); it runs out partway through the eleventh
interval. The fixed-step model clips the final velocity increment and continues
coasting rather than removing the existing velocity.

For the first lateral burn from the initial state, the semi-implicit integrator
produces 353.77489875 km of lateral displacement relative to coasting and 9,000 km
of longitudinal drift. The heading changes by about 4.486 degrees. These are
provisional gameplay values; mission and evasion balance have not been validated.

## Travel time and observed warning

The speed of light remains 299,792.458 km/s. A signal crosses the initial range
in about 0.800554 s. For a stationary launcher and target, an immediately detected
launch flash would therefore precede impact by about 362.83581 s. Actual warning
will depend on geometry, visibility, detection, and processing delay.

The original 240–480 s flight-time tuning band corresponds to ranges of
158,400–316,800 km at 660 km/s. Reaction lead time must be tuned separately;
this scale does not retain the previous near-c weapon's short warning window.
No light-delay model or observation-limited AI exists in the current sandbox.
Both displayed ships are authoritative diagnostic state. Sensor rules, evasion,
and return to the mission route remain step 3.

## Reference frames and numerical model

Simulation storage and Lua use metres, m/s, m/s², and seconds. The UI displays
positions in km and velocities/delta-v in km/s. Numeric acceleration inputs use
m/s². Simulation time is the shared mission frame's coordinate time. Ships use
Newtonian kick/drift integration; projectiles drift along straight world-frame
trajectories. Gravity, relativistic acceleration, and proper-time effects are
not modeled.

`World::launch` accepts a world-frame velocity, not a muzzle velocity. The current
flight test uses a stationary firing frame. Adding the firing ship's velocity
and solving moving-target interception belong to the future firing UI and shot
solver. The sandbox has no firing controls, reload mechanic, or hit consequences.

A 660 km/s projectile travels 165 km per tick, far beyond a ship's 0.5 km radius.
Swept relative-motion contacts therefore remain necessary. Regression tests also
retain metre-scale contacts at the earlier astronomical ranges and near-c speeds;
those are numerical stress fixtures, not current gameplay settings. The flight
test verifies that the reference projectile survives three 120-second intervals
and hits during the fourth.

## Map scale

Overview centers between the ships and automatically fits their separation to
the current viewport. An elliptical fit leaves 35% margin on each half-axis.
Scale eases logarithmically with a 0.35-second time constant in real frame time;
a 10% margin guard immediately zooms out after a large jump or resize. The
existing scale limits still apply. Automatic updates pause during burn dragging.
Manual zoom suspends auto fitting; selecting Overview restores it. Own ship and
Contact follow the chosen ship at a fixed scale, initially 100,000 m per render
unit. These camera operations change presentation only. The ship-centered burn control
retains the current camera and maps handle offsets to the feasible maneuver.
The control ring is not a physical destination region at map scale.

The renderer subtracts the camera center in `f64`, divides by the map scale, and
only then converts to `f32`. The grid uses labeled 1/2/5 decade spacing in km.
Ship symbols and velocity arrows are enlarged; physical radius rings appear only
when large enough to see. At manual zoom, a ship can leave the viewport after long execution;
select Overview to restore automatic fitting.
