# Step 5: diagnostics, evaluation, and reproducible retry

Step 5 adds bounded paused parameter editing and a terminal score to the existing
Lua inspection, shared command API, pause/step controls, and Retry flow. The
implementation stays in one Rust crate with plain TypeScript presentation modules.
Default physics, weapon settings, enemy policy, and win conditions are unchanged.

## Diagnostic configuration

`game.parameters()` returns a detached table of the current settings.
`game.configure(name, value)` sends `Command::Configure` through `Session::dispatch`,
which also owns normal UI and Lua orders. Names, units, ranges, and examples are
listed in the [README](../README.md#mission-controls). The API has seven settings:

- Initial scheduling: deadline, reload, and sensor processing time. These require
  paused playback, no execution yet, and an empty reservation queue. Values must
  lie exactly on the physical tick grid. This prevents retroactive cooldown,
  observation delivery, and locked-shot changes.
- AI policy: reaction horizon, safety radius, hold duration, and recovery time.
  These can change at any paused instant before a result. A real policy change
  releases the old hold and is evaluated on the next scheduled one-second AI
  decision tick. It does not immediately advance time or invent observations.

All validation occurs before mutation. Unknown parameters, invalid values, running
edits, and post-result edits return errors. One call is atomic; a sequence of Lua
calls retains earlier successful effects if a later call fails. Applying the same
value is a no-op. The bounds cap diagnostic work and are not recommended difficulty
presets. Physics tick size, command duration, ship properties, and muzzle speed
remain compile-time scenario settings.

The normal shot and burn commands retain their existing command-boundary rules.
Diagnostic edits are explicitly separate: they never rewrite command history,
active burns, pending shot targets/ticks, or flying projectile velocities.

## Initial conditions and run identity

Before execution, accepted edits update both the current configuration and the
Retry baseline. After execution has begun, edits update only the current run.
`mission.initial_parameters` exposes the baseline; `mission.parameter_edits` lists
the tick, parameter name, new value, and whether execution had begun for each real
change. Repeating the same initial conditions, gameplay commands, and diagnostic
edits at the same ticks reproduces the result. There is no general replay-file
format; the existing command log alone does not encode diagnostic changes.

The UI and Lua identify the run as:

| Kind | Meaning |
| --- | --- |
| Standard | Standard initial world and mission configuration; no mid-play edits |
| Custom | Nonstandard initial world or configuration; no mid-play edits |
| Debug | At least one accepted mid-play parameter change, even if later undone |

Retry reconstructs the initial world and baseline configuration, clears all
reservations, flying rounds, command history, and edit history, restores the
initial observations, and starts paused at the initial tick with 32x playback. Standard browser scenarios
start at tick zero. Custom baselines remain custom; transient debug changes are
discarded. The existing UI confirmation clears draft controls when accepted and
preserves/resumes them when cancelled. Lua globals survive reset.
`game.command("mission")` explicitly returns to the standard blockade.

## Score contract

`evaluation::evaluate` is the sole scoring implementation. The normal presentation
and Lua both consume it; TypeScript only renders values. While the mission is
running, evaluation is absent. After a result it uses the frozen world:

| Component | Victory | Defeat |
| --- | --- | --- |
| Victory points | 1,000 for either disablement or timeout | 0 |
| Shot economy | max(0, 500 - 20 times actual launches) | 0 |
| Fuel economy | floor(500 times remaining friendly delta-v / initial friendly delta-v), clamped to 0–500 | 0 |

Zero initial delta-v awards zero fuel points. The total is the sum of these three
components. Actual launches and delta-v expenditure are reported even on defeat.
Unfired reservations and cancellations do not cost points. There is no separate
kill, speed, damage, or enemy-fuel bonus. Default successful runs therefore score
between 1,000 and 2,000, and every defeat scores zero.

The final world is frozen at the end of the event's physical tick, so fuel usage
includes that complete tick even when the displayed event time is fractional.
Changing playback speed or dismissing the result cannot change the score.
Custom/debug runs retain the same arithmetic but are visibly identified and are
not comparable to standard runs. There is no leaderboard or persistent score store.

The result dialog shows total, components, shots, fuel, time, reason, and run kind.
The inline result retains the total after dismissal. Short landscape viewports
use two columns to keep the result heading and actions visible together. Lua's
`game.state().mission.evaluation` exposes the same numeric fields; diagnostic
settings and true enemy state do not enter normal prediction rendering.

## Verification

- `tests/parameters.rs`: atomic rejection, paused-only changes, empty-queue and
  first-command guards, immutable orders/projectiles, deterministic continuation,
  detached Lua values, edit history, Retry baselines, and standard restoration.
- `tests/evaluation.rs`: real timeout and disabling-hit outcomes, zero-point
  defeat, unlaunched reservations, fuel normalization, bonus limits, zero initial
  fuel, stable terminal scores, stepping equivalence, and UI/Lua agreement.
- `scripts/diagnostic_checks.py`, called by the CI mission smoke test: visible Lua
  settings/errors, custom/debug identification, score breakdown, retained inline
  score, and baseline restoration on desktop, portrait, and landscape layouts.
- Existing browser checks retain coverage of planning, observation-based aiming,
  touch, Auto/Commit/Skip, results, and in-game Retry confirmation.
- Current placement trials: no attack and repeated lead shots
  lose (0 points); eight spread shots win without hits (1,840 points) at the
  current 1,215-second deadline; twelve spread shots win without hits (1,760 points);
  sixteen spread shots win without hits (1,680 points).

Chromium interaction checks exercise the built artifact under the GitHub Pages
project path and the Vite development build. They do not establish compatibility
with every physical phone or Safari. Score weights and difficulty remain provisional;
step 6 is the remaining tuning/playtesting milestone. Closest-approach feedback,
general scenario editing, saved replays, and a full modding framework are excluded.
