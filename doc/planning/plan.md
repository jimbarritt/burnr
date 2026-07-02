# burnr — Implementation Plan

## ── WHAT'S NEXT ──────────────────────────────────────────────────────────
**Next:** Action 1.1 — Initialise the cargo project and repository scaffolding
**Sub-doc:** (none)
**Blockers:** None
─────────────────────────────────────────────────────────────────────────────

## Phase 1: Project setup & visual core

### Action 1.1: Initialise the cargo project and repository scaffolding
- TODO — `cargo init --name burnr`, add `.gitignore`, wire up `ratatui` + `crossterm` dependencies, and confirm a hello-world binary builds and runs. Set edition, minimal metadata, and organise a starter module layout (`main.rs`, `app.rs`, `fire/`, `ingest/`).

### Action 1.2: Terminal render loop
- TODO — Build the core TUI frame loop: enter/leave alternate screen, raw mode, graceful shutdown on `q`/Ctrl-C, and a fixed-timestep tick (~30fps) driving a `draw()` call. Handle terminal resize.

### Action 1.3: Particle system for rising glyphs
- TODO — Implement the ember particle model: each particle is a Matrix-style glyph (katakana/half-width kana plus digits) with position, upward velocity, slight randomised horizontal drift, age, and lifetime. Particles spawn near the bottom of the screen, float upwards, and fade out (colour ramp e.g. white → yellow → orange → dim red → gone) as they age. Update physics each tick; render via ratatui buffer cells.

### Action 1.4: Intensity model and synthetic burst trigger
- TODO — Define a `burst(tokens: u64)` entry point that maps a token count to spawn rate, particle count, velocity, and brightness (bigger burns → taller, denser, brighter fire). Hardcode a synthetic trigger (e.g. keypress or timer emitting random token counts) to prove the animation pipeline end-to-end before real data exists.

## Phase 2: Real data ingestion

### Action 2.1: Investigate Claude Code session log format and location
- TODO — Inspect the local JSONL transcripts Claude Code writes (believed to be under `~/.claude/projects/<project-hash>/*.jsonl` — needs confirmation). Document the actual schema: where token usage appears (input/output/cache tokens per assistant message), how sessions are identified, and how "the current/most recent session" can be detected. Capture findings in a sub-doc under `doc/planning/`.

### Action 2.2: JSONL tailer
- TODO — Build a file tailer that follows a growing JSONL file (poll or `notify`-based), parses newly appended lines with `serde_json`, tolerates partial lines and unknown fields, and emits structured token-usage events. Include session auto-detection: pick the most recently modified transcript for the current project (or a given path).

### Action 2.3: Token event extraction and unit tests
- TODO — Map raw log entries to a clean `TokenBurnEvent { input, output, cache_read, cache_creation, timestamp }` domain type. Unit-test against captured sample lines from real transcripts so schema drift is caught by tests, not at runtime.

## Phase 3: Wire ingestion to visuals

### Action 3.1: Connect the tailer to the fire
- TODO — Run ingestion on a background thread/channel feeding the render loop; each `TokenBurnEvent` triggers a `burst()` sized by tokens burned. Remove/feature-gate the synthetic trigger (keep it behind a `--demo` flag for development and screenshots).

### Action 3.2: Tune intensity scaling
- TODO — Calibrate the token-count → visual-intensity curve against real sessions (likely logarithmic, since turns range from hundreds to hundreds of thousands of tokens with cache reads). Ensure a quiet session smoulders and a huge turn produces a satisfying blaze without swamping the terminal. Add a simple on-screen counter (tokens this session / last burst) as a sanity check.

### Action 3.3: Idle behaviour and robustness
- TODO — Decide and implement what the fire does between turns (embers die down to a gentle flicker), and harden against real-world conditions: session file rotation/new session started, no session found, malformed lines, very small terminals.

## Phase 4: Packaging & publish

### Action 4.1: CLI arguments
- TODO — Add a minimal CLI (`clap`): auto-detect the latest session by default, `--session <path>` to watch a specific transcript, `--project <dir>` to scope detection, `--demo` for synthetic mode, `--help`/`--version`.

### Action 4.2: Crate metadata, README and licence
- TODO — Fill in `Cargo.toml` (description, keywords, categories, repository, licence — MIT or Apache-2.0/MIT dual), write a README with an animated GIF/screenshot of the fire, install instructions (`cargo install burnr`) and usage. Add the licence file(s).

### Action 4.3: CI and release checks
- TODO — GitHub Actions workflow: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` on Linux and macOS. Verify `cargo publish --dry-run` passes.

### Action 4.4: Publish v0.1.0 to crates.io
- TODO — Tag v0.1.0, publish to crates.io (see `/publish-crate` skill), verify `cargo install burnr` works on a clean machine, and announce.

---

## Implementation Notes

### Architecture

- **TUI stack: ratatui + crossterm.** The most popular, actively maintained modern Rust TUI combination; good support for frame-loop animation, cell-level buffer access (needed for per-glyph colour fades), and cross-platform terminal handling. No strong reason to deviate.
- **Core metaphor.** Tokens burned by Claude Code are rendered as Matrix-style glyph "embers" that spawn low, float upwards with randomised drift, and fade out over their lifetime. Fire intensity (particle count, spawn rate, velocity, brightness) scales with tokens burned in that turn — big burns look like big fires.
- **Data source assumption — needs verification (Action 2.1).** We believe Claude Code writes JSONL session transcripts under `~/.claude/projects/<project-hash>/*.jsonl` containing per-message token usage, but we have not yet inspected the format. Confirming the exact location and schema is the first real-data task; the ingestion layer should be schema-tolerant (ignore unknown fields) to survive drift.
- **Live monitor, not replay.** v0.1 attaches to an in-progress (or most recent) session and animates in real time. Replay/analysis of historical sessions is explicitly out of scope for v0.1 — noted as a future idea only.
- **v0.1 bar: "minimal but real".** Not a synthetic-data prototype — v0.1 must show the basic fire driven by real Claude Code token events and be published on crates.io. Data ingestion (Phase 2) comes early so the whole pipeline is validated before visual polish.
- **Concurrency shape.** One background thread tails the transcript and sends `TokenBurnEvent`s over a channel; the main thread owns the terminal, runs the fixed-timestep tick, and drains the channel each frame. Keeps the render loop simple and lock-free.
- **Intensity curve.** Expect to need a logarithmic (or similar compressive) mapping from token counts to visual intensity — cache reads can be 100k+ tokens per turn while output may be a few hundred.

### Future ideas (not in scope for v0.1)

- Replay mode for historical sessions (fast-forward a whole session's burn).
- Richer visuals: flame body/core rendering, smoke, heat shimmer, colour themes, sound.
- Multiple simultaneous sessions as multiple fires; cost-in-currency overlay; daily/weekly burn statistics.
