# PHASE VECTOR

A browser-based 2D space tactics proof of concept. Place attacks at an enemy's
future position and prevent a breakthrough through hits, evasive course changes,
and delta-v expenditure.

**[Play the blockade](https://kirisaki.github.io/phase-vector/)**

## Status

**Steps 1–5 complete; Step 6 acceptance exercised, with a provisional moving-cover extension.**
One friendly ship must stop a non-firing enemy
from reaching its exit before 1,215 seconds. Maneuver around two moving asteroids
to clear your firing corridor. Fire an unguided railgun at a future
point; the enemy reacts only to delivered attack observations and tries to
recover its course. Win by disabling it or delaying it past the deadline.

Physics and continuous 120-second command cycles remain independent of rendering.
Plan thrust directly on the map and queue multiple shots for future firing
slots, while running or paused. Enemy markers, camera framing, and the amber prediction
use delayed observations. The Lua console exposes true state separately for
diagnostics. The UI and Lua share the same validated command API.

The time slider previews future friendly, observed enemy, and ballistic positions.
Choose a world point and firing time to reserve a shot. AIM leads beyond the
selected instant by the computed flight time. Maneuver changes recompute shot
direction and arrival while retaining firing time and target. Live retains automatic timing.
The result includes a deterministic score and resource breakdown. Paused Lua
settings support reproducible custom trials. Tuning is provisional;
the current enemy is conservative. See [step 4 planning](docs/step-4-planning.md)
for timing tolerance, replanning priority, validation, and prediction limits.
The [acceptance coverage](docs/step-3-blockade.md#acceptance-coverage) records
which requirements are verified natively and through real browser interaction.

See the [PoC specification](docs/poc-spec.md),
[stack decisions](docs/step-1-stack.md),
[physics and time model](docs/step-2-physics.md),
[provisional combat scale](docs/combat-scale.md),
[blockade rules and assumptions](docs/step-3-blockade.md), and
[evaluation and diagnostic settings](docs/step-5-evaluation.md).

## Build and run locally

Install Git, [rustup](https://rustup.rs/), Python 3.10 or newer, and Node.js
matching `^20.19.0 || >=22.12.0` (CI uses Node.js 24). Rust 1.100.0,
`rustfmt`, Clippy, and `wasm32-unknown-unknown` are declared in
`rust-toolchain.toml`. The first build requires access to Rust distribution and
crate and npm servers. The release build uses TypeScript but needs neither
Trunk nor Emscripten.

```sh
git clone https://github.com/kirisaki/phase-vector.git
cd phase-vector
rustup show active-toolchain
cargo install wasm-bindgen-cli --version 0.2.130 --locked
npm ci
python3 scripts/build_web.py
python3 -m http.server 8080 --directory dist
```

Open `http://localhost:8080/` in a browser with WebGL2 enabled. Use an HTTP server;
opening `index.html` with `file://` does not load the WebAssembly module correctly.
`cargo run` is not an entry point: the browser calls the library's `start()` export.

The build script checks that the CLI version matches `Cargo.lock`, generates
WASM declarations, type-checks the UI and Vite configuration, and compiles the
TypeScript modules to JavaScript. It produces a self-contained `dist/` with HTML,
CSS, JavaScript, and WASM. Relative asset URLs support both a local root and the
`/phase-vector/` GitHub Pages project path.

### Live UI development

For UI work, install Node.js satisfying `^20.19.0 || >=22.12.0`, then run:

```sh
npm ci
npm run build:wasm # Once initially, and again after changing Rust code
npm run dev
```

Open `http://127.0.0.1:5173/`. If `dist/pkg/` already contains a current WASM
build, skip the build command. Vite serves `web/` directly and uses the generated
module in `dist/pkg/`; edit `web/`, not `dist/`. CSS changes update in place.
HTML and TypeScript changes reload the page, resetting the simulation and Lua
session. Vite transpiles TypeScript without checking types; run `npm run check`
or keep `npm run check:watch` in another terminal. Generate `dist/pkg/*.d.ts`
with the initial WASM build before checking types. The dev server binds only to
loopback; stop it with Ctrl+C.

Rust changes are not hot-swapped. Run `npm run build:wasm` in another terminal,
wait for it to finish, then reload the page. The build prepares the whole site
in a temporary directory. It publishes generated WASM bindings/declarations
first so the type checker can resolve them, then checks and emits the UI before
publishing the full site. It keeps `dist/pkg/` and its imported paths available
while Vite serves the previous build. This requires the Rust toolchain and
matching wasm-bindgen CLI described above. Vite is only a local development tool;
the release build and GitHub Pages pipeline continue using `scripts/build_web.py`.
See the [Vite server documentation](https://vite.dev/config/server-options) for
watcher settings if your filesystem does not report changes.

To preview on a phone connected to the same LAN, bind to the computer's LAN IPv4
address instead of loopback (replace the example address with your own):

```sh
npm run dev -- --host 192.168.100.3
```

Open `http://192.168.100.3:5173/` on the phone. CSS hot updates and automatic page
reloads work over this connection too. The default `npm run dev` remains limited
to loopback. Use a trusted LAN and stop the LAN server when finished.
On WSL2, mirrored networking supports direct LAN access; Windows/Hyper-V firewall
rules must also permit incoming TCP traffic on port 5173. NAT mode needs additional
forwarding. See [Microsoft's WSL networking guide](https://learn.microsoft.com/en-us/windows/wsl/networking).
The current workstation uses the scoped Hyper-V rule `PhaseVector-Vite-LAN`
(TCP 5173, destination `192.168.100.3`, source `192.168.100.0/24`). To remove this
permission later, run `Remove-NetFirewallHyperVRule -Name PhaseVector-Vite-LAN`
in an administrator PowerShell window. If the LAN address changes, update both
the bind address and the scoped firewall rule.

As of 2026-11-18, `npm audit` reports the transitive `picomatch` ReDoS advisory
[GHSA-fc5q-39xc-vjpf](https://github.com/advisories/GHSA-fc5q-39xc-vjpf), including
its dependents. The registry currently has no patched picomatch release. Keep
this development server on loopback or a trusted LAN; Vite and its dependencies
are not shipped in the published site. Do not use `npm audit fix --force` to
downgrade Vite automatically.

## Mission controls

The application fills the available browser viewport without page scrolling.
The help reader shows a scrollbar when its content overflows; game panels keep
scrollbars hidden. Desktop map panels at least 1,000 px wide place flight,
weapon, mission, and ship controls in a 540 px sidebar on the right. The timeline
stays below the map. Narrow panels use the remaining space above the controls;
short, wide viewports also place controls beside the map. On narrow or short screens, opening
the Lua console replaces the map area until closed. Console history remains
scrollable by touch, wheel, or keyboard. Compact viewports also allow scrolling
within the flight controls so inputs remain reachable while the map keeps a
minimum visible height. Browser chrome stays visible; no fullscreen permission
is requested. The layout follows visual-viewport changes and device safe areas.
On wide desktop panels, the display heading and view selectors share a toolbar.
The sidebar stacks playback, mission status, weapons, telemetry, and burn
controls. Retry stays in a footer at the bottom right while the controls scroll.
Shot reservations scroll vertically in a fixed-height area, with
recent launches below. The layout responds to the map panel's available width,
so opening the Lua console can restore the stacked arrangement. Narrow panels
and short landscape screens retain their compact layouts.
Clock, telemetry, and status rows reserve their space so planning, playback,
pause/resume, and changing digits do not resize the map. Numeric-edit warnings
share the status area; longer messages can be scrolled inside it.
Shared CSS type sizes use 12–14 px for labels, telemetry, and buttons, and 16 px
for help body text and Lua input. Timeline, status, and history rows reserve room
for the larger text. Narrow mission layouts retain at least 160 px of map height
and scroll the controls internally; short keyboard-sized viewports use the compact layout.

| Control | Behavior |
| --- | --- |
| Time slider / Live | Preview a future absolute time and use it for new shot launches / return to current positions and automatic shot timing; scrubbing never changes existing reservations |
| Aim crosshair | Drag the future target point; within 12 screen pixels of observed lead, snap to intercept tracking. Hold Shift to bypass snapping. Arrows move it by 100 km (Shift: 10 km); Home restores lead tracking |
| Fine aim (mouse/touch) | Drag the separate round grip to move the target at 35% sensitivity. Shows a magnified aim detail; keeps the crosshair clear of a finger and bypasses snapping |
| Track lead | Follow the constant-velocity intercept from the last received enemy observation; this is a prediction, not a guaranteed hit |
| Queue shot | In Live, add a shot at the earliest free reload slot. In future preview, reserve the selected point and firing time; the solver determines direction and arrival |
| Cancel / Clear queue | Remove one numbered reservation / all editable reservations; shots committed to the current interval and flying rounds stay unchanged |
| Retry (game controls) | Confirm a restart while playing or paused; restart directly after the result. Clear drafts and restore the initial scenario, paused at time zero |
| Ship-centered Burn handle | Available while running or paused; drag directly without switching modes or changing the camera |
| Drag / release | Preview the burn, delta-v cost, remaining delta-v, and arrival velocity; release to queue the next interval; the current interval keeps its active order |
| Drag outside the ring | Clamp the handle to the reachable circle in the cursor's direction |
| Arrow keys / Shift + arrows | Adjust and immediately update the burn; Shift makes finer adjustments; Home plans coasting |
| Escape during a drag | Discard the current gesture and retain the previously applied plan |
| Numeric thrust → Acceleration X/Y + Apply | Optional expanded inputs in m/s²; magnitude is clamped to 49.03325 (5 G) |
| Overview / Own ship / Contact | Overview automatically fits both ships; Own ship and Contact follow one ship at a fixed zoom |
| Map − / + | Set a manual zoom; select Overview again to resume automatic fitting |
| Drag empty map space | Pan with the mouse or one finger; keep this world position in view |
| Two-finger pinch | Zoom around the midpoint and pan; cancels any uncommitted handle drag |
| Auto / Pause | Start or stop playback; the applied thrust vector repeats every 120 seconds until changed |
| Commit | Confirm orders and play at the selected speed to the next 120-second boundary, then pause |
| Skip | Immediately simulate to the next 120-second boundary and pause, preserving the queued order |
| +10 s | Advance 10 simulated seconds while paused, stopping at the next planning boundary if sooner |
| Playback speed buttons | Select 1×, 32×, 64×, or 128× (default 32×) beside Auto / Pause and Step; the active speed is highlighted and follows Lua changes |
| Help | Open the in-game guide and pause playback; closing it leaves playback paused |
| Show / Hide Lua console | Toggle the initially hidden REPL; input, output, and Lua globals are retained. Close or Escape returns focus to the toggle |
| Run Lua | Evaluate input; Ctrl+Enter or Command+Enter also submits |
| Example buttons | Fill the console input without executing it |
| Reload page | Start a new simulation and Lua session |

Help provides **Quick start**, **Controls**, and **Lua REPL** sections. Its language
switch is shared with Feedback; game controls and Lua commands remain in English. On
first use, a Japanese browser language selects Japanese and other languages use
English. An explicit choice is stored for one year in the first-party
`phase-vector.language` cookie, scoped to the application path with SameSite=Lax
and Secure on HTTPS. Existing localStorage preferences migrate once to this cookie.
If cookies are unavailable, the choice lasts for the current page. Switching
language retains the current section and expanded references. The guide remains
available while the engine is loading, but Lua insertion waits for readiness.

Opening help pauses Auto or Commit without resetting the mission. Closing with
the close button, Escape, or a click outside does not resume playback. Only the
reading area scrolls, keeping language, section, and close controls reachable on
small screens. The background stays visible without dimming. Lua examples replace
the console input and focus it without executing; Run Lua remains an explicit
action. Help is a compact reference, not an interactive tutorial or a full Lua manual.

The Lua input colors keywords, strings, comments, and numbers as you type,
including long bracket strings/comments and incomplete input. Coloring is lexical;
the interpreter still reports syntax errors when executed. Native text selection,
undo/redo, and scrolling remain available. During IME composition the input uses
plain text, and the execution shortcut waits until composition ends. Forced-color
mode also uses plain text. Console history is not syntax-highlighted; completion
and inline error diagnostics are not included.

Browser checks cover tokenization, editing shortcuts, composition event handling,
and overlay alignment at desktop and phone-sized viewports. Physical phone IMEs
and Safari have not been verified for highlighting.

The red **EXIT** circle marks the enemy's destination. Prevent entry until the
1,215-second deadline. A hit wins immediately; fuel exhaustion or a temporary
course deviation alone does not win. Terminal results freeze time and report the
reason, event time, shot count, and friendly delta-v spent.
The standard deadline leaves about 42 seconds beyond unopposed entry, down from
about 57 seconds at the previous 1,230-second deadline. See the
[balance trials](docs/step-3-blockade.md#tuning-and-scope-limits) for reproducible
placement-dependent wins with eight or twelve shots in the open-space baseline.
The default asteroid trial additionally requires clearing the firing corridor; see
[asteroid cover](docs/asteroid-cover.md). Exact simultaneous
impact/entry favors disablement; entry at the deadline beats timeout. Disablement earns higher victory points than timeout.

A centered result dialog announces VICTORY or DEFEAT with the reason, elapsed
time, shots fired, and delta-v spent. The battlefield stays visible without
dimming or blur. Retry restarts immediately; View map, Escape, or a click/tap
outside the dialog dismisses it so the frozen battlefield can be inspected. The
inline result and Retry button remain available after dismissal.

Terminal results also show a total score and its three components. Running missions
have no final score. Disablement earns **2,000 victory points**, including an enemy
asteroid collision. Timeout earns **1,000 + floor(500 × enemy Δv consumed / initial
enemy Δv)**, with the depletion bonus clamped to 0–500. Disablement has no extra
depletion bonus, so an early hit is not penalized for leaving enemy fuel unused.
A zero initial enemy budget earns no depletion bonus.

On either victory, add `max(0, 500 - 20 × shots fired)` for shot economy and
`floor(500 × remaining friendly Δv / initial friendly Δv)` for fuel economy,
clamped to 0–500. Zero initial friendly Δv earns no fuel bonus.
Defeat earns zero points in every component, while retaining actual resource
usage in the breakdown. Reserved or cancelled shots are not counted; only
physical launches cost points. There is no speed bonus. Timeout totals range from
1,000–2,500 and disablement totals from 2,000–3,000. Fuel exhaustion alone never
ends the mission.

The score uses the frozen terminal world (including the full final physics tick's
fuel expenditure) and stays unchanged after dismissal, pause, or playback-speed
changes. It remains available in the inline status and at
`game.state().mission.evaluation` in Lua. The fields are `total`, `victory_points`,
`shot_points`, `fuel_points`, `shots_fired`, and `spent_delta_v` (m/s), plus
`enemy_depletion_points`, `enemy_initial_delta_v`, and `enemy_remaining_delta_v`
(the latter two in m/s). `enemy_depletion_points` is already included in
`victory_points`; do not add it again. Enemy fuel uses the true terminal state and
the scenario's initial budget, not a delayed observation or a fixed standard budget.
This result-only disclosure does not change observed predictions during play.
`evaluation` is nil in Lua / null in UI JSON before a result.

Results identify **Standard**, **Custom settings**, or **Debug run**. Custom means
the initial scenario differs from the standard blockade. Debug means a parameter
actually changed after execution began, even if it was later changed back. Their
scores use the same formula but are not comparable to standard runs. Retry repeats
the initial settings and removes mid-play changes; `game.command("mission")`
restores the standard mission. Lua examples include settings, scores, and standard
reset. No leaderboard, durable save, or general scenario editor is included.
Scoring weights are provisional and remain subject to step 6 playtesting.

The gun has unlimited ammunition and a 30-second reload. Each reservation fires
once at its reserved firing tick, using the firing ship's velocity plus a fixed
660 km/s muzzle velocity. The preview solves direction and flight time for the
chosen world point. Unreachable points are rejected. Pending previews are
recomputed from the predicted launch state; the actual launch solves again.
Valid reservations are sorted by launch time and keep stable IDs. In Live with the default
reload, four initial additions fire at 0, 30, 60, and 90 seconds. Each press of
Queue shot still reserves exactly one round. Cancellation leaves other Live launch
slots unchanged; editable Lua arrival-constrained requests are re-solved against the freed window.
The next Live addition fills the earliest free slot. No launch can be queued at or beyond the mission deadline. Future firing
states assume the draft burn repeats after the frozen current interval, including
acceleration and delta-v limits. Targets stay fixed; changing a target requires
cancelling it and adding another shot. The queue lists firing and arrival times;
unreachable reservations are marked and can be cancelled before committing.
Starting a 120-second interval freezes its reserved shots, including those that
have not fired yet. Their Cancel buttons are disabled. Running or paused edits
inside that interval affect only the next command interval and beyond. Clear
queue removes only editable shots. No shots repeat automatically. At most four
unfired reservations may exist, including locked shots in the active interval.
Flying rounds do not count toward this limit. Each launch or cancellation frees
one slot; new running orders still apply no earlier than the next command
boundary. The Queue button displays the current count out of four. The same
limit is enforced by the UI, Lua, and mission API; ammunition remains unlimited.
All shots in an interval are validated before the interval starts; an invalid
later shot prevents the entire interval from starting until corrected or cancelled.
Already flying rounds are unaffected by later maneuvers. The white dashed line
shows the aim; brighter numbered dashed lines retain all reserved shots. Flying
rounds keep their reservation numbers. Dotted white paths show only the remaining
segment from each round's current position to its target; travelled paths are not
drawn. After target passage the line disappears, while the round and its number
continue along their ballistic course. Target passage does not guarantee a hit.
Recent launches lists the latest four rounds with their actual simulation launch
times, newest first, even after they leave the map. Amber dots show a 360-second constant-observed-velocity
prediction. Enemy fuel and AI decisions are absent from the normal view.

Commit is the cyan primary playback action; Queue is the amber primary weapon
action. Auto retains continuous playback as a secondary action. An amber diamond
marks the enemy's estimated position at the proposed shot's arrival time, using
only the delivered observation and constant velocity. The weapon feedback shows
the estimated offset between that position and the target. Neither the marker
nor lead assist anticipates enemy evasion. Dragging preserves the initial grab
offset so touching the edge of a handle does not make the target jump.

The initial state is paused, with unlimited planning time. Auto starts the first
interval and continues automatically across subsequent 120-second boundaries.
The last applied thrust direction and magnitude repeat; they do not reset to
coasting. A new map or numeric order can be queued while running or paused and
takes effect at the next boundary. The active interval remains unchanged.
Commit plays just to the next boundary and pauses before applying the following
interval's orders. Its highlighted button and countdown show the automatic stop.
At a paused boundary, Auto, Commit, or stepping applies the latest order immediately.
Position, velocity, delta-v, and projectiles persist. Unapplied numeric edits
block Auto, Commit, stepping, and Skip, but never prevent pausing playback.

A pale ring expands from the friendly ship whenever a new interval's order is
applied, including a zero-thrust order. This marks the **120-second command
cycle**, not engine thrust. Its 0.9-second fade uses real time for readability at
all playback speeds. The countdown shows when the next order will apply.

UI coordinates use km with positive Y pointing upward;
velocity and delta-v use
km/s. The grid spacing changes with zoom and is labeled on the map. Symbols and
velocity-direction arrows are enlarged for readability; collision radii remain
physical and are visible as rings at close zoom.

The orange enemy ship symbol points along its observed velocity and keeps a
readable size on phones. Its speed label and faint velocity arrow show motion;
the arrow uses one pixel per km/s, capped at 72 pixels. A bright arrow and the
observed slowing/speeding/turning label show acceleration estimated from the two
latest delivered fixes. The acceleration arrow is enlarged for readability.
These indicators inherit sensor delay and never read the enemy's hidden orders
or future decisions. The symbol indicates travel direction, not hull attitude.

Overview automatically adjusts zoom as the friendly ship and observed contact
move, using the actual viewport aspect ratio and keeping space around both ships. `Overview · Auto` indicates
that this is enabled. Zoom changes ease over real frame time, independently of
playback speed; a large jump or resize zooms out immediately if needed to keep
both ships visible. Manual −/+ zoom suspends automatic fitting until Overview is
selected again. Own ship and Contact keep their selected scale while following
the ship. Automatic zoom pauses during a burn drag so the control stays stable.
Camera changes never alter simulation state or commands.

Dragging empty map space or pinching switches to a fixed world-centered camera,
including during playback. A pinch keeps the world position under the gesture's
midpoint anchored. One finger on the burn ring or an aiming handle still edits
that order; adding a second finger discards the current drag draft and navigates
the map instead. Previously applied orders remain intact. Select Overview to
restore automatic fitting, or Own ship / Contact to resume following a ship.
Retry retains the camera settings. Pinch handling is limited to the map;
trackpad gestures and mouse-wheel zoom are not implemented.

The control ring is centered on the friendly ship; manipulating it does not
change camera focus or zoom. Its **Burn** handle specifies a maneuver relative to inertial coasting:
direction chooses thrust direction and distance chooses the maneuver amount.
The ring is a control, not a destination at the map's scale. Its size depends
only on the viewport: panning near an edge clips it instead of shrinking it or
changing the thrust-to-pixel ratio. The center means
zero thrust. The actual trajectory, boundary times, and arrival-velocity arrow remain
at map scale, without a circular arrival marker. An amber arrow shows the queued
burn from the ship; the cyan velocity arrow shows actual travel direction.
The planned route uses cyan dots with fixed screen spacing, including during
dragging and playback. It reaches the next three command boundaries (up to
360 seconds), with time labels at each boundary. During execution the current
interval remainder counts as the first; subsequent intervals repeat the queued
vector. Fuel exhaustion naturally continues as inertial coasting. The gray
dashed line shows the coasting reference.

While an interval is executing, the preview first follows its unchanged active
burn to the next boundary, then follows the queued order for two more intervals.
The burn control and arrival-velocity arrow still target the first queued
interval. The cost readout covers that queued interval; remaining delta-v and
arrival velocity also account for the active
remainder. The status line shows the active thrust separately. Repeated orders
retain thrust direction and magnitude, not an absolute arrival destination.

Thrust changes velocity; it does not turn the ship instantly toward the handle.
The current trial uses a 75 km/s initial speed, 5 G (49.03325 m/s²) maximum
acceleration, and a 60 km/s delta-v budget. A full 120-second lateral burn adds
5.88399 km/s, changes heading by about 4.486°, and displaces the ship 353.775 km
sideways relative to coasting. The budget supports about 10.2 full-thrust
intervals. Overview spans the 240,000 km initial engagement, so the physical
route displacement is much smaller than the enlarged burn control.
View and zoom buttons remain available to inspect the actual trajectory.

Orders specify pass-through positions, not automatic braking or station keeping.
Times are fixed in this version; there are no waypoints or multi-burn schedules.
Releasing a graphical drag replaces the current draft, including any unapplied
numeric input. Escape, interrupted touch, or loss of window focus cancels only
the current gesture and preserves the previous draft and numeric input. The
telemetry and preview metrics remain visible throughout. Auto from pause, stepping,
Commit, and Skip are disabled during a drag; Pause remains available while running.
A drag that crosses a command boundary keeps its direction and fraction of the
reachable disk, rebased onto the next interval. External order changes through
Lua cancel a stale drag, retaining the last applied plan. Releasing the handle queues an order
without changing playback. Explicit Lua arrival commands reject unreachable
endpoints instead of clamping them.

```lua
answer = 6 * 7; print("Answer:", answer)
return game.state().phase
return game.state().ship.remaining_dv
game.burn(49.03325, 0)
game.command("play")
game.command("pause")
game.command("step"); return game.state().time_seconds
game.command("step_forward") -- Same 10-second advance as the UI
game.speed(64)
game.command("play")
game.command("commit") -- Animate to the next turn and pause
game.command("skip") -- Immediately simulate to the next turn and pause
game.command("reset")
```

Bare expressions also work. `game.state()` returns a detached snapshot with
`phase`, `paused`, `ticks`, `time_seconds`, `remaining_seconds`, `order_start_seconds`, `speed`,
`command_number`, `command_count`, `planned_burn`, `active_burn`, and `ships`. `ship` aliases the
friendly ship snapshot. Ship fields include `id`, `x`, `y`, `vx`, `vy`,
`remaining_dv`, `spent_dv`, and `max_acceleration`. Projectile and impact counts
are also exposed. Lua continues to use m and m/s, and `game.burn` takes m/s².
`game.arrive(x, y)` accepts an absolute position in metres at the end of the next
120-second interval. During execution, that means the interval after the current
one. The solver starts from the predicted boundary state, including fuel used
by the active burn, and shares validation with the GUI.
Unreachable endpoints return an error without changing the existing plan.
`game.state().timing` exposes the shared `step_seconds`, `interval_ticks`,
`interval_seconds`, `step_forward_ticks`, `step_forward_seconds`,
`forecast_intervals`, and `forecast_seconds`. The browser uses the same values
for tick conversion and labels. These are read-only snapshots, not parameter
setters.
`game.state().scale` exposes `light_speed_mps`, `railgun_speed_mps`,
`reference_range_m`, and `reference_flight_seconds`. Changing these Lua tables
does not modify the Rust state.
`game.command("reset")` (also `retry`) restores the current scenario at tick zero
with full delta-v and 32× playback while retaining Lua globals. The normal Retry
button is available during play, while paused, and after mission completion.
During an unfinished mission it opens an in-game confirmation dialog and pauses
playback. Cancel, Escape, or clicking outside keeps plans and inputs intact and
restores the previous Auto/Commit playback mode. Confirming clears UI drafts and stops active editing
gestures. `game.command("sandbox")` selects
the original physics sandbox; `game.command("mission")` starts a new blockade.
The URL query `?sandbox=1` also starts the sandbox for development and regression
testing.
`game.command("step")` retains the diagnostic single-tick (0.25 s) advance;
`game.command("step_forward")` matches the UI's +10 s button. Both require paused
playback and process all physical ticks without crossing a command boundary.
They can start the next interval while staying paused. `play` and `resume` select
continuous playback. `commit` starts or resumes playback with an automatic stop
at the next command boundary. Repeated Commit calls during an active interval
do not extend its stop time or change its frozen orders. Speed changes preserve
the automatic stop. `pause` cancels the automatic stop; use Commit to continue
only to the boundary or Auto to continue across turns. `skip` immediately
simulates to that boundary and pauses; `next_turn` remains its compatible alias.
`game.state().stop_at_boundary` reports whether Commit playback is active.
Invalid commands,
syntax errors, and runtime errors appear in the console. One evaluation has a
20,000-fuel budget to interrupt accidental infinite loops. Effects performed
before an error or budget exhaustion are retained; there is no transaction rollback.
Input is limited to 4,096 UTF-8 bytes, printed output to 64 lines per evaluation,
each displayed value to 1,024 characters, and visible history to 100 entries.

Mission commands and privileged diagnostics:

```lua
game.fire(game.aim()) -- Append an observed-lead target at the earliest free reload slot
game.fire(game.aim()) -- Append another shot one reload later
game.state().mission.queue -- IDs, targets, launch ticks/seconds, and locked status
game.cancel_shot(game.state().mission.queue[1].id) -- Cancel one reservation
game.command("cancel_shot") -- Clear editable reservations; active interval stays frozen
game.fire(120000000, -13600000) -- Fixed world target in metres
return game.state().mission.outcome, game.state().mission.shots
return game.state().mission.ai_reason, game.state().mission.evasions
return game.state().mission.contact.sampled_tick, game.state().mission.contact.received_tick
return game.state().mission.threats[1] -- Delivered projectile track, if available
```

`game.state().ships` is authoritative diagnostic state. `mission.contact` and
`mission.threats` are detached delivered observations with `id`, `x/y`, `vx/vy`,
`radius`, `sampled_tick`, and `received_tick`. `ai_ax`, `ai_ay`, and
`ai_hold_until_tick` expose the current enemy order. Editing these returned Lua
tables does not change the simulation. Use the explicit diagnostic settings API:

```lua
game.parameters() -- Detached current settings, in seconds and metres
game.configure("reaction_seconds", 60)
game.configure("safety_radius_m", 250000)
game.state().mission.initial_parameters -- Retry baseline
game.state().mission.parameter_edits -- Tick, parameter, value, during_play
game.state().mission.run_kind -- standard, custom, or debug
game.command("retry") -- Restore the baseline, discarding mid-play edits
game.command("mission") -- Restore the standard blockade and all default settings
```

Configuration requires paused playback and an unfinished blockade. Before the
first command, edits become the Retry baseline. During play, only AI policy edits
are accepted; they mark the run as debug, release the previous AI hold, and take
effect at its next one-second decision tick. They do not advance time or rewrite
observations, frozen player orders, reservations, or flying rounds. Retry restores
the initial world and baseline settings, paused, with empty queues and edit history.
A custom baseline stays custom after Retry. Returning a live-edited parameter to
its old value does not remove the debug label. Lua globals survive Retry.

| Parameter | Allowed range | When editable |
| --- | --- | --- |
| `deadline_seconds` | 0.25–7,200 s | Before play, empty shot queue |
| `reload_seconds` | 0.25–600 s | Before play, empty shot queue |
| `sensor_processing_seconds` | 0–120 s | Before play, empty shot queue |
| `reaction_seconds` | 0–600 s | Paused |
| `safety_radius_m` | 0–2,000,000 m | Paused |
| `hold_seconds` | 0–120 s | Paused |
| `recovery_seconds` | 0.25–1,200 s | Paused |

The first three settings require exact 0.25-second multiples. Other settings
retain their numerical precision. These bounds limit diagnostic computation;
they are not balance recommendations. Unknown names, invalid types, nonfinite or
out-of-range values, running edits, and edits after a result fail without mutation.
Each `configure` call is atomic; multiple Lua calls are not a transaction. A
no-op value creates no history entry. Step size, command duration, ship geometry,
fuel, and muzzle speed remain fixed in this PoC API.

## Private feedback

**Feedback** in the header or mission result opens a private report form with a
category, message, and optional application screenshot. No rating is requested. The game
pauses, captures before the dialog opens, and lets you review metadata and the
image. Images are attached only after selecting the attachment checkbox; no
report is sent until consent, Turnstile verification, and Send feedback.
Lua content and true enemy fuel are excluded from capture and metadata. Capture
failure does not prevent text submission. The game still works with no backend.

On desktop the form uses two columns, with a separate Data & privacy tab for the
metadata review. Both tabs fit without dialog scrolling at the tested desktop
viewports (900×600 through 1360×1000); smaller/mobile viewports can scroll to keep
all controls accessible. Japanese/English controls in Feedback and Help share
the same language cookie. Changing language preserves the report and attachment
choice; an existing verification must be repeated in the new language.

The optional `backend/` Worker stores structured reports in D1 and images in
private R2. It has its own lockfile, tests, and deployment workflow. The production
bindings identify the owner's resources; deployments to other accounts require
their own resources, secrets, and public frontend configuration.
See [the feedback setup and operating guide](backend/README.md) for the complete
Cloudflare checklist, API contract, retention, privacy, tests, and limitations.

The static build reads public `PV_FEEDBACK_API_URL` and `PV_TURNSTILE_SITE_KEY`
environment variables and writes `runtime-config.json` with the Git revision.
For Vite use `.env.local` based on [.env.example](.env.example); for Pages use the
GitHub Actions variables `FEEDBACK_API_URL` and `TURNSTILE_SITE_KEY`. These are
public configuration, never secret keys. The UI remains plain TypeScript; only
the DOM capture module bundles its dependency. Backend deployment is independent
of Pages and is never triggered by merely opening the game.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --example blockade_trials
cargo run --locked --example asteroid_trials
cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings
python3 scripts/build_web.py
npm run check
npm --prefix backend ci
npm --prefix backend test
```

Native Rust tests cover inertial motion, acceleration magnitude limits, partial
fuel exhaustion, braking, fast and moving-target contacts, temporal misses,
contact ordering, projectile flight across command intervals, metre-scale misses at
astronomical distances, camera-relative precision, deterministic replay, and
WEGO boundaries at every playback speed,
queued order timing, projectile persistence, reset, and Lua error recovery.
Endpoint-planning tests cover prediction/execution agreement, partial fuel
exhaustion, unreachable targets, and shared Lua validation. Presentation tests
check the JSON contract, observation-only enemy state and camera framing, and
separation of pending reservations from already flying rounds.
Mission and observation tests additionally cover queued-hit victory with pending
launches, actual route recovery after an oblique attack, reservation secrecy,
light-delay delivery, out-of-order samples, and stale-track expiration.
Bevy is a browser-target dependency; native tests need no display server.

For real browser interaction checks:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r requirements-browser.txt
python -m playwright install --with-deps chromium
python scripts/browser_smoke.py
python scripts/mission_smoke.py
python scripts/feedback_smoke.py
```

The browser suites use `scripts/browser_site.py` to serve `dist/` at a virtual
HTTPS `/phase-vector/` URL, preserving the deployment subpath and WASM MIME type.
The sandbox smoke test checks engine startup, a visible scene
change after commands, delta-v consumption, exact interval boundaries, queued
orders, and identical results from playback and manual stepping. A pointer-authored
plan is executed through the actual Auto button and its final position, velocity,
and delta-v are compared with the preview; running edits preserve the active burn.
Continuous playback, repeated orders, and the interval pulse are also checked. The test also checks
help pause/focus behavior, language persistence, section retention, Lua insertion
without execution, and help layouts down to a 320 px phone viewport.
It checks
Lua persistence, validation, error recovery, direct map dragging and release-to-apply,
radial clamping outside the reachable circle, automatic zoom and manual override,
gesture cancellation, keyboard
editing, and touch dragging in narrow viewports. Screenshots and errors are
written to ignored `artifacts/`. `--executable-path` can select an existing
Chromium installation. With Vite running, verify module resolution during a WASM
rebuild and then run the same browser interactions against the development server:

```sh
python scripts/browser_smoke.py --url http://127.0.0.1:5173/ --rebuild
```

The mission smoke test uses the default game and checks observed prediction,
mouse/touch target dragging, queue/cancel, running reservations, 30-second salvo timing, frozen active shots,
ballistic persistence, asteroid interception, no-attack defeat, maneuver-enabled
12-shot non-hit timeout victory,
terminal locks, retry, desktop/phone layouts, mouse panning, anchored touch
pinching, and cancellation of editor drafts when a second finger takes over.
It also accepts `--url` and
`--executable-path`.

This requires the build tools on `PATH`. To exercise the real hosted site:

```sh
python scripts/browser_smoke.py --url https://kirisaki.github.io/phase-vector/
python scripts/mission_smoke.py --url https://kirisaki.github.io/phase-vector/
```

## CI and GitHub Pages

[CI and Pages](.github/workflows/ci.yml) runs Rust checks, builds the release site,
and runs Chromium interaction checks before packaging it. Pull requests can test
and upload artifacts, but cannot deploy. Successful `main` builds publish the
same tested artifact to Pages, then repeat browser checks against the public URL.
Screenshots are retained as Actions artifacts for seven days.

Pages uses the GitHub Actions publishing mode. An administrator can configure a
new repository entirely through the authenticated GitHub CLI:

```sh
gh api --method POST repos/kirisaki/phase-vector/pages -f build_type=workflow
gh workflow run ci.yml --repo kirisaki/phase-vector --ref main
gh run list --repo kirisaki/phase-vector
gh run watch RUN_ID --repo kirisaki/phase-vector --exit-status
```

Use `GET` to inspect an existing Pages configuration and `PUT` to change it;
the creation command is only needed once. This publishes static files and does
not require a game server or custom domain.

## Moving asteroid cover

The default blockade includes two moving asteroids. Brown **A1** (near your ship) and
**A2** (near the enemy) show their physical radii and dotted predicted routes.
Each starts 25,000 km inward from its nearer ship along the engagement axis.
Scrubbing the firing-time slider shows their
future positions. **Blocked by A1/A2** and a red contact cross warn when a round will
be absorbed before reaching its target. Queue still accepts such a shot, and
maneuver changes recalculate reservations. Own-ship collision loses the mission;
enemy collision disables it. Both asteroids are indestructible and block rounds
from either side, but do not block sensors.

Use Burn on the map to change your firing position. For a reproducible starting
experiment, drag Burn down to maximum slowdown, advance two intervals, then use
Home while Burn is focused to coast. From 240 s, three salvos of four shots with
200 km vertical spacing can deny entry without a hit with the provisional
90-second enemy reaction horizon. Four rounds are absorbed by A2 in this trial.
The same shots and timing
while coasting are absorbed; repeated lead shots after maneuvering still lose.
This is a provisional example, not the only winning strategy. See
[asteroid rules and measured trials](docs/asteroid-cover.md) for tuning values,
collision ordering, diagnostics, assumptions, and verification.

## Future planning

Move the time slider to choose an absolute **firing time** and preview positions
at that instant. The cyan portion reserves 40% of the bar for now through the
next command boundary; amber covers later times through the planning limit.
Marks show 120-second boundaries. Pointer/touch input lightly snaps near them
(within 5 pixels and 6 seconds). Arrow keys bypass snapping and move one
0.25-second tick; Page Up/Down move 120 seconds, Home selects now, and End selects
the preview limit. The scale stays fixed during a drag even while Auto runs.
The two portions use different scales; labels show absolute seconds.

Green diamonds mark the start of each free firing window, including openings
between existing reservations. **Ready 30 s**, for example, identifies the next
reload opening. These marks update on queue/cancel, firing, and command boundaries,
and disappear when all four reservation slots are occupied or the mission ends.
Pointer/touch input snaps within 5 pixels of a green mark, with priority over
nearby command ticks. Arrow keys still bypass snapping. Marks use the same gun
schedule as Queue; they do not reserve a shot or guarantee an intercept.

AIM uses the friendly ship's predicted firing position and velocity, then leads
the observed enemy by the projectile flight time. The enemy-at-fire marker and
the later arrival estimate are distinct. For example, selecting 120 seconds
reserves a shot firing at 120 seconds, not one that must arrive in 120 seconds.
While dragging Fine aim, a passive inset centers on the target at eight times
the main map's scale at gesture start. It follows the Fine aim handle, choosing
an adjacent position within the map that avoids the pointer and crosshair when
space permits. Its world scale stays fixed for the gesture. The inset shows the
approaching ballistic segment and the same observation-based enemy arrival
estimate as the main map, with offset and a scale bar in kilometres. An outlined
ship marked `At impact` shows the predicted arrival position, oriented along the
observed velocity. An orange dotted route extends from the current estimated
position to the forecast horizon or shot arrival, whichever is later. When the
current estimate falls inside the inset, a filled ship and `Now · est.` label
distinguish it from the arrival prediction. An edge arrow and `Off view` indicate
an arrival estimate outside the inset. Marker sizes are symbolic. Missing shot
solutions clear the ballistic segment and arrival marker; the observed course
remains available. Neither the ships nor the route read privileged enemy state.

The inset follows the existing preview while playing or paused, without changing
time, aim sensitivity, orders, or camera framing. It remains for 1.5 real seconds
after release. Cancellation (including a second finger taking over navigation),
Retry, help, or mission completion removes it immediately. Regular Aim dragging
and keyboard nudges do not open it. The inset's dimensions adapt to the map;
it may overlap other map labels and changes sides when needed near edges.
Desktop and phone-sized Chromium interaction checks cover the inset; physical
phone and Safari testing remains outstanding.

Drag Aim or Fine aim to choose a different fixed world point. Queue stores that
point and firing tick. Track lead restores the computed intercept.

Press **Q** while the timeline is focused or being dragged, or hold one finger
on the timeline and tap **Queue** with another. Holding Q does not repeat shots,
and Q does not intercept Lua text entry. Existing orders stay fixed as the bar
moves. Reload conflicts, a full queue, or firing inside the locked active interval
reject the reservation with a reason. Firing must precede the mission deadline.
The preview can show the deadline, but Queue cannot schedule a launch there.

Cyan map markers show the friendly forecast, amber markers show the observed
enemy, and numbered markers show existing/reserved rounds. These forecasts do
not predict enemy decisions or hits. Live restores earliest-free-slot timing;
reaching the selected time during playback also returns to Live. Scrubbing
never advances the world. The horizon is bounded to 3,600 seconds and the deadline.

Maneuver changes preserve fixed firing ticks and targets while recomputing the
ballistic previews. Active orders and flying rounds remain unchanged. Lua shares
this workflow through `game.aim_at(seconds)`, `game.preview_launch(x, y, seconds)`,
and `game.fire_at_launch(x, y, seconds)`. The original arrival-constrained solver
remains available through `game.preview_shot_at` and `game.fire_at`; those requests
can change their firing ticks after a maneuver. See [planning rules](docs/step-4-planning.md)
for both APIs, validation, and replanning priority.

## Assumptions and limitations

- Bevy 0.20.1 and `piccolo` 0.3.3 are pinned for this feasibility check.
- `piccolo` is an experimental pure Rust Lua implementation with incomplete Lua
  compatibility. This probe does not promise arbitrary Lua libraries, filesystem
  I/O, package loading, or a full modding environment.
- The REPL is a local developer tool. Fuel limiting is not a hardened memory or
  CPU sandbox; standard-library calls and compilation have their own costs.
- Browser execution is single-threaded and uses WebGL2. WebGPU, shared-memory
  threads, native graphical builds, and cross-browser compatibility are not
  verified in this milestone. CI uses Chromium with software rendering.
- Physics uses 0.25-second ticks and `f64` values. Each tick applies a bounded
  velocity increment, then drifts linearly. Swept contacts are continuous along
  these discrete segments; sub-tick thrust cutoff and curved paths are not modeled.
- Replay is deterministic for the same initial state, commands, and build/runtime.
  Bit-identical results across browsers or CPU architectures are not claimed.
- The current trial uses a 660 km/s railgun reference and 240,000 km initial
  separation, giving a stationary-target flight time of about 363.636 seconds.
  Ships start at 75 / 37.5 km/s with 5 G maximum acceleration (using standard
  gravity), 60 km/s delta-v each, and the retained 0.5 km collision radii.
  Ship motion uses a Newtonian approximation. Sensor samples arrive after 1 s
  processing plus rounded-up light travel time measured at sampling. Moving
  receivers, noise, occlusion, and relativistic effects are not modeled.
- The enemy reacts autonomously once per second, within player command intervals.
  Its 90 s threat horizon is independent of projectile flight time. The provisional
  safety corridor is 500 km, with 1 s evasion commitments. The controller checks
  multiple observed rounds together, prefers safe low-thrust candidates, and
  resumes cruise recovery as soon as it predicts that recovery is safe.
  Candidate prediction approximates constant burns with finite delta-v; it is
  not an optimal pilot. See [balance trials](docs/step-3-blockade.md#tuning-and-scope-limits).
- The normal view extrapolates the latest received enemy track. The initial track
  is supplied by the briefing. Lua inspection intentionally exposes true state.
- One constant burn repeats each interval; a shot reservation fires only once.
  Lua arrival-constrained shots use a 0.25-second tolerance because launch times stay on the physical tick grid. The solved arrival time is shown explicitly; the target and muzzle speed stay fixed.
- Projectiles have no expiry or map culling. Contacts consume projectiles and
  record events; hitting the enemy ends the mission with disablement. Terminal
  physics is frozen at the end of the event tick; result time includes its fraction.
- Goal circles have a minimum display radius of 3 px for readability; physical
  entry uses the specified 1,000 km radius. Symbols are also enlarged.
- Scoring weights are provisional; diagnostic settings cover only the documented subset. The default balance
  demonstrates maneuver- and placement-dependent denial and is not final difficulty tuning.
- First-load size and performance are not final. Browser errors are shown in the
  page; details are also available in the browser developer console.

## Development conventions

Write README files, design documents, and code comments in English. Reports may
be in Japanese. Use Conventional Commits, keep the implementation small, separate
simulation from presentation, and share the command API between UI and Lua.
Commit `Cargo.lock`; do not commit credentials or generated output.

Shared numerical settings have distinct owners:

| Values | Source |
| --- | --- |
| SI conversions, light speed, standard gravity | `src/units.rs` |
| Physics tick rate, playback clock, command/step duration, forecast horizon | `src/timing.rs` |
| Ship speeds, range, acceleration, delta-v, collision radii, muzzle speed | `src/scenario.rs` |
| Goal, deadline, reload, sensor processing | `mission::Config::default()` |
| Sensor sampling/retention and AI decision cadence | Named constants in `observation.rs` and `ai.rs` |
| Independent threat horizon, safety corridor, evasion/recovery duration | `ai::Policy::default()` |
| Score weights | `src/evaluation.rs` |
| Aim sensitivity and magnifier scale/lifetime, timeline snapping/geometry, navigation and burn gestures, command pulse, history length | `web/ui-settings.ts` |

Whole-second durations become ticks through `timing::seconds_to_ticks`; the
playback clock must be an integer multiple of the physics frequency. Equal
numbers with different meanings remain separate: a 30 s reload or 90 s threat
horizon does not inherit the 120 s command interval. Solver tolerances stay
local with quantity-specific names and units.

Tests import these settings for setup and execution boundaries. Browser tests
read `state_json().timing`, and Lua exposes the same table. Independent analytic
expectations and selected trial regression anchors in `tests/scale.rs` remain
literal so an incorrect shared constant cannot silently make a test pass.
Parameter tests cover atomic rejection, paused-only edits, frozen orders and flying
rounds, deterministic continuation, and custom Retry baselines. Evaluation tests
cover both victory types, zero-point defeats, actual launches versus reservations,
fuel normalization, bonus limits, terminal stability, and matching UI/Lua output.
Browser input helpers read `web/ui-settings.ts` and computed slider geometry
instead of copying sensitivity or thumb dimensions. The slider's CSS dimensions
and snap calculation share the same thumb size. Expected gameplay outcomes,
small collision fixtures, assertion tolerances, and test gesture offsets keep
independent values; they are not scenario tuning.

`session::Snapshot` is authoritative state for commands and Lua diagnostics.
Its shot planning methods are shared by command validation, Lua, and UI previews;
command boundaries remain integer ticks throughout prediction and reservation locking.
`presentation::State` explicitly selects the normal UI's serialized fields,
using the same observed world as the renderer and camera. This module has no
browser dependency and is covered by native tests. Keep new normal-view fields
here rather than serializing the authoritative snapshot in browser bindings.

Browser code uses TypeScript with `strict`, `isolatedModules`, and unused-code
checks. `web/types.ts` describes the normal-view fields consumed by the UI;
`web/api.ts` is the single JSON decoding boundary for trusted local Rust output.
These hand-maintained JSON types do not validate external data or automatically
track Rust schema changes. Keep them aligned with the Rust presentation and
planning contracts. `wasm-bindgen` generates the function declarations, so those
signatures are not duplicated. `rootDirs` joins source modules and generated
`dist/pkg/` declarations for type checking; emitted ES modules retain relative
`.js` imports. `web/dom.ts` checks required elements and supplies types for IDs
from `index.html`. Keep that ID table aligned with markup changes. Vite's own
configuration is checked separately by `tsconfig.node.json`.

`web/map-overlay.ts` owns the SVG projection shared by burn planning and aiming.
`web/mission.ts` coordinates previews and commands. `web/aim-editor.ts` owns the
draft target, fine aim, keyboard input, and pointer capture/cancellation; it
notifies the controller without importing WASM. `web/timeline-scale.ts` contains
the pure time mapping and snap policy, while `web/timeline.ts` owns selection,
pointer ownership, forecast caching, and DOM updates. `web/mission-panel.ts`
renders status, reservation/history rows, and the result dialog, forwarding actions
through controller callbacks. `web/mission-preview.ts` computes one observation-only
arrival estimate for weapon feedback, the map, and the passive inset in
`web/aim-detail.ts`; `web/aim-detail-layout.ts` selects the inset's position using
surface-local geometry without reading the DOM. `web/mission-map.ts` draws
the supplied state and preview without issuing commands. These are plain modules;
they do not introduce a separate simulation or command API. Keep the existing
command-boundary and gesture behavior when changing presentation.

`web/dialog.ts` shares Escape and backdrop dismissal handling between Help,
Retry, and the result dialog. Each caller retains its own focus and playback
behavior. `web/help.ts` owns help navigation and language selection; localized
content lives in `web/help-content.ts`. Dialog styles live in
`web/styles/dialogs.css`, loaded after the game layout in `web/style.css` to
preserve the cascade, including the help reader's visible scrollbar.

## License

Copyright (c) 2026 Akihito Kirisaki <kirisaki@klara.works>.

Licensed under either the [MIT license](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.
SPDX: `MIT OR Apache-2.0`.
