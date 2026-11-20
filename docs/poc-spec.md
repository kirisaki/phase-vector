# PHASE VECTOR PoC specification

## Goal

Build a browser-based 2D space tactics game in which the player places attacks at
an enemy's predicted future positions. A successful blockade may disable the
enemy or delay its arrival beyond a deadline through forced evasive maneuvers and
delta-v expenditure.

This document records the requested behavior, not completed functionality.
The current implementation includes step-3 blockade rules and observation-limited
enemy control, connected to the browser and Lua. See [blockade implementation](step-3-blockade.md)
for adopted rules, tuning, and verification. Later milestone requirements below
remain planned unless explicitly described as implemented.

An early subset of step 4 now provides friendly 120-second endpoint planning,
a three-boundary route forecast, arrival-velocity previews, and direct map manipulation with a ship-centered
burn control. This does not
implement a variable-time slider or arbitrary-arrival-time shot scheduling.
Step 3 adds an observed constant-velocity enemy forecast and a reload-slot
gun solver whose target is a world point and whose flight time is solved.

## Scope and implementation approach

- Prefer Rust and Bevy, subject to a minimal browser and embedded Lua feasibility
  check. Explain any necessary stack changes with evidence.
- Start with one crate. Keep deterministic simulation logic independent of the
  renderer and avoid a generalized framework or complete modding platform.
- Use geometric tactical-map visuals. Art assets are not needed for the PoC.
- Write README files, design documents, and code comments in English.
- Implement, verify, and briefly report each authorized milestone. Record
  assumptions and continue without routine confirmation when no major issue
  prevents progress.

## Physics and time

- Two spatial dimensions, no gravity, inertial motion, bounded acceleration, and
  a finite remaining delta-v budget.
- A fixed simulation timestep independent of rendering and playback speed.
- Reproducible results from the same initial state and commands. Any randomness
  must be explicitly seeded and recorded.
- Continuous or swept collision handling to prevent fast projectiles from
  tunneling. A hit requires the trajectories to intersect at the same time, not
  merely cross the same map location at different times.
- WEGO execution: each applied order advances all actors simultaneously for
  **120 simulated seconds**. Play continues across boundaries, repeating the
  last thrust vector until changed; Pause provides unlimited planning time.
- Draft orders may be edited while running or paused. Changes apply only at the
  next 120-second command boundary; an active interval's order is immutable.
- A ring around the friendly ship signals each applied command interval,
  including coasting. Commit plays at the selected speed to the next boundary
  and pauses; Skip immediately simulates to that boundary and pauses.

## Blockade mission

- One friendly ship, one enemy ship that does not return fire, and one unguided
  gun type.
- Unlimited ordinary ammunition, a reload interval, and a hit that disables the
  enemy.
- Defeat if the enemy reaches the designated region within the mission deadline.
- Victory if the enemy is disabled or the deadline passes without a breakthrough.
- Delta-v exhaustion and temporary deviation from the mission route are not
  victory conditions on their own.
- The enemy evaluates only attacks it has observed, evades them, and attempts to
  return to its mission route.
- Keep authoritative world state and observations separate. AI decisions must
  not read hidden attacks, and displayed enemy predictions must derive from
  observations rather than privileged future state.

## Planning and presentation

- A geometric tactical map, a time slider, velocity indicators, predicted ship
  paths, and projectile trajectories.
- Player maneuver planning.
- A shot is specified by a future position and arrival time. Solve for launch
  time and direction using fixed muzzle speed and the firing ship's velocity at
  launch in the shared mission frame.
  Explicitly identify unreachable requests.
- Recalculate pending shots when the friendly maneuver plan changes.
- Preserve unlaunched reservations and in-flight projectiles across command
  intervals. A launched projectile retains its ballistic trajectory.
- Present enemy future positions as observation-based predictions, not promises
  of what the enemy will actually do.

## Lua REPL and evaluation

- Inspect authoritative state, observations, and AI decision explanations through
  the developer REPL. Privileged inspection is diagnostic only and must not leak
  into enemy logic or normal prediction displays.
