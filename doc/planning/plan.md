# burnr — Implementation Plan

## What's Next

**Next:** Delta 4.4 — Publish v0.1.0 to crates.io
**Sub-doc:** (none)
**Blockers:** Manual steps for Jim: push to GitHub, `cargo login`, `cargo publish` (blocked from running in-session by policy)

## Summary

| Delta | Task | Status |
|-------|------|--------|
| [Delta 1: Project setup and visual core](#delta-1-project-setup-and-visual-core) | [1.1 Initialise the cargo project and repository scaffolding](#task-11-initialise-the-cargo-project-and-repository-scaffolding) | ✓ DONE |
| | [1.2 Terminal render loop](#task-12-terminal-render-loop) | ✓ DONE |
| | [1.3 Particle system for rising glyphs](#task-13-particle-system-for-rising-glyphs) | ✓ DONE |
| | [1.4 Intensity model and synthetic burst trigger](#task-14-intensity-model-and-synthetic-burst-trigger) | ✓ DONE |
| [Delta 2: Real data ingestion](#delta-2-real-data-ingestion) | [2.1 Investigate Claude Code session log format and location](#task-21-investigate-claude-code-session-log-format-and-location) | ✓ DONE |
| | [2.2 JSONL tailer](#task-22-jsonl-tailer) | ✓ DONE |
| | [2.3 Token event extraction and unit tests](#task-23-token-event-extraction-and-unit-tests) | ✓ DONE |
| [Delta 3: Wire ingestion to visuals](#delta-3-wire-ingestion-to-visuals) | [3.1 Connect the tailer to the fire](#task-31-connect-the-tailer-to-the-fire) | ✓ DONE |
| | [3.2 Tune intensity scaling](#task-32-tune-intensity-scaling) | ✓ DONE |
| | [3.3 Idle behaviour and robustness](#task-33-idle-behaviour-and-robustness) | ✓ DONE |
| [Delta 4: Packaging and publish](#delta-4-packaging-and-publish) | [4.1 CLI arguments](#task-41-cli-arguments) | ✓ DONE |
| | [4.2 Crate metadata, README and licence](#task-42-crate-metadata-readme-and-licence) | ✓ DONE |
| | [4.3 CI and release checks](#task-43-ci-and-release-checks) | ✓ DONE |
| | [4.4 Publish v0.1.0 to crates.io](#task-44-publish-v010-to-cratesio) | TODO |

## Delta 1: Project setup and visual core

### Task 1.1: Initialise the cargo project and repository scaffolding
- DONE — `cargo init --name burnr`; added `ratatui`, `crossterm`, `serde`+derive, `serde_json`, `dirs`. Hello-world binary builds and runs. Module layout in place: `main.rs`, `app.rs`, `fire/`, `ingest/`. Crate metadata left minimal — full metadata is Task 4.2. Licence is already MIT (GitHub set this up when the repo was created). Confirmed `burnr` is unclaimed on crates.io.

### Task 1.2: Terminal render loop
- DONE — `src/app.rs` + `src/main.rs`: alternate screen, raw mode, ~30fps fixed-timestep tick, `q`/Ctrl-C quit, panic hook to restore the terminal on crash. Confirmed live in a tmux pane: renders, quits cleanly, survives resize.

### Task 1.3: Particle system for rising glyphs
- DONE — `src/fire/particle.rs` (single `Particle`: position/velocity/age/lifetime, half-width-katakana+digit glyph set, white→yellow→orange→dim-red colour ramp) and `src/fire/system.rs` (`ParticleSystem`: spawn/update/render to ratatui buffer cells). Ambient trickle (1 particle/tick) wired into `App` for visibility before Task 1.4's real trigger exists. Confirmed live by Jim and Dan — "already awesome", no tuning requested yet.

### Task 1.4: Intensity model and synthetic burst trigger
- DONE — `ParticleSystem::burst(width, tokens)` maps a token count to particle count and rise speed via a log-scaled intensity (`intensity_for_tokens`), so the huge real-world range (hundreds to 100k+ tokens) stays legible rather than one giant cache-read turn swamping the screen. Ambient trickle split out as `spawn_ambient`. Synthetic trigger: press **space** in the running app to fire a burst with a random log-uniform token count (100–~158,000), simulating a real turn. 6 new unit tests (burst scaling, zero-width guard, intensity monotonicity), 20 total passing, clippy clean. Visual sign-off given by Jim: "the fire looks great we can press space and make it burn!" Curve constants left as-is for now; revisit in Task 3.2 against real data if needed.

## Delta 2: Real data ingestion

### Task 2.1: Investigate Claude Code session log format and location
- DONE — Confirmed by inspecting this project's own live transcript. Path is `~/.claude/projects/<slugified-cwd>/<session-uuid>.jsonl`. Only `assistant`-type lines carry `message.usage` with `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`. Key nuance found: streaming duplicates the same `message.id` with identical usage across consecutive lines — must dedupe or it overcounts. Full findings in [`claude-code-log-format.md`](claude-code-log-format.md).

### Task 2.2: JSONL tailer
- DONE — `src/ingest/tailer.rs`: poll-based `Tailer` that follows a growing file from a given offset, buffers partial trailing lines until complete. `src/ingest/session.rs`: `slugify_cwd`, `project_log_dir`, `find_latest_session` for session auto-detection. Unit-tested (append-while-tailing, partial-line buffering, newest-mtime selection).

### Task 2.3: Token event extraction and unit tests
- DONE — `src/ingest/event.rs`: `TokenBurnEvent` domain type, `parse_line` (schema-tolerant, ignores unknown fields, returns `None` for non-assistant/malformed lines), and `EventDeduper` to filter the repeat-snapshot nuance from 2.1. Unit-tested against a real captured transcript line, including the duplicate-message-id case.

## Delta 3: Wire ingestion to visuals

### Task 3.1: Connect the tailer to the fire
- DONE — `src/ingest/watcher.rs`: `spawn_watcher(log_dir)` runs on a background thread, finds the latest session (retrying until one appears), tails it from the current end, dedupes, and sends `TokenBurnEvent`s over an `mpsc` channel. `TokenBurnEvent::total_tokens()` sums input/output/cache-read/cache-creation into one burst magnitude. `App` split into `App::demo()` (old synthetic space-bar trigger, status bar shows `[demo]`) and `App::live(receiver)` (drains real events each tick, bursts on them, no space-bar trigger). `main.rs` checks for a `--demo` arg; otherwise resolves the project's log directory via `dirs::home_dir()` and spawns the watcher, erroring out before touching the terminal if the log dir can't be resolved. 2 new watcher unit tests (using temp dirs, real file writes — no mocks) plus a `total_tokens` test; 23 total passing, clippy clean, rustfmt applied project-wide. Manually verified live: ran the release binary against an isolated fake `$HOME`/throwaway session file in tmux, confirmed ambient trickle at rest and a clearly denser burst immediately after appending a ~90k-token usage line to the tailed file; also reran against this project's own real, currently-growing session transcript with no crash. `--demo` mode reconfirmed working (space bar bursts, `[demo]` status line).

### Task 3.2: Tune intensity scaling
- DONE — Calibrated against real data: analysed ~5,000 deduped assistant turns across all local Claude Code projects (p1 ~19k, median ~75k, p99 ~246k, max ~285k tokens — cache reads dominate). The old `ln(t)/12` curve crushed that whole range into intensity 0.83–1.0, so every real burst looked near-maximal. `intensity_for_tokens` now log-interpolates between an observed floor (10k → 0) and ceiling (300k → 1), spreading the median turn to ~0.6; burst particle count scaled up to `3 + intensity×60` so big turns still blaze. Demo mode's sample range retuned to match (log-uniform ~12.6k–316k). On-screen counter added to the status line: `burned {session total} · last {burst}` with humanised formatting (950 / 24.6k / 1.3M). 4 new unit tests (real-world spread, floor/ceiling saturation, humanise, session accumulation), 27 total passing, clippy clean, rustfmt applied. Verified live in tmux: demo bursts accumulate the counter; live mode attached to this project's real growing transcript and caught a real 40.5k-token turn.

### Task 3.3: Idle behaviour and robustness
- DONE — Idle behaviour: ambient smoulder is now rate-based — `ParticleSystem::spawn_ambient(width, embers_per_second, dt)` resolves fractional expected counts probabilistically so low rates flicker rather than stopping. `App` tracks `last_burst_at`; within `idle.after_seconds` of a burst the ambient runs at `active_embers_per_second` (default 30/s), otherwise it dies down to a gentle flicker at `embers_per_second` (default 4/s). The app starts in flicker until the first burst. All three knobs are parameterisable via a new `~/.config/burnr.toml` (`src/config.rs`, using the `toml` crate): missing or partial file falls back to defaults, unknown fields are ignored, and a malformed file errors out cleanly before the terminal opens (deliberately `~/.config` on all platforms, not `dirs::config_dir()`). Robustness: the watcher now rotates to a newer session file when a fresh session starts, tailing it from the start so early turns aren't lost (puts the previously-dead `Tailer::open_from_start` to use); no-session-found retry and malformed-line tolerance already existed from 3.1/2.3; confirmed no crash on an 8×2 terminal with a burst. 10 new unit tests (37 total passing), clippy clean bar the two pre-existing dead-code warnings, rustfmt applied. Verified live in tmux against an isolated fake `$HOME`: config rates honoured (flicker rate 0 → screen fully still; burst → active smoulder; still again after `after_seconds`), defaults flicker gently with no config file, malformed TOML surfaces a readable parse error.

## Delta 4: Packaging and publish

### Task 4.1: CLI arguments
- DONE — `main.rs` rewritten around a clap derive `Cli`: `--demo` (synthetic mode), `--session <PATH>` (pin to one transcript — new `spawn_session_watcher` in `src/ingest/watcher.rs`, no discovery/rotation, errors before the terminal opens if the file is missing), `--project <DIR>` (watch another project's latest session; path canonicalised so relative dirs slug correctly), plus derived `--help`/`--version`. `--demo`/`--session`/`--project` are mutually exclusive via clap conflicts. `watch_loop` refactored to take `Option<&Path>` for rotation so both watcher variants share it. 2 new tests (pinned watcher ignores newer files; missing file errors); verified live: `--version`, `--help`, conflict error, and missing-session error all correct.

### Task 4.2: Crate metadata, README and licence
- DONE — `Cargo.toml` filled in: description, homepage/repository (github.com/jimbarritt/burnr), `license = "MIT"` (LICENSE file already existed), readme, keywords (claude/tui/terminal/tokens/visualisation), categories (command-line-utilities, development-tools, visualization). README rewritten for release: strapline, install (`cargo install burnr`), usage + all CLI options, status-line explanation (including why burned total outgrows context size), `~/.config/burnr.toml` config reference, how-it-works section. Animated GIF recorded by Jim (screen capture of a live session), cropped and converted with ffmpeg (12fps, 720px, two-pass palette) to `doc/burnr-demo.gif` (3.5MB); README image live and confirmed rendering cleanly in Marq.

### Task 4.3: CI and release checks
- DONE — `.github/workflows/ci.yml`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on ubuntu + macos, plus `cargo publish --dry-run` on ubuntu. To make `-D warnings` pass, the long-standing dead-code warnings were cleared: `ParticleSystem::len`/`is_empty` are now `#[cfg(test)]`, and the unused `claude_projects_dir`/`slugify_cwd` re-exports were dropped from `src/ingest/mod.rs`. All checks verified green locally (39 tests); `cargo publish --dry-run` itself is blocked from running in-session and is verified as part of the manual publish steps in 4.4.

### Task 4.4: Publish v0.1.0 to crates.io
- TODO — Tag v0.1.0, publish to crates.io (see `/publish-crate` skill), verify `cargo install burnr` works on a clean machine, and announce.

---

## Checkpoint: Session 2026-07-02

**What was completed this session:**
- Visual sign-off given on Task 1.4 (intensity model / synthetic burst trigger) — confirmed the fire looks great and space-bar burst works as expected.
- Task 3.1 done: `src/ingest/watcher.rs` background-thread tailer wired into `App` via an `mpsc` channel; `App` split into `App::demo()` / `App::live(receiver)`; `main.rs` gates the two behind a `--demo` arg and resolves the real project log directory otherwise.
- Verified live end-to-end in tmux against an isolated fake `$HOME`/throwaway session (ambient trickle at rest, clear burst on a ~90k-token appended line) and against this project's own real, currently-growing transcript (no crash). `--demo` mode reconfirmed working too.

**State of the project:**
Deltas 1–3.1 complete. The fire is now driven end-to-end by real Claude Code token events when run without `--demo`; the old synthetic trigger survives only behind that flag. 23 tests passing, clippy clean, rustfmt applied project-wide. Three small pre-existing dead-code warnings remain in files untouched this session (`ParticleSystem::len`/`is_empty`, `Tailer::open_from_start`, unused `claude_projects_dir`/`slugify_cwd` re-exports) — harmless, not addressed as out of scope for 3.1.

**Immediate next priorities:**
1. Task 3.2 — Tune intensity scaling against real sessions (the burst-size/speed constants are still first-guess placeholders from 1.4)
2. Task 3.3 — Idle behaviour and robustness (embers-down-to-flicker between turns, session rotation, no-session-found, tiny terminals)
3. Delta 4 — CLI args, crate metadata/README, CI, publish

## Checkpoint: Session 2026-07-06

**What was completed this session:**
- No code changes — reviewed the plan and gave Jim a status summary (Deltas 1–3.1 done, 3.2 next).
- Migrated `plan.md` from the old Phase/Action + ASCII-separator format to the current Delta/Task format with a linked Summary table, per the shared `plan-format` skill convention.

**State of the project:**
Unchanged from the last checkpoint: Deltas 1–3.1 complete and verified live, 23 tests passing, clippy clean. Task 3.2 (intensity tuning) is the next open work.

**Immediate next priorities:**
1. Task 3.2 — Tune intensity scaling against real sessions
2. Task 3.3 — Idle behaviour and robustness
3. Delta 4 — CLI args, crate metadata/README, CI, publish

## Checkpoint: Session 2026-07-06 (later)

**What was completed this session:**
- Task 3.2 done: intensity curve recalibrated against real data. Analysed ~5,000 deduped assistant turns across all local projects (median ~75k tokens, p99 ~246k) and replaced the saturating `ln(t)/12` curve in `src/fire/system.rs` with a log interpolation anchored at 10k (floor) and 300k (ceiling) tokens; burst count raised to `3 + intensity×60`.
- On-screen counter added in `src/app.rs`: status line now shows `burned {session total} · last {burst}` via a `humanise_tokens` helper; demo-mode token sampling retuned to the real-world range.
- 4 new unit tests; verified live in tmux in both demo mode (counter accumulates on space-bar bursts) and live mode (attached to this project's real transcript, caught a genuine 40.5k-token turn).

**State of the project:**
Deltas 1–3.2 complete. Real bursts now spread visibly across the intensity scale instead of all looking maximal, and the status line gives a numeric sanity check. 27 tests passing, clippy clean, rustfmt applied. The three pre-existing dead-code warnings remain (untouched, out of scope).

**Immediate next priorities:**
1. Task 3.3 — Idle behaviour and robustness (embers-down-to-flicker, session rotation, no-session-found, tiny terminals)
2. Delta 4 — CLI args (4.1), crate metadata/README (4.2), CI (4.3), publish v0.1.0 (4.4)

## Checkpoint: Session 2026-07-07

**What was completed this session:**
- Task 3.3 done — idle behaviour: rate-based ambient smoulder (`ParticleSystem::spawn_ambient` now takes embers-per-second + dt with probabilistic fractional spawning); `App` tracks `last_burst_at` and drops from `active_embers_per_second` (default 30/s) to a gentle flicker (`embers_per_second`, default 4/s) once quiet for `idle.after_seconds` (default 10s).
- New `src/config.rs` + `toml` dependency: user config at `~/.config/burnr.toml` with an `[idle]` section (`after_seconds`, `embers_per_second`, `active_embers_per_second`); defaults when missing/partial, unknown fields ignored, malformed file errors before the terminal opens.
- Watcher robustness in `src/ingest/watcher.rs`: rotates to a newer session transcript when one appears, tailing it from the start (uses the previously-dead `Tailer::open_from_start`); unit-tested.
- Verified live in tmux with an isolated `$HOME`: config rates honoured end-to-end, defaults flicker gently, malformed TOML gives a readable error, no crash on an 8×2 terminal.

**State of the project:**
Delta 3 complete — the fire runs end-to-end on real token events, tuned against real data, with configurable idle behaviour and session-rotation robustness. 37 tests passing, clippy clean bar two pre-existing dead-code warnings, rustfmt applied. Only Delta 4 (packaging and publish) remains for v0.1.0.

**Immediate next priorities:**
1. Task 4.1 — CLI arguments (clap: `--session`, `--project`, `--demo`, `--help`/`--version`)
2. Task 4.2 — Crate metadata, README (with GIF) and licence
3. Task 4.3 — CI and release checks
4. Task 4.4 — Publish v0.1.0 to crates.io

## Checkpoint: Session 2026-07-07b

**What was completed this session:**
- Watched burnr live against its own session transcript (very meta) — real bursts confirmed, counter raced past 176k within two turns.
- Task 4.1 done: clap CLI (`--demo`, `--session`, `--project`, `--help`/`--version`) with mutual-exclusion, new pinned `spawn_session_watcher`, `watch_loop` shared between both watcher variants.
- Task 4.2 done: full crates.io metadata in `Cargo.toml` (name `burnr` confirmed still unclaimed), release README rewritten, MIT LICENSE already in place. GIF placeholder commented out — still to record.
- Task 4.3 done: GitHub Actions CI (fmt/clippy -D warnings/test on ubuntu+macos, publish dry-run); dead-code warnings cleared so `-D warnings` passes; 39 tests green locally.

**State of the project:**
Everything for v0.1.0 is code-complete: Deltas 1–3 plus 4.1–4.3. 39 tests passing, clippy clean under `-D warnings`, rustfmt applied, CI workflow in place. Only Task 4.4 remains — the actual publish, which is manual (push to GitHub, `cargo login`, `cargo publish`, verify with `cargo search burnr` / `cargo install burnr`).

**Immediate next priorities:**
1. Task 4.4 — Jim: push to GitHub (repo must be public), `cargo login`, `cargo publish`, verify install on a clean machine
2. Record the README GIF (e.g. with vhs) and uncomment the image line
3. Post-publish future ideas live in Implementation Notes (cost-weighted bursts, replay mode, etc.)

## Implementation Notes

### Architecture

- **TUI stack: ratatui + crossterm.** The most popular, actively maintained modern Rust TUI combination; good support for frame-loop animation, cell-level buffer access (needed for per-glyph colour fades), and cross-platform terminal handling. No strong reason to deviate.
- **Core metaphor.** Tokens burned by Claude Code are rendered as Matrix-style glyph "embers" that spawn low, float upwards with randomised drift, and fade out over their lifetime. Fire intensity (particle count, spawn rate, velocity, brightness) scales with tokens burned in that turn — big burns look like big fires.
- **Data source assumption — needs verification (Task 2.1).** We believe Claude Code writes JSONL session transcripts under `~/.claude/projects/<project-hash>/*.jsonl` containing per-message token usage, but we have not yet inspected the format. Confirming the exact location and schema is the first real-data task; the ingestion layer should be schema-tolerant (ignore unknown fields) to survive drift.
- **Live monitor, not replay.** v0.1 attaches to an in-progress (or most recent) session and animates in real time. Replay/analysis of historical sessions is explicitly out of scope for v0.1 — noted as a future idea only.
- **v0.1 bar: "minimal but real".** Not a synthetic-data prototype — v0.1 must show the basic fire driven by real Claude Code token events and be published on crates.io. Data ingestion (Delta 2) comes early so the whole pipeline is validated before visual polish.
- **Concurrency shape.** One background thread tails the transcript and sends `TokenBurnEvent`s over a channel; the main thread owns the terminal, runs the fixed-timestep tick, and drains the channel each frame. Keeps the render loop simple and lock-free.
- **Intensity curve.** Expect to need a logarithmic (or similar compressive) mapping from token counts to visual intensity — cache reads can be 100k+ tokens per turn while output may be a few hundred.

### Future ideas (not in scope for v0.1)

- Replay mode for historical sessions (fast-forward a whole session's burn).
- Richer visuals: flame body/core rendering, smoke, heat shimmer, colour themes, sound.
- Multiple simultaneous sessions as multiple fires; cost-in-currency overlay; daily/weekly burn statistics.
- **Dan's idea — fire-themed working verbs.** Claude Code shows a random present-participle verb (e.g. "Pondering…", "Marinating…") in its status line while it works. Re-skin these to fit the bonfire theme: "Immolating…", "Combusting…", "Incinerating…", "Vaporising…", "Smouldering…", "Kindling…". This is a Claude Code settings customisation (not a burnr crate feature) — needs the exact settings key confirmed before implementing; candidate for the `update-config` skill. Fun pairing-session touch, not required for v0.1 publish.
- **Toby's idea — float the actual prompt/response text.** Instead of (or alongside) abstract glyph embers, show fragments of the real prompt/response text drifting upward as they're generated — makes the burn visibly tied to *what* was said, not just token counts. Bigger scope than the glyph system: needs to pull message content (not just `usage`) from the transcript, chunk it sensibly (words? lines?), and render readable text inside a particle without wrecking the fire aesthetic. Likely a v0.2+ feature, layered on top of the Delta 1 particle system rather than replacing it.
- **Available tokens / context remaining visualisation.** Separate idea (not yet detailed) about showing remaining context window / available tokens alongside the burn — e.g. the fire dims or the bonfire "runs low on fuel" as the context window fills up, giving a sense of budget remaining rather than just burn rate. Needs the context-window-size side of the transcript/session data investigated (distinct from the per-turn `usage` data Task 2.1 already covered) and a decision on visual treatment before this can be scoped into an action.
