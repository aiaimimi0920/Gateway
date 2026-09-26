use serde_json::Value;

use crate::protocol::canonical::TokenUsage;

use super::event_parse::parse_raw_event;

#[derive(Debug, Clone)]
pub(crate) enum ParsedEvent {
    Start {
        model: Option<String>,
        usage: Option<TokenUsage>,
    },
    ProviderError {
        code: String,
        message: String,
    },
    Text(String),
    ToolStart {
        index: i64,
        id: String,
        name: String,
    },
    ToolDelta {
        index: i64,
        partial: String,
    },
    ToolEnd {
        index: i64,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    Finish {
        reason: Option<String>,
        usage: Option<TokenUsage>,
    },
    Done,
}

pub(crate) fn parse_sse_line(line: &[u8]) -> Option<Vec<ParsedEvent>> {
    let line = std::str::from_utf8(line).ok()?.trim();
    let payload = line.strip_prefix("data:")?.trim();
    if payload.is_empty() {
        return None;
    }
    if payload == "[DONE]" {
        return Some(vec![ParsedEvent::Done]);
    }
    let outer: Value = serde_json::from_str(payload).ok()?;
    let raw = if let Some(raw_json) = outer.get("raw_response_json").and_then(|v| v.as_str()) {
        serde_json::from_str(raw_json).ok()?
    } else {
        outer
    };
    Some(parse_raw_event(&raw))
}

pub(super) fn drain_parsed_events(buffer: &mut Vec<u8>) -> Vec<ParsedEvent> {
    let mut events = Vec::new();
    loop {
        if let Some((payload, consumed)) = try_parse_aws_eventstream_payload(buffer) {
            buffer.drain(..consumed);
            events.extend(parse_eventstream_payload(&payload));
            continue;
        }

        let Some(pos) = buffer.iter().position(|&b| b == b'\n') else {
            if let Some(parsed) = parse_sse_line(buffer) {
                events.extend(parsed);
                buffer.clear();
            }
            break;
        };
        let line: Vec<u8> = buffer.drain(..=pos).collect();
        if let Some(parsed) = parse_sse_line(&line) {
            events.extend(parsed);
        }
    }
    events
}

fn try_parse_aws_eventstream_payload(buffer: &[u8]) -> Option<(Vec<u8>, usize)> {
    if buffer.len() < 16 {
        return None;
    }
    let total_len = u32::from_be_bytes(buffer[0..4].try_into().ok()?) as usize;
    let headers_len = u32::from_be_bytes(buffer[4..8].try_into().ok()?) as usize;
    if total_len < 16 || total_len > buffer.len() {
        return None;
    }
    if 12 + headers_len > total_len.saturating_sub(4) {
        return None;
    }
    let payload_start = 12 + headers_len;
    let payload_end = total_len - 4;
    Some((buffer[payload_start..payload_end].to_vec(), total_len))
}

fn parse_eventstream_payload(payload: &[u8]) -> Vec<ParsedEvent> {
    let Ok(text) = std::str::from_utf8(payload) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    parse_raw_event(&value)
}