- Support pause, simulation stepping, reset, and parameter changes while paused.
- Use the same gameplay command API for the UI and REPL, including command-phase
  validation. Debug parameter edits are distinct from gameplay orders.
- Display mission outcome, a score, and a breakdown including shots fired and
  friendly delta-v expenditure.
- Provide a retry action that restores a reproducible initial scenario.

## Initial tuning targets

- Current trial: **240,000 km** initial separation and **660 km/s** reference
  muzzle speed, giving a stationary-target flight time of about **363.636 s**.
  This supersedes the earlier near-light-speed trial. See [combat scale](combat-scale.md).
- Friendly/contact initial speeds: **75 / 37.5 km/s**; acceleration limit **5 G**
  (49.03325 m/s²) and initial delta-v **60 km/s** for each ship.
- Projectile flight-time tuning band: roughly **240–480 simulated seconds**.
- Around **10 command opportunities** per mission.
- Enemy reaction lead time adjustable independently of projectile flight time,
  subject to light propagation and the observation rules. Light crosses the
  initial range in about 0.801 s. Step 3 uses 1-second samples and processing,
  plus quantized light delay; a 120-second threat horizon controls enemy reaction.
- Tune parameters against measurable mission outcomes rather than a scripted
  success path.

## Milestones and acceptance criteria

| Step | Deliverable | Verification |
| --- | --- | --- |
| 0 | Public repository, dual license, Rust scaffold, pinned tooling, CI, and requirements | Native execution, formatting, Clippy, test harness, and WebAssembly compilation |
| 1 | Minimal browser application and embedded Lua feasibility spike | Load the app in an actual browser and evaluate a Lua expression; document stack decisions |
| 2 | Physics and WEGO time progression | Reproducibility, acceleration and delta-v limits, swept collision cases, and command-phase enforcement |
| 3 | Blockade mission and observation-limited enemy | Breakthrough, disablement, timeout, limited observation, evasion, and route recovery |
| 4 | Prediction and planning UI | Feasible and infeasible shots, maneuver recalculation, and persistence across command intervals |
| 5 | REPL, scoring, and retry | Shared command validation, inspection, pause/step/reset, paused parameter edits, and score breakdown |
| 6 | Tuning and browser play verification | No-attack breakthrough, different outcomes from equal shot counts with different placement, and prevention without a disabling hit |

Step 6 must include real browser interaction and README updates with launch
instructions, controls, adopted assumptions, and known limitations. Automated
compilation checks alone do not satisfy this milestone.

## Initial assumptions and open decisions

- The repository is `kirisaki/phase-vector`, public, with `main` as the default
  branch. Publishing the source does not automatically deploy a playable site.
- Rust 1.100.0 is the initial pinned development compiler because a local
  installation includes the required tools and WebAssembly standard library.
- The authoritative simulation is local and single-player; networking is outside
  the current scope.
- Step 1 selects Bevy 0.20.1, piccolo 0.3.3, wasm-bindgen, and WebGL2. See
  [the stack decision](step-1-stack.md) for tradeoffs and verification limits.
- Step 2 selects 0.25-second ticks, SI units, `f64` simulation state, and swept
  circle contacts. See [physics and time](step-2-physics.md). Step 3 defines
  mission geometry, observations, AI policy, and the basic gun solver in
  [blockade rules](step-3-blockade.md). Arbitrary-arrival scheduling and scoring
  remain open.
- Reproducibility must be tested independently of rendering and playback speed.
  Bit-identical results across browsers and architectures are not claimed until
  explicitly verified.
- About ten 120-second intervals suggests an initial mission duration near
  1,200 seconds. Step 3 adopts this provisional deadline.
- Exact ordering for arrival, impact, and timeout at the same simulation instant
  is implemented and tested: earlier event wins, exact hit/entry ties favor
  disablement, and entry/impact at the deadline precedes timeout.

## Out of scope

Warp, multi-ship fleets, guided weapons, detailed damage, campaigns, and a complete
modding framework.
