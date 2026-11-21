# Step 3: blockade mission

Step 3 is complete for the PoC scope: the mission core, shared command API,
observation-limited enemy, and browser mission controls are implemented and
verified. The acceptance coverage below distinguishes native simulation checks
from real browser interaction. Difficulty tuning remains provisional.

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
| Reload | 30 s; no ammunition-count limit |
| Sensor samples / processing latency | Every 1 s / 1 s plus rounded-up light travel time |
| Stale projectile track retention | 300 s from its last sample |
| Enemy decision cadence | Every 1 s |
| Enemy threat lookahead | 120 s, independent of projectile flight time |
| Safety corridor | 1,000 km plus physical contact radii |
| Evasion commitment | 120 s |
| Cruise recovery response | 120 s |

These are game tuning assumptions. In particular the enemy is a conservative
controller, not an optimal pilot. The deadline has little slack; this is a
functional demonstration, not a claim of final difficulty balance.

The warning corridor was widened from 75 km to 1,000 km so modest targeting
offsets can provoke evasion at the overview scale. Physical hit radii and the
120-second threat horizon are unchanged. Reaction is not monotonic with corridor
width: a different avoidance direction can help the enemy recover its schedule.
The current seven-shot lead trial still prevents breakthrough without a hit;
the displaced seven-shot trial and the no-attack trial still lose.

## Gun and command timing

A shot is a world-space future target point. The gun solves its flight time and
direction at a fixed relative muzzle speed, including the firing ship's current
velocity. The default lead point uses only the delivered contact's extrapolated
position and velocity. It does not read the enemy's future decisions.

`QueueShot` appends one round at the earliest reload-compatible tick at or after
the next editable player command boundary. The button and target-placement UX
are unchanged. Four initial additions fire at 0, 30, 60, and 90 seconds; the
command interval remains 120 seconds. Reload spacing applies against fired rounds
and every reservation. IDs and target points are stable. Cancelling an editable
shot does not move others; later additions can fill freed slots. The deadline
bounds firing times. The queue holds at most four unfired reservations, including
locked shots in the active interval. Flying rounds do not consume queue capacity.
Launching or cancelling a reservation frees one slot without moving other shots.
The mission API enforces this limit for both UI and Lua commands. Capacity and
the full-queue error are exposed to the normal UI; Lua reports `queue_capacity`.

Starting an interval validates and freezes every shot scheduled inside it.
Current-interval shots cannot be cancelled, even while paused. Clear queue removes
only shots at or after the next editable boundary. Running edits can add shots
only for that boundary or later. Future launch predictions use the active burn
for the remainder of the current interval and the repeating draft thereafter,
including acceleration and delta-v limits. Maneuver edits recalculate previews,
without changing reserved targets or launch ticks.

Every physical tick checks due launches before its acceleration kick and sensor
sample. A reservation counts as a shot only when launched; shots do not repeat
with thrust. In-flight projectiles retain inertial velocity across intervals.
The command log records the complete frozen shot list (targets and launch ticks)
alongside the applied thrust vector, including gaps left by cancellation, for
deterministic replay. Terminal outcomes stop pending launches.

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
The enemy's directional ship symbol follows observed velocity, not hull attitude.
Its speed readout and bright acceleration arrow use the two latest delivered
fixes, with acquisition times determining the average acceleration. The label
distinguishes observed slowing, speeding up, turning, and coasting; it does not
claim knowledge of an AI evasion decision. No acceleration estimate is available
until fixes with distinct acquisition times arrive. Terminal displays retain the
last observed speed and hide the acceleration arrow.
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

### Acceptance coverage

| Requirement | Evidence |
| --- | --- |
| One non-firing enemy, unlimited gun ammunition, reload, disabling hits | `tests/mission.rs`: inherited muzzle velocity, reload reservations, active salvos, and an actual queued hit through the session API |
| Goal entry, disablement, timeout, and exact deadline ordering | Swept goal/impact tests include entry and impact exactly at the deadline and a contact after it |
| Fuel exhaustion or temporary deviation is not a win | Zero-fuel breakthrough and off-route tests keep the mission running until an actual terminal condition |
| Only observed attacks can influence the enemy | Unlaunched reservations leave world state, delivered observations, and AI decisions identical to an unarmed run; hidden projectiles cannot trigger evasion before delivery |
| Delayed, detached observations | `tests/observation.rs`: processing plus quantized light delay, older packets arriving after newer fixes, and stale projectile knowledge surviving hidden removal until sample-age expiry |
| Evasion and return toward the mission route | AI decision tests plus a full oblique-shot run that measures lateral displacement growing and then shrinking during recovery under the normal acceleration and delta-v limits |
| Terminal state and retry | A queued disabling hit freezes the session and pending launches; retry restores the initial world, sensors, queue, cooldown, and decision state |
| Playable mission and reproducibility | Native full-command replay and playback-speed checks; Chromium tests for mouse/touch aiming, launch timing, no-attack defeat, non-hit victory, terminal controls, and retry |

The queued-hit integration fixture gives the enemy zero initial delta-v to
isolate gun-to-impact-to-outcome behavior; it is not an alternate playable mode
or evidence that hitting the fully fueled default enemy is easy. Browser mission
checks use the normal default scenario. No browser hit-victory fixture or
physical-phone/Safari verification is claimed.

### Tuning and scope limits

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
recomputed from the predicted launch state. At launch, each shot records its
reservation ID and the target passage time calculated from the actual launch
state. Flying rounds retain that ID; the latest four launch IDs and simulation
timestamps appear next to the queue. Dotted white lines extend from each round's
current position to its target, without a travelled trail. The line disappears
after target passage; a missed round continues flying with its number visible.
Target passage is not itself a collision or a hit;
amber dots indicate a constant-observed-velocity estimate, not the AI's future.

Formal score weighting and general paused parameter editing remain step 5.
This milestone reports outcome, elapsed time, shot count, and friendly delta-v.
