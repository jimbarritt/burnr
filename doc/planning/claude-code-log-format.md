# Claude Code session log format (Action 2.1 findings)

Investigated by inspecting this project's own live transcript file while pairing.

## Location

```
~/.claude/projects/<slugified-cwd>/<session-uuid>.jsonl
```

- `<slugified-cwd>` is the absolute working directory with `/` replaced by `-`
  (e.g. `/Users/jmdb/Code/github/jimbarritt/burnr` →
  `-Users-jmdb-Code-github-jimbarritt-burnr`).
- The same directory also contains a subdirectory named after the session
  UUID (holding unrelated session state) and a `memory/` directory — the
  tailer should only glob `*.jsonl` files directly inside the project
  directory, not recurse.
- The most recently modified `*.jsonl` file in the current project's
  directory is the current/most recent session.

## Line format

Newline-delimited JSON. One JSON object per line. Relevant top-level fields:

```json
{
  "type": "assistant",
  "sessionId": "...",
  "uuid": "...",
  "timestamp": "...",
  "cwd": "...",
  "message": { "id": "msg_...", "model": "claude-sonnet-5", "usage": { ... } }
}
```

Observed `type` values: `assistant`, `user`, `system`, `attachment`,
`file-history-snapshot`, `ai-title`, `last-prompt`, `mode`,
`permission-mode`, `queue-operation`. **Only `assistant` lines carry token
usage.**

## Token usage shape

`message.usage` on an `assistant` line:

```json
{
  "input_tokens": 2,
  "cache_creation_input_tokens": 223,
  "cache_read_input_tokens": 56528,
  "output_tokens": 226,
  "server_tool_use": { "web_search_requests": 0, "web_fetch_requests": 0 },
  "service_tier": "standard",
  "cache_creation": { "ephemeral_1h_input_tokens": 223, "ephemeral_5m_input_tokens": 0 },
  "inference_geo": "not_available",
  "iterations": [ { "type": "message", "input_tokens": 2, "output_tokens": 226, "...": "..." } ],
  "speed": "standard"
}
```

Field names match the planned `TokenBurnEvent` almost exactly:
`input_tokens`, `output_tokens`, `cache_read_input_tokens`,
`cache_creation_input_tokens`. Extra fields (`service_tier`, `cache_creation`
breakdown, `inference_geo`, `iterations`, `speed`) should be ignored by a
schema-tolerant parser rather than causing failures.

## Important nuance: duplicate lines per message

**The same `message.id` appears on multiple consecutive lines with
identical `usage` values** (observed directly in this session's transcript
— streaming responses append a snapshot line per chunk, all carrying the
same cumulative usage for that message). A naive tailer that sums
`output_tokens` per line will drastically overcount.

**Required handling:** key events by `message.id` and only count a given
id's usage once — either take the latest line per id as the authoritative
snapshot, or diff against the previously seen value for that id and ignore
zero-delta repeats.

## Session auto-detection

`sessionId` and `cwd` are present on every line, so a tailer pointed at a
project directory can confirm it's reading the right file, and "current
session" = newest-mtime `*.jsonl` in the slugified-cwd directory.
