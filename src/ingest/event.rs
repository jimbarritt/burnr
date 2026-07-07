use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenBurnEvent {
    pub message_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

impl TokenBurnEvent {
    /// Total tokens behind this usage snapshot, used to size the visual
    /// burst. Includes cache reads/writes alongside input/output — cache
    /// activity is still real spend even though it isn't newly generated
    /// text, and in practice it dominates the token count for most turns.
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens + self.cache_creation_tokens
    }
}

#[derive(Debug, Deserialize)]
struct RawLine {
    #[serde(rename = "type")]
    line_type: String,
    #[serde(default)]
    message: Option<RawMessage>,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    id: String,
    #[serde(default)]
    usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize, Default)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

/// Parses one JSONL transcript line into a `TokenBurnEvent`, if it's an
/// assistant line carrying usage. Unknown fields are ignored so the parser
/// tolerates schema drift.
pub fn parse_line(line: &str) -> Option<TokenBurnEvent> {
    let raw: RawLine = serde_json::from_str(line).ok()?;
    if raw.line_type != "assistant" {
        return None;
    }
    let message = raw.message?;
    let usage = message.usage?;
    Some(TokenBurnEvent {
        message_id: message.id,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_tokens: usage.cache_read_input_tokens,
        cache_creation_tokens: usage.cache_creation_input_tokens,
    })
}

/// Filters out repeated usage snapshots for the same message id.
/// Claude Code appends one line per streaming chunk, each carrying a
/// cumulative usage snapshot for the message — most of these are byte-for-byte
/// repeats of the previous snapshot and must not be double-counted.
#[derive(Debug, Default)]
pub struct EventDeduper {
    last_seen: HashMap<String, TokenBurnEvent>,
}

impl EventDeduper {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `Some(event)` if this is a new or changed snapshot for its
    /// message id, `None` if it's a repeat of the last snapshot seen for
    /// that id.
    pub fn dedupe(&mut self, event: TokenBurnEvent) -> Option<TokenBurnEvent> {
        if self.last_seen.get(&event.message_id) == Some(&event) {
            return None;
        }
        self.last_seen
            .insert(event.message_id.clone(), event.clone());
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured (trimmed) from a real Claude Code session transcript.
    const ASSISTANT_LINE: &str = r#"{"type":"assistant","uuid":"aef92612-0d2d-4b2e-84d5-c803fd8dad98","message":{"id":"msg_015V2rZ4s1DXo4dh8m6RCyp2","model":"claude-sonnet-5","usage":{"input_tokens":2,"cache_creation_input_tokens":279,"cache_read_input_tokens":56249,"output_tokens":114,"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":279,"ephemeral_5m_input_tokens":0},"inference_geo":"not_available","iterations":[{"input_tokens":2,"output_tokens":114,"cache_read_input_tokens":56249,"cache_creation_input_tokens":279}],"speed":"standard"}}}"#;

    const NON_ASSISTANT_LINE: &str = r#"{"type":"user","uuid":"6adb7ade-bd98-48a0-962d-ec6d0e0e0e0e","message":{"role":"user"}}"#;

    const MALFORMED_LINE: &str = r#"{"type":"assistant","message":{"id":"msg_broken"#;

    #[test]
    fn parses_usage_from_assistant_line() {
        let event = parse_line(ASSISTANT_LINE).expect("should parse");
        assert_eq!(event.message_id, "msg_015V2rZ4s1DXo4dh8m6RCyp2");
        assert_eq!(event.input_tokens, 2);
        assert_eq!(event.output_tokens, 114);
        assert_eq!(event.cache_read_tokens, 56249);
        assert_eq!(event.cache_creation_tokens, 279);
    }

    #[test]
    fn ignores_non_assistant_lines() {
        assert_eq!(parse_line(NON_ASSISTANT_LINE), None);
    }

    #[test]
    fn ignores_malformed_lines() {
        assert_eq!(parse_line(MALFORMED_LINE), None);
    }

    #[test]
    fn total_tokens_sums_all_four_fields() {
        let event = parse_line(ASSISTANT_LINE).unwrap();
        assert_eq!(event.total_tokens(), 2 + 114 + 56249 + 279);
    }

    #[test]
    fn deduper_drops_identical_repeat_snapshots() {
        let mut deduper = EventDeduper::new();
        let event = parse_line(ASSISTANT_LINE).unwrap();

        assert_eq!(deduper.dedupe(event.clone()), Some(event.clone()));
        // Same message id, identical usage — this is the observed streaming-duplicate case.
        assert_eq!(deduper.dedupe(event.clone()), None);
    }

    #[test]
    fn deduper_passes_through_changed_snapshots_for_same_id() {
        let mut deduper = EventDeduper::new();
        let first = parse_line(ASSISTANT_LINE).unwrap();
        let mut grown = first.clone();
        grown.output_tokens += 50;

        assert_eq!(deduper.dedupe(first.clone()), Some(first));
        assert_eq!(deduper.dedupe(grown.clone()), Some(grown));
    }
}
