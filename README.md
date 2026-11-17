# PHASE VECTOR

A browser-based 2D space tactics proof of concept. Place attacks at an enemy's
future position and prevent a breakthrough through hits, evasive course changes,
and delta-v expenditure.

## Status

**Step 0: repository setup.** This repository currently contains a minimal Rust
executable, a pinned toolchain, native and WebAssembly build checks, and the PoC
specification. Running the executable prints a setup message; there is no playable
game or browser interface yet.

Rust with Bevy is the first-choice implementation stack. Browser execution and an
embedded Lua REPL will be evaluated together in step 1 before adding those
dependencies. Trunk is a candidate for browser builds and local serving; it is not
required for this scaffold. Node.js is not currently required.

See [the PoC specification](docs/poc-spec.md) for requirements, assumptions, and
acceptance criteria.

## Prerequisites

- Git.
- [Rustup](https://rustup.rs/) and access to the Rust distribution servers for the
  first toolchain installation.
- Rust **1.100.0**, `rustfmt`, Clippy, and the `wasm32-unknown-unknown` standard
  library. These are declared in `rust-toolchain.toml` and installed by rustup
  when a Rust command is first run in the repository.

The version is pinned for development and CI consistency; it is not a claim that
older Rust versions are incompatible with this scaffold.

## Get started

```sh
git clone https://github.com/kirisaki/phase-vector.git
cd phase-vector
rustup show active-toolchain
cargo run --locked
```

Expected output:

```text
PHASE VECTOR: repository setup complete; gameplay is not implemented yet.
```

## Checks

Run these from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release --target wasm32-unknown-unknown
cargo run --locked
```

GitHub Actions runs the same checks on pushes to `main` and pull requests.
`Cargo.lock` is committed so future dependency resolution can be reproduced.
There are no gameplay tests yet; the current test command checks that the test
harness builds and runs.

The WebAssembly artifact is written to
`target/wasm32-unknown-unknown/release/phase-vector.wasm`. This is only a compilation
check. Browser loading, rendering, JavaScript integration, and Lua execution are
not validated by this build. Browser launch instructions and controls will be
added once step 1 provides an actual browser application.

## Repository layout

```text
.github/workflows/ci.yml  Native and WebAssembly checks
docs/poc-spec.md          Requirements, assumptions, and milestone acceptance
src/main.rs              Minimal executable entry point
AGENTS.md                Instructions for coding agents
Cargo.toml               Package metadata and dependencies
Cargo.lock               Locked dependency resolution
rust-toolchain.toml      Pinned compiler, tools, and WebAssembly target
LICENSE-MIT              MIT license
LICENSE-APACHE           Apache License 2.0
```

## Development conventions

- Write documentation and code comments in English. Progress reports may be in
  Japanese.
- Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/), for
  example `chore: bootstrap repository` or `feat: add fixed-step simulation`.
- Keep the implementation small and document assumptions before relying on them.
- Separate simulation from presentation, and share the command API between the
  normal UI and the eventual Lua REPL.
- Keep credentials, local environment files, and generated output out of Git.
- Complete each milestone's relevant checks and report limitations before moving
  to the next authorized milestone.

## License

Copyright (c) 2026 Akihito Kirisaki <kirisaki@klara.works>.

Licensed under either of the following, at your option:

- [MIT license](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

The SPDX license expression is `MIT OR Apache-2.0`.
