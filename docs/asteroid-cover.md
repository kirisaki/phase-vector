# Asteroid cover trial

The blockade now contains two moving, indestructible circular asteroids. This
extends the original open-space mission: a firing-position change can open a
ballistic corridor, but positioning alone does not guarantee denial.

## Model and assumptions

- Asteroids have an ID, position, velocity, and positive physical radius. They
  drift without gravity, thrust, deflection, damage, fragmentation, or debris.
- Both sides know the asteroid ephemeris from the briefing. Terrain blocks
  projectiles, not sensors. Enemy ship tracks and projectile observations retain
  their existing sampling, light delay, processing, and expiration rules.
- Any projectile owner uses the same collision rule. The current enemy still has
  no weapon; friendly protection is verified with rounds owned by either side in
  native tests, without adding enemy fire to the mission.
- Ship contact disables propulsion and weapons. A disabled hull retains its
  velocity in the physics model; the mission freezes at its terminal tick.
  Friendly contact is defeat (`own_disabled`); enemy contact is a disabling win.
- The fixed timestep remains 0.25 seconds. Each ship receives the usual bounded
  velocity kick, then drifts linearly. Swept relative circle contacts cover the
  entire segment, including moving asteroids and initial overlap. No sub-tick
  acceleration or bounce is modeled.
- A projectile is consumed at its earliest contact. Terrain wins an exact
  terrain/ship tie. Terrain contact records sort by fraction, asteroid ID, ship
  ID, then projectile ID. Terminal events use their sub-tick time: earlier wins;
  own disablement wins an exact terminal tie; enemy disablement beats simultaneous
  goal entry. Contact or entry at the deadline precedes timeout. World positions
  and delta-v accounting still finish the event tick, as in the original model.

## Provisional setup

Standard asteroid values live in `scenario::default_asteroid()` (A1) and
`scenario::enemy_cover()` (A2). `scenario::blockade_world()` adds both fixtures; `scenario::initial_world()` keeps
an empty-terrain sandbox and the historical open-space test fixture.

| Property | A1: own-side cover | A2: enemy-side cover |
| --- | --- | --- |
| Initial center | (-100,000, -3,100) km | (+100,000, -3,100) km |
| Velocity | (0, +65.625) km/s | (0, -28.125) km/s |
| Physical radius | 450 km | 150 km |
| Initial axis distance from the nearer ship | 20,000 km | 20,000 km |

A2 is near the enemy-side center of the firing corridor, not centered on the
enemy hull or its unopposed route. Both are known inertial bodies. A1 remains
unchanged from the first cover trial. Its placement near coasting lead-shot
crossings means that waiting alone does not clear early shots. A2 adds a smaller
second obstruction; clearing A1 no longer guarantees a clear complete shot.

The enemy threat horizon is shortened from 120 to **90 seconds**, independently
of the 120-second command interval and projectile flight time. This changes how
far ahead it evaluates observed threats, not a delay after seeing a round. A
shorter horizon can make denial harder: the enemy may spend less time detouring.
The deadline, muzzle speed, acceleration, safety corridor, and scoring weights
are unchanged. All values remain provisional.

For experiments, construct an `Asteroid::new(id, position, velocity, radius)` and
replace `world.asteroids` before `Session::with_mission(world, 1, config)`.
Validation rejects nonfinite/out-of-bounds coordinates and nonpositive radii.
Custom initial worlds are labeled Custom and Retry restores their complete
initial terrain. Asteroid parameters are not mutable through the Lua parameter
editor yet; its existing seven bounded settings are unchanged. No map editor or
second terrain type is introduced.

## Shared prediction and AI

`physics::asteroid_contact` is the common swept query for execution, ballistic
previews, and observed threat screening. The own-ship timeline and burn-route
forecast use the normal bounded burn integration with known-terrain contacts.
After a predicted disabling contact, they cease applying thrust. Future shots
from a predicted disabled ship are rejected.

A ballistic `Solution` includes an optional `obstruction`: asteroid ID, absolute
simulation seconds, and round contact position. Blocked solutions remain
queueable deliberately: the preview warns rather than silently choosing another
target or launch time. Maneuvers recompute editable reservations through the
existing API while keeping fixed targets and firing ticks. Already flying rounds
retain their velocity and launch-time cover prediction. Arrival-constrained Lua
reservations retain their existing timing tolerance and replanning rules.

The enemy considers only delivered projectile tracks. For each track, it uses
known terrain to truncate the predicted round lifetime at its first cover
contact. That calculation starts at the observation's sample time, so stale
tracks do not become ghost rounds on the far side of cover. Known asteroids also
enter candidate-burn clearance checks, allowing the controller to avoid them.
Its existing finite-horizon, low-cost evasion search is still approximate; this
is not a global path planner or an AI that optimizes hiding behind cover.

## Interface and diagnostics

