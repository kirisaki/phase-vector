# PHASE VECTOR

A browser-based 2D space tactics proof of concept. Place attacks at an enemy's
future position and prevent a breakthrough through hits, evasive course changes,
and delta-v expenditure.

**[Open the physics sandbox](https://kirisaki.github.io/phase-vector/)**

## Status

**Step 2: physics and WEGO execution.** Two ships move with inertia, bounded
acceleration, and finite delta-v. Plan a constant thrust vector, then commit a
120-second interval. Bevy draws the authoritative physical state; the UI and Lua
use the same validated command API.

This is a physics sandbox. The second ship coasts without an AI. Mission rules,
weapons, observations, scoring, and gameplay predictions remain steps 3–6.
Projectile contacts are implemented and tested in the physics core; firing UI
and hit consequences are not implemented yet.

See the [PoC specification](docs/poc-spec.md),
[stack decisions](docs/step-1-stack.md), and
[physics and time model](docs/step-2-physics.md).

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
wait for it to finish, then reload the page. This requires the Rust toolchain and
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

## Sandbox controls

| Control | Behavior |
| --- | --- |
| Acceleration X/Y + Apply | Draft a constant vector in m/s² for the next interval; magnitude is clamped to 0.02 |
| Commit 120 s | Freeze orders and execute both ships simultaneously |
| Resume / Pause | Control playback during execution; orders remain locked |
| +0.25 s | Advance one physical tick during paused execution |
| Playback speed | Select 1×, 4×, 16×, or 64× without changing physical results |
| Reset | Restore planning at tick zero and full delta-v; Lua globals remain |
| Show / Hide Lua console | Toggle the initially hidden REPL; input, output, and Lua globals are retained. Close or Escape returns focus to the toggle |
| Run Lua | Evaluate input; Ctrl+Enter or Command+Enter also submits |
| Example buttons | Fill the console input without executing it |
| Reload page | Start a new simulation and Lua session |

Planning has no time limit. Every committed interval ends at exactly 120 simulated
seconds and returns to planning. The next draft defaults to coasting. Position,
velocity, and remaining delta-v persist. Edited input fields must be applied
before the UI permits committing. Planning commands are rejected during execution,
including while paused. Coordinates use metres with positive Y pointing upward.
The grid spacing is 100 m; velocity arrows show 50 seconds of displacement.

```lua
answer = 6 * 7; print("Answer:", answer)
return game.state().phase
return game.state().ship.remaining_dv
game.burn(0.02, 0)
game.command("commit")
game.command("pause")
game.command("step"); return game.state().time_seconds
game.speed(64)
game.command("resume")
game.command("reset")
```

Bare expressions also work. `game.state()` returns a detached snapshot with
`phase`, `paused`, `ticks`, `time_seconds`, `remaining_seconds`, `speed`,
`command_number`, `command_count`, `planned_burn`, and `ships`. `ship` aliases the
friendly ship snapshot. Ship fields include `id`, `x`, `y`, `vx`, `vy`,
`remaining_dv`, `spent_dv`, and `max_acceleration`. Projectile and impact counts
are also exposed. Changing these Lua tables does not modify the Rust state.
Invalid commands,
syntax errors, and runtime errors appear in the console. One evaluation has a
20,000-fuel budget to interrupt accidental infinite loops. Effects performed
before an error or budget exhaustion are retained; there is no transaction rollback.
Input is limited to 4,096 UTF-8 bytes, printed output to 64 lines per evaluation,
each displayed value to 1,024 characters, and visible history to 100 entries.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings
python3 scripts/build_web.py
```

Native Rust tests cover inertial motion, acceleration magnitude limits, partial
fuel exhaustion, braking, fast and moving-target contacts, temporal misses,
contact ordering, deterministic replay, WEGO boundaries at every playback speed,
paused command locks, projectile persistence, reset, and Lua error recovery.
Bevy is a browser-target dependency; native tests need no display server.

For real browser interaction checks:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r requirements-browser.txt
python -m playwright install --with-deps chromium
python scripts/browser_smoke.py
```

The smoke test opens Chromium and fulfills browser requests from `dist/` at a
virtual HTTPS `/phase-vector/` URL. It checks engine startup, a visible scene
change after commands, delta-v consumption, exact interval boundaries, locked
orders, and identical results from playback and manual stepping. It also checks
Lua persistence, validation, error recovery, and a narrow viewport. Screenshots and errors are
written to ignored `artifacts/`. `--executable-path` can select an existing
Chromium installation. To exercise the real hosted site:

```sh
python scripts/browser_smoke.py --url https://kirisaki.github.io/phase-vector/
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
- Step 2 supports one constant burn per interval. There is no mission deadline,
  enemy evasion, firing UI, victory condition, or observation model yet. The fixed
  map can be left after many intervals; telemetry continues to show the state.
- Projectiles have no expiry or map culling. Contacts consume projectiles and
  record events; ship disablement will be defined by step-3 mission rules.
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
