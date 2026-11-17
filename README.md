# PHASE VECTOR

A browser-based 2D space tactics proof of concept. Place attacks at an enemy's
future position and prevent a breakthrough through hits, evasive course changes,
and delta-v expenditure.

**[Open the browser probe](https://kirisaki.github.io/phase-vector/)**

## Status

**Step 1: browser and Lua integration.** Bevy renders a geometric reference field
through WebGL2. A Lua REPL runs inside the same WebAssembly module. UI controls and
Lua commands operate on the same Rust state.

This is an integration probe, not a playable mission. The moving marker is an
illustration driven by a tick counter. Physics, WEGO, shooting, enemy AI,
observations, scoring, and gameplay predictions belong to steps 2–6.

See the [PoC specification](docs/poc-spec.md) and
[technical decisions](docs/step-1-stack.md).

## Build and run locally

Install Git, [rustup](https://rustup.rs/), and Python 3.10 or newer. Rust 1.100.0,
`rustfmt`, Clippy, and `wasm32-unknown-unknown` are declared in
`rust-toolchain.toml`. The first build requires access to Rust distribution and
crate servers. Node.js, Trunk, and an Emscripten toolchain are not required.

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

## Probe controls

| Control | Behavior |
| --- | --- |
| Resume / Pause | Start or stop the probe counter; rendering continues |
| Step +1 | Advance one probe tick while paused |
| Reset | Restore the paused probe at tick zero; Lua globals remain |
| Run Lua | Evaluate the input; Ctrl+Enter or Command+Enter also submits |
| Example buttons | Fill the console input without executing it |
| Reload page | Start a new probe and Lua session |

The probe starts paused and advances at 60 ticks per second when running. These
ticks are an integration aid, not the mission's final timestep or command cycle.

```lua
return 1 + 2
answer = 6 * 7; print("Answer:", answer)
return answer
return game.state().ticks
game.command("pause")
game.command("step"); return game.state().ticks
game.command("resume")
game.command("reset")
```

Bare expressions also work. `game.state()` returns a copy with `paused` and
`ticks`; modifying that table does not change the Rust state. Invalid commands,
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

Native Rust tests cover real Lua evaluation, persistent globals, UI/REPL command
consistency, pause validation, snapshot isolation, and recovery from errors and
an infinite loop. Bevy is a browser-target dependency; native tests do not need a
display server or graphics libraries.

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
change after commands, Lua results and persistence, UI/REPL state sharing,
validation, error recovery, and a narrow viewport. Screenshots and errors are
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
- The counter and visual movement do not establish deterministic game physics.
  The 120-second WEGO cycle and command-phase restrictions are future work.
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
