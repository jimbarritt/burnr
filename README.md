# burnr

A Rust TUI that visualises LLM token usage as a bonfire. Tokens burned by a
Claude Code session rise as Matrix-style glyph embers and fade out — the more
tokens burned in a turn, the more intense the fire.

## Status

Early development — not yet published. See
[`doc/planning/plan.md`](doc/planning/plan.md) for the implementation plan
and current progress.

## Idea

- Token usage is read live from a Claude Code session's JSONL transcript.
- Each burst of tokens spawns glyph "embers" that float upwards with a
  slightly randomised drift and fade out as they age.
- Fire intensity (particle count, spawn rate, brightness) scales with how
  many tokens were burned.

## Install

Not yet published to crates.io. Once released:

```bash
cargo install burnr
```

## Licence

TBD.
