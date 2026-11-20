# PHASE VECTOR

A browser-based 2D space tactics proof of concept. Place attacks at an enemy's
future position and prevent a breakthrough through hits, evasive course changes,
and delta-v expenditure.

**[Play the blockade](https://kirisaki.github.io/phase-vector/)**

## Status

**Step 3: blockade mission.** One friendly ship must stop a non-firing enemy
from reaching its exit before 1,200 seconds. Fire an unguided railgun at a future
point; the enemy reacts only to delivered attack observations and tries to
recover its course. Win by disabling it or delaying it past the deadline.

Physics and continuous 120-second command cycles remain independent of rendering.
Plan thrust directly on the map and queue a single shot for the next boundary,
while running or paused. Enemy markers, camera framing, and the amber prediction
use delayed observations. The Lua console exposes true state separately for
diagnostics. The UI and Lua share the same validated command API.

This milestone includes minimal aiming, prediction, result metrics, and retry
controls to play and verify the mission. Arbitrary shot arrival times, multiple
future reservations, a time slider, formal scoring, and paused parameter editing
remain steps 4–5. Tuning is provisional; the current enemy is conservative.

See the [PoC specification](docs/poc-spec.md),
[stack decisions](docs/step-1-stack.md),
[physics and time model](docs/step-2-physics.md),
[provisional combat scale](docs/combat-scale.md), and
[blockade rules and assumptions](docs/step-3-blockade.md).

## Build and run locally

Install Git, [rustup](https://rustup.rs/), and Python 3.10 or newer. Rust 1.100.0,
`rustfmt`, Clippy, and `wasm32-unknown-unknown` are declared in
`rust-toolchain.toml`. The first build requires access to Rust distribution and
crate servers. The release build does not require Node.js, Trunk, or Emscripten.

```sh
git clone https://github.com/kirisaki/phase-vector.git
cd phase-vector
rustup show active-toolchain
cargo install wasm-bindgen-cli --version 0.2.130 --locked
python3 scripts/build_web.py
python3 -m http.server 8080 --directory dist
```

Open `http://localhost:8080/` in a browser with WebGL2 enabled. Use an HTTP server;
opening `index.html` with `file://` does not load the WebAssembly module correctly.
`cargo run` is not an entry point: the browser calls the library's `start()` export.

The build script checks that the CLI version matches `Cargo.lock`. It produces a
self-contained `dist/` with HTML, CSS, JavaScript, and WASM. Relative asset URLs
support both a local root and the `/phase-vector/` GitHub Pages project path.

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
HTML and JavaScript changes reload the page, resetting the simulation and Lua
session. The dev server binds only to loopback; stop it with Ctrl+C.

Rust changes are not hot-swapped. Run `npm run build:wasm` in another terminal,
wait for it to finish, then reload the page. The build prepares the whole site
in a temporary directory before replacing output files. It keeps `dist/pkg/`
and its imported paths available while Vite serves the previous build.
This requires the Rust toolchain and
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

The application fills the available browser viewport without page scrolling or
visible scrollbars. The map uses the remaining space above the controls; short,
wide viewports place the controls beside it. On narrow or short screens, opening
the Lua console replaces the map area until closed. Console history remains
scrollable by touch, wheel, or keyboard. Compact viewports also allow scrolling
within the flight controls so inputs remain reachable while the map keeps a
minimum visible height. Browser chrome stays visible; no fullscreen permission
is requested. The layout follows visual-viewport changes and device safe areas.
Clock, telemetry, and status rows reserve their space so planning, playback,
pause/resume, and changing digits do not resize the map. Numeric-edit warnings
share the status area; longer messages can be scrolled inside it.

| Control | Behavior |
| --- | --- |
| Aim crosshair | Drag the future target point on the map; arrows move it by 100 km (Shift: 10 km), Home restores observed lead tracking |
| Track lead | Follow the constant-velocity intercept from the last received enemy observation; this is a prediction, not a guaranteed hit |
| Queue shot / Replace shot | Reserve one shot at the next 120-second boundary; subsequent aim movement does not change the reservation until Replace is pressed |
| Cancel shot | Remove an unlaunched reservation; flying rounds remain ballistic |
| Retry | Available after victory or defeat; restore the complete initial scenario |
| Ship-centered Burn handle | Available while running or paused; drag directly without switching modes or changing the camera |
| Drag / release | Preview the burn, delta-v cost, remaining delta-v, and arrival velocity; release to queue the next interval; the current interval keeps its active order |
| Drag outside the ring | Clamp the handle to the reachable circle in the cursor's direction |
| Arrow keys / Shift + arrows | Adjust and immediately update the burn; Shift makes finer adjustments; Home plans coasting |
| Escape during a drag | Discard the current gesture and retain the previously applied plan |
| Numeric thrust → Acceleration X/Y + Apply | Optional expanded inputs in m/s²; magnitude is clamped to 49.03325 (5 G) |
| Overview / Own ship / Contact | Overview automatically fits both ships; Own ship and Contact follow one ship at a fixed zoom |
| Map − / + | Set a manual zoom; select Overview again to resume automatic fitting |
| Play / Pause | Start or stop playback; the applied thrust vector repeats every 120 seconds until changed |
| Next turn | Execute to the next 120-second boundary and pause, preserving the queued order |
| +10 s | Advance 10 simulated seconds while paused, stopping at the next planning boundary if sooner |
| Playback speed buttons | Select 1×, 32×, 64×, or 128× (default 32×) beside Play / Pause and Step; the active speed is highlighted and follows Lua changes |
| Show / Hide Lua console | Toggle the initially hidden REPL; input, output, and Lua globals are retained. Close or Escape returns focus to the toggle |
| Run Lua | Evaluate input; Ctrl+Enter or Command+Enter also submits |
| Example buttons | Fill the console input without executing it |
| Reload page | Start a new simulation and Lua session |

The red **EXIT** circle marks the enemy's destination. Prevent entry until the
1,200-second deadline. A hit wins immediately; fuel exhaustion or a temporary
course deviation alone does not win. Terminal results freeze time and report the
reason, event time, shot count, and friendly delta-v spent. Exact simultaneous
impact/entry favors disablement; entry at the deadline beats timeout. No formal
points formula is implemented yet.

The gun has unlimited ammunition and a 120-second reload. Each reservation fires
once at the next command boundary, using the firing ship's velocity plus a fixed
660 km/s muzzle velocity. The preview solves direction and flight time for the
chosen world point. Unreachable points are rejected. Pending previews are
recomputed from the predicted launch state; the actual launch solves again.
Already flying rounds are unaffected by later maneuvers. The white dashed line
shows the aim; a brighter dashed line retains the reserved shot. Thin white
trails show launched rounds. Amber dots show a 360-second constant-observed-velocity
prediction. Enemy fuel and AI decisions are absent from the normal view.

The initial state is paused, with unlimited planning time. Play starts the first
interval and continues automatically across subsequent 120-second boundaries.
The last applied thrust direction and magnitude repeat; they do not reset to
coasting. A new map or numeric order can be queued while running or paused and
takes effect at the next boundary. The active interval remains unchanged.
At a paused boundary, Play or stepping applies the latest order immediately.
Position, velocity, delta-v, and projectiles persist. Unapplied numeric edits
block Play, stepping, and Next turn, but never prevent pausing playback.

A pale ring expands from the friendly ship whenever a new interval's order is
applied, including a zero-thrust order. This marks the **120-second command
cycle**, not engine thrust. Its 0.9-second fade uses real time for readability at
all playback speeds. The countdown shows when the next order will apply.

UI coordinates use km with positive Y pointing upward;
velocity and delta-v use
km/s. The grid spacing changes with zoom and is labeled on the map. Symbols and
velocity-direction arrows are enlarged for readability; collision radii remain
physical and are visible as rings at close zoom.

Overview automatically adjusts zoom as the friendly ship and observed contact
move, using the actual viewport aspect ratio and keeping space around both ships. `Overview · Auto` indicates
that this is enabled. Zoom changes ease over real frame time, independently of
playback speed; a large jump or resize zooms out immediately if needed to keep
both ships visible. Manual −/+ zoom suspends automatic fitting until Overview is
selected again. Own ship and Contact keep their selected scale while following
the ship. Automatic zoom pauses during a burn drag so the control stays stable.
Camera changes never alter simulation state or commands.

The control ring is centered on the friendly ship; manipulating it does not
change camera focus or zoom. Its **Burn** handle specifies a maneuver relative to inertial coasting:
direction chooses thrust direction and distance chooses the maneuver amount.
The ring is a control, not a destination at the map's scale. The center means
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
telemetry and preview metrics remain visible throughout. Play from pause, stepping,
and Next turn are disabled during a drag; Pause remains available while running.
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
game.command("next_turn")
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
`game.state().scale` exposes `light_speed_mps`, `railgun_speed_mps`,
`reference_range_m`, and `reference_flight_seconds`. Changing these Lua tables
does not modify the Rust state.
`game.command("reset")` (also `retry`) restores the current scenario at tick zero
with full delta-v and 32× playback while retaining Lua globals. The normal Retry
button is enabled after mission completion. `game.command("sandbox")` selects
the original physics sandbox; `game.command("mission")` starts a new blockade.
The URL query `?sandbox=1` also starts the sandbox for development and regression
testing.
`game.command("step")` retains the diagnostic single-tick (0.25 s) advance;
`game.command("step_forward")` matches the UI's +10 s button. Both require paused
playback and process all physical ticks without crossing a command boundary.
They can start the next interval while staying paused. `play`, `resume`, and
`commit` are equivalent playback commands; repeated calls do not restart an
active interval or change its orders.
Invalid commands,
syntax errors, and runtime errors appear in the console. One evaluation has a
20,000-fuel budget to interrupt accidental infinite loops. Effects performed
before an error or budget exhaustion are retained; there is no transaction rollback.
Input is limited to 4,096 UTF-8 bytes, printed output to 64 lines per evaluation,
each displayed value to 1,024 characters, and visible history to 100 entries.

Mission commands and privileged diagnostics:

```lua
game.fire(game.aim()) -- Queue the observed-lead target at the next boundary
game.command("cancel_shot")
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
tables does not change the simulation. There is no general parameter editor yet.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --example blockade_trials
cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings
python3 scripts/build_web.py
```

Native Rust tests cover inertial motion, acceleration magnitude limits, partial
fuel exhaustion, braking, fast and moving-target contacts, temporal misses,
contact ordering, projectile flight across command intervals, metre-scale misses at
astronomical distances, camera-relative precision, deterministic replay, and
WEGO boundaries at every playback speed,
queued order timing, projectile persistence, reset, and Lua error recovery.
Endpoint-planning tests cover prediction/execution agreement, partial fuel
exhaustion, unreachable targets, and shared Lua validation.
Bevy is a browser-target dependency; native tests need no display server.

For real browser interaction checks:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r requirements-browser.txt
python -m playwright install --with-deps chromium
python scripts/browser_smoke.py
python scripts/mission_smoke.py
```

The sandbox smoke test opens Chromium and fulfills browser requests from `dist/` at a
virtual HTTPS `/phase-vector/` URL. It checks engine startup, a visible scene
change after commands, delta-v consumption, exact interval boundaries, queued
orders, and identical results from playback and manual stepping. A pointer-authored
plan is executed through the actual Play button and its final position, velocity,
and delta-v are compared with the preview; running edits preserve the active burn.
Continuous playback, repeated orders, and the interval pulse are also checked. The test also checks
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
mouse/touch target dragging, queue/cancel, running reservations, boundary firing,
ballistic persistence, no-attack defeat, seven-shot non-hit timeout victory,
terminal locks, retry, and desktop/phone layouts. It also accepts `--url` and
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
  Its 120 s threat horizon is independent of projectile flight time. A conservative
  75 km safety corridor and 120 s committed evasion are tuning assumptions.
- The normal view extrapolates the latest received enemy track. The initial track
  is supplied by the briefing. Lua inspection intentionally exposes true state.
- One constant burn repeats each interval; a shot reservation fires only once.
  Arbitrary arrival times and multiple future reservations remain step 4.
- Projectiles have no expiry or map culling. Contacts consume projectiles and
  record events; hitting the enemy ends the mission with disablement. Terminal
  physics is frozen at the end of the event tick; result time includes its fraction.
- Goal circles have a minimum display radius of 3 px for readability; physical
  entry uses the specified 1,000 km radius. Symbols are also enlarged.
- Mission scoring and paused parameter edits remain step 5. The default balance
  demonstrates placement-dependent denial and is not final difficulty tuning.
- First-load size and performance are not final. Browser errors are shown in the
  page; details are also available in the browser developer console.

## Development conventions

Write README files, design documents, and code comments in English. Reports may
be in Japanese. Use Conventional Commits, keep the implementation small, separate
simulation from presentation, and share the command API between UI and Lua.
Commit `Cargo.lock`; do not commit credentials or generated output.

## License

Copyright (c) 2026 Akihito Kirisaki <kirisaki@klara.works>.

Licensed under either the [MIT license](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.
SPDX: `MIT OR Apache-2.0`.
