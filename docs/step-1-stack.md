# Step 1: browser and Lua stack

## Decision

Keep Rust and Bevy. Build a single `cdylib` for `wasm32-unknown-unknown`, use Bevy
0.20.1 with an explicit WebGL2/2D feature set, and embed `piccolo` 0.3.3 in the
same WASM module. Keep HTML controls outside the canvas for straightforward text
entry, focus, and accessibility. No server, JavaScript Lua VM, C toolchain, or
additional application framework is required.

## Why this Lua implementation

The [mlua project](https://github.com/mlua-rs/mlua#supported-platforms) documents
WebAssembly support through `wasm32-unknown-emscripten`. That would introduce a
different target and C toolchain from the normal Bevy browser path. We did not
claim that all mlua/browser combinations are impossible or build a full
Emscripten experiment; the documented target mismatch is sufficient to prefer
a smaller integration for this PoC.

[Piccolo](https://github.com/kyren/piccolo/tree/v0.3.3) is a pure Rust, stackless
Lua VM with a fuel-limited executor. Its
[compatibility document](https://github.com/kyren/piccolo/blob/v0.3.3/COMPATIBILITY.md)
describes missing and intentionally unsupported Lua features. It is suitable for
the constrained debugging surface being tested here, subject to later API needs.
It is not a promise of full PUC-Lua or third-party module compatibility.

Both getrandom 0.2 and 0.3 occur in the dependency graph. Their browser backends
must be enabled, and `.cargo/config.toml` selects `wasm_js` for getrandom 0.3.
This was a concrete compiler failure discovered during the integration.

## Build choice

Use Cargo followed by a matching `wasm-bindgen` CLI, plus a small Python copy/build
script. Trunk remains a viable alternative, but provides no necessary capability
for this single-page probe. Pin the bindings crate, and derive the expected CLI
version from the lockfile. CI verifies the upstream release archive checksum.
All site references are relative so the same artifact works under a project path.

## Boundaries

- `probe.rs`: renderer-independent probe state and validation.
- `repl.rs`: persistent Lua VM, copied state inspection, shared command callback,
  printed output, and bounded evaluation.
- `browser.rs`: WASM exports, Bevy window/camera setup, and illustrative gizmos.
- `web/`: accessible HTML controls and a thin JavaScript binding to the WASM API.
- `scripts/`: release packaging and real Chromium interaction checks.

The UI calls `command(name)`, and Lua calls `game.command(name)`. Both ultimately
invoke `ProbeState::command`. `game.state()` creates a detached snapshot; it cannot
mutate authoritative state by editing table fields. The diagnostic counter is
advanced from Bevy's fixed schedule, but is not the future physical simulation.

The host is thread-local because this browser build is intentionally single-threaded.
No shared-memory headers or worker deployment are required on GitHub Pages. This
choice must be revisited if rendering or simulation is later moved to workers.

## Verification and deployment contract

Native tests exercise real Lua, shared commands, validation, and error recovery.
Browser tests operate on the release artifact at `/phase-vector/`, then on the
deployed URL. They check actual canvas changes, not just a successful compilation
or a DOM readiness flag. Desktop and narrow-layout screenshots are retained.

Only successful `main` builds deploy. PRs receive read-only repository permissions
and no Pages write permission. The deployment job uses the tested artifact and
receives only the additional Pages/OIDC permissions it needs. A post-deployment
test failure reports failure; it does not automatically roll back the site.

## Remaining uncertainty

Chromium software rendering establishes basic feasibility, not compatibility with
all browsers or GPUs. VM compatibility, first-load size, and memory behavior need
continued evaluation as the real command API grows. Physics, mission rules,
observation separation, and WEGO restrictions are intentionally outside this probe.
