# Step 3: blockade mission

The mission core, shared command API, and browser mission controls are implemented.

## Rules and scope

The blockade has one friendly ship, one enemy that cannot fire, and one unguided
railgun. A hit disables the enemy. Entry of the enemy's center into the goal
circle before or exactly at the deadline is a defeat. Surviving without an entry
until the deadline is a victory. Exhausting delta-v or temporarily leaving the
route never ends the mission. Terminal outcomes freeze playback; retry restores
the initial world, sensors, orders, ammunition cooldown, and decision history.

The mission layer consumes the existing physics core's swept impact events.
Goal entry is also swept within each 0.25-second tick. Earlier events win; an
exact impact/entry tie favors disablement. Entry or impact at the deadline is
resolved before timeout. The reported outcome time includes the tick fraction;
the frozen physical state is at the end of that tick.

## Provisional tuning

| Parameter | Value |
| --- | --- |
| Initial separation, velocities, acceleration, delta-v | Existing 240,000 km / 75 and 37.5 km/s / 5 G / 60 km/s trial |
| Goal center | Enemy's initial X, Y = -45,000 km |
| Goal radius | 1,000 km; entry uses the ship center |
| Deadline | 1,200 s (ten command intervals) |
| No-attack entry time | 1,173.333333 s |
| Relative muzzle speed / projectile radius | 660 km/s / 1 m |
| Reload | 120 s; no ammunition-count limit |
| Sensor samples / processing latency | Every 1 s / 1 s plus rounded-up light travel time |
| Stale projectile track retention | 300 s from its last sample |
| Enemy decision cadence | Every 1 s |
| Enemy threat lookahead | 120 s, independent of projectile flight time |
| Safety corridor | 75 km plus physical contact radii |
| Evasion commitment | 120 s |
| Cruise recovery response | 120 s |

These are game tuning assumptions. In particular the enemy is a conservative
controller, not an optimal pilot. The deadline has little slack; this is a
functional demonstration, not a claim of final difficulty balance.

## Gun and command timing

A shot is a world-space future target point. The gun solves its flight time and
direction at a fixed relative muzzle speed, including the firing ship's current
velocity. The default lead point uses only the delivered contact's extrapolated
position and velocity. It does not read the enemy's future decisions.

`QueueShot` appends at the earliest free player command boundary that satisfies
reload spacing against fired rounds and all reservations. Initial default slots
are 0, 120, and 240 seconds. Reservations have stable IDs and fixed target points.
Cancelling one does not move the others; later additions can fill freed slots.
The deadline bounds the queue (ten initial slots with default tuning).
Future launch predictions repeat the draft burn after the frozen current interval
and account for acceleration and remaining delta-v. Each preview is recalculated
when the maneuver changes; the target and launch tick remain fixed.
Editing during execution cannot fire immediately. Launch occurs at the boundary
before the first acceleration kick. A reservation does not consume a shot until
launch. Shots are one-shot orders and do not repeat with thrust. Cancel affects
only unlaunched reservations (`CancelQueuedShot(id)` for one, `CancelShot` for all). Launched projectiles retain their inertial world
velocity and survive command boundaries. The shot target is recorded beside the
applied thrust vector for deterministic replay.

The current UI/API does not choose an arbitrary arrival time; that remains step 4. There is no blast damage, projectile lifetime,
ship-to-ship collision, guided weapon, or enemy gun.

## Information boundary

`observation::Sensors` is the only adapter that samples the true world. Pending
packets are private. Delivery occurs only after processing plus light travel.
Propagation delay uses the distance at sampling; moving-receiver corrections
are outside this PoC. Samples are tagged with acquisition and receipt ticks.
The initial enemy track comes from the mission briefing. The PoC assumes unobstructed detection of all
friendly projectiles. It does not model noise, occlusion, or passive emissions.

Enemy AI receives only its own ship, goal, cruise speed, delivered projectile
tracks, and its previous decision. Its function cannot access the world or
pending packets. Ballistic tracks are extrapolated from their sample time. Stale
projectile knowledge expires by sample age, not by consulting hidden truth.

Player-facing enemy positions and velocity predictions use the last delivered
contact fix. They assume constant observed velocity, not a privileged forecast.
Camera fitting uses that same presented position. Enemy fuel is not exposed in
the normal view. The Lua REPL intentionally retains privileged true ship state
for diagnostics, with observation timestamps and AI reasons available separately.

## Enemy controller

The controller reevaluates once per simulated second. It normally steers toward
the goal at the original cruise speed, correcting transverse velocity and
returning after an evasion. An approaching observed projectile becomes a threat
when its predicted closest approach enters the safety corridor within the
reaction window. Evasion accelerates perpendicular to relative projectile
velocity, choosing the side farther from the predicted crossing. A symmetric
approach uses a deterministic side. The controller holds the evasion for the
configured commitment, then resumes recovery or reacts to another observation.
All thrust uses the same acceleration and delta-v limits as the player.

Enemy autonomy can react inside a player command interval. It cannot receive
player draft orders or observe reservations before launch. No random choices or
hidden script forces an outcome.

## Verification and limitations

Native tests cover no-attack breakthrough, zero fuel, off-route timeout, swept
entry, hit/entry order and ties, the exact deadline, gun speed inheritance,
reservation/cooldown validation, projectile persistence, observation delivery,
no hidden-state presentation, evasion/recovery, terminal locks, retry, Lua/API
agreement, and full mission replay.

`cargo run --example blockade_trials` compares no attack with seven observed-lead
shots and seven shots placed 10,000 km away in Y. With the current tuning, the
lead pattern wins by timeout without hits; the displaced pattern loses by
breakthrough. Both fire the same number of shots and the enemy still has fuel.
The demonstration validates placement-dependent delay, not optimal tactics.

`scripts/mission_smoke.py` plays the default mission in Chromium, including a
no-attack defeat and the seven-shot non-hit victory. It exercises mouse/touch
aiming, queue/cancel, running edits, launch boundaries, frozen outcomes, and retry.
The browser renders the default blockade; `?sandbox=1` retains the original
sandbox for `scripts/browser_smoke.py` regression checks.

The Aim crosshair edits a world-space point without changing the reserved shot.
Queue explicitly appends it; Track lead restores observation-based aiming.
A reserved point is fixed until cancelled, while its direction/flight preview is
recomputed from the predicted launch state. White trails show flying rounds;
amber dots indicate a constant-observed-velocity estimate, not the AI's future.

Formal score weighting and general paused parameter editing remain step 5.
This milestone reports outcome, elapsed time, shot count, and friendly delta-v.
