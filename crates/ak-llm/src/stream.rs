//! SSE helpers for OpenAI-compatible and Anthropic streaming responses.

use serde::Deserialize;

/// Parse one SSE `data:` payload from an OpenAI chat completion stream.
/// Returns `Some(delta)` for a text delta, `None` for keep-alives / role chunks,
/// and `Err(())` for `[DONE]`.
pub fn openai_sse_delta(data: &str) -> Result<Option<String>, ()> {
    let data = data.trim();
    if data.is_empty() || data == ": keep-alive" {
        return Ok(None);
    }
    if data == "[DONE]" {
        return Err(());
    }
    let parsed: OpenAiStreamChunk = serde_json::from_str(data).map_err(|_| ())?;
    let delta = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.delta.content)
        .filter(|s| !s.is_empty());
    Ok(delta)
}

/// Parse one Anthropic SSE data payload; returns text deltas only.
pub fn anthropic_sse_delta(data: &str) -> Option<String> {
    let data = data.trim();
    if data.is_empty() {
        return None;
    }
    let parsed: AnthropicStreamEvent = serde_json::from_str(data).ok()?;
    if parsed.event_type == "content_block_delta" {
        if let Some(delta) = parsed.delta {
            if delta.delta_type.as_deref() == Some("text_delta") {
                return delta.text.filter(|s| !s.is_empty());
            }
        }
    }
    None
}

/// Feed raw SSE bytes / lines into a buffer and invoke `on_data` for each `data:` payload.
pub struct SseLineParser {
    buf: String,
}

impl SseLineParser {
    pub fn new() -> Self {
        Self { buf: String::new() }
    }

    pub fn push(&mut self, chunk: &str, mut on_data: impl FnMut(&str)) {
        self.buf.push_str(chunk);
        while let Some(idx) = self.buf.find('\n') {
            let mut line = self.buf[..idx].to_string();
            self.buf.drain(..=idx);
            if line.ends_with('\r') {
                line.pop();
            }
            if let Some(data) = line.strip_prefix("data:") {
                on_data(data.trim_start());
            }
            // ignore event:/id:/comment lines
        }
    }
}

impl Default for SseLineParser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct OpenAiStreamChunk {
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Deserialize)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: OpenAiDelta,
}

#[derive(Deserialize, Default)]
struct OpenAiDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicDelta>,
}

#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_content_delta() {
        let d = r#"{"id":"x","choices":[{"delta":{"content":"Bon"},"index":0}]}"#;
        assert_eq!(openai_sse_delta(d).unwrap(), Some("Bon".into()));
        assert!(openai_sse_delta("[DONE]").is_err());
        assert_eq!(
            openai_sse_delta(r#"{"choices":[{"delta":{"role":"assistant"}}]}"#).unwrap(),
            None
        );
    }

    #[test]
    fn anthropic_text_delta() {
        let d = r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"jour"}}"#;
        assert_eq!(anthropic_sse_delta(d).as_deref(), Some("jour"));
        assert!(anthropic_sse_delta(r#"{"type":"message_stop"}"#).is_none());
    }

    #[test]
    fn line_parser_splits() {
        let mut p = SseLineParser::new();
        let mut out = Vec::new();
        p.push("data: one\ndata: two\nda", |d| out.push(d.to_string()));
        p.push("ta: three\n", |d| out.push(d.to_string()));
        assert_eq!(out, ["one", "two", "three"]);
    }
}