- Brown **A1** and **A2** show live physical circular silhouettes and small
  symbolic center crosses. Each dotted track uses the same forecast horizon
  as the ship routes. Each label states physical radius in kilometres.
- Moving the firing-time slider adds a dashed circle for each asteroid at that time.
  It does not move the world. Ship and round forecasts use the same selected time;
  rounds predicted to have hit terrain are absent from the future round markers.
- **Blocked by A1/A2** in weapon feedback and a red cross on the map identify the
  predicted round contact and absolute time. The preview line stops at cover.
  Queued rows and target labels also mark blocked shots. Fine aim retains its
  enemy estimate but reports the obstruction in its heading.
- A predicted own-ship collision is marked on the route. The selected-time label
  shows `COLLISION` when the selected future state is past that contact.
- Actual cover contact removes the round and its remaining path/number. History
  still records that the shot was fired. Retry restores terrain and clears contacts.
- Help includes the cover rule and a suggested maneuver-and-fire experiment.

Lua uses the same forecast and planning data:

```lua
return game.state().asteroids[1] -- id, x, y, vx, vy, radius (SI units)
return game.forecast(240).asteroids[1]
local x, y = game.aim_at(240)
local shot = game.preview_launch(x, y, 240)
return shot.blocked_by, shot.blocked_seconds -- nil when clear
return game.state().asteroid_impact_count, game.state().ship.disabled
```

## Tactical acceptance evidence

Run `cargo run --locked --example asteroid_trials`. Each salvo uses four observed
lead points separated vertically by the listed spacing, at the earliest free
30-second firing slots. Burns end before the first salvo; subsequent motion
coasts. The enemy receives no privileged future orders.

Current two-asteroid / 90-second-horizon examples (12 shots, zero ship hits):

| Strategy | First salvo | Absorbed | Outcome | Own delta-v | Score |
| --- | --- | --- | --- | --- | --- |
| Coast, +200 km spacing | 240 s | 12 by A1 | Breakthrough at 1,173.333 s | 0 km/s | 0 |
| Slow at 5 G for 240 s, repeated lead | 240 s | 0 | Breakthrough at 1,201.630 s | 11.768 km/s | 0 |
| Slow at 5 G for 240 s, +200 km spacing | 240 s | 4 by A2 | Timeout at 1,215 s | 11.768 km/s | 1,661 |
| Slow at 5 G for 240 s, +900 km spacing | 240 s | 0 | Breakthrough at 1,183.553 s | 11.768 km/s | 0 |

No attack still produces a breakthrough at 1,173.333 s, with no terrain collision.
The successful equal-budget strategy needs both a firing-position change and
shot placement. Not every unobstructed shot contributes useful delay.

Use `cargo run --locked --example asteroid_trials -- --compare` to separate the
layout and horizon changes. The comparison includes the old one-asteroid /
120-second baseline, then two asteroids with 120, 110, 105, 90, and 60 seconds:

| Same strategy | One asteroid, 120 s | Two asteroids, 120 s | Two asteroids, 90 s | Two asteroids, 60 s |
| --- | --- | --- | --- | --- |
| Slow 240 s, +900 km spacing | Timeout | Timeout | Breakthrough | Breakthrough |
| Slow 360 s, +900 km spacing | Timeout | Breakthrough (2 rounds absorbed) | Breakthrough | Breakthrough |
| Slow 240 s, +200 km spacing | Timeout | Timeout (4 rounds absorbed) | Timeout | Breakthrough at 1,213.741 s |

`cargo run --locked --example cover_balance_sweep` samples 39 fixed placements
per horizon: 13 spacings crossed with three starting offsets, after the same
240-second slowdown. Two samples win at 90 seconds, none at 60, and four at 110.
The second 90-second winning sample starts 600 km below lead and spaces shots
400 km apart. These are deterministic examples, **not player win rates** or proof
that other placements fail. The current 90-second choice offers a meaningfully
shorter reaction horizon with verified non-hit wins; it is not claimed to be
easier than the previous balance. Physical playtesting should guide the next
adjustment.

`tests/asteroids.rs` covers moving-object sweeps, temporal misses, cover for both
owners, contact priority, disabled propulsion, terminal ordering, prediction versus
execution, reservation recalculation, AI cover/avoidance, stale observations, and
exact command replay. Existing open-space tests retain explicit terrain-free
fixtures where they test the old balance or unobstructed flight.

`scripts/asteroid_checks.py`, called by the mission browser suite, checks actual
map gestures, timeline ephemerides, fixed reservation recalculation, disappeared
round trails, a predicted and executed own-ship collision, Retry, and
desktop/phone-sized layouts. The mission suite performs
the 240-second slowdown through the map controls and then wins with 12 rounds at 200 km spacing through Queue and Fine aim. Its expected score includes maneuver fuel.
The physics sandbox browser suite continues to cover the original UI, Lua, Help,
playback, and navigation behavior. Physical phone and Safari validation remain
outside the automated Chromium evidence.
