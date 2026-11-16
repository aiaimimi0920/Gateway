//! Accio-only response admission. Shared Anthropic compatibility keeps its own semantics.
use super::{event_parse::parse_raw_event, parse_provider_error, ParsedEvent};
use crate::{
    error::GatewayError,
    protocol::{
        sse_parse::{parse_sse_line, SseParseState},
        upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES,
    },
};
use serde_json::Value;
use std::collections::HashSet;

pub(super) const FRAME_LIMIT: usize = 256 * 1024;
pub(super) struct Decoder {
    buffer: Vec<u8>,
    frame: SseParseState,
    frame_bytes: usize,
    total: usize,
    binary: Option<bool>,
    reason: Option<String>,
    done: bool,
    output_seen: bool,
    tool_count: usize,
    argument_bytes: usize,
    active_tools: HashSet<i64>,
}
impl Decoder {
    pub(super) fn new() -> Self {
        Self {
            buffer: Vec::new(),
            frame: SseParseState::new(),
            frame_bytes: 0,
            total: 0,
            binary: None,
            reason: None,
            done: false,
            output_seen: false,
            tool_count: 0,
            argument_bytes: 0,
            active_tools: HashSet::new(),
        }
    }
    pub(super) fn feed(&mut self, bytes: &[u8]) -> Result<Vec<ParsedEvent>, GatewayError> {
        self.total = self.total.saturating_add(bytes.len());
        if self.total > MAX_ACCUMULATED_UPSTREAM_BODY_BYTES {
            return Err(invalid());
        }
        let mut events = Vec::new();
        for byte in bytes {
            let binary = *self.binary.get_or_insert(*byte == 0);
            self.buffer.push(*byte);
            self.frame_bytes += 1;
            if self.frame_bytes > FRAME_LIMIT {
                return Err(invalid());
            }
            if binary {
                // Preserve the old binary-envelope compatibility, not an assertion of AWS CRC support.
                if self.buffer.len() < 12 {
                    continue;
                }
                let total = u32::from_be_bytes(self.buffer[..4].try_into().unwrap()) as usize;
                let headers = u32::from_be_bytes(self.buffer[4..8].try_into().unwrap()) as usize;
                if !(16..=FRAME_LIMIT).contains(&total) || headers > total - 16 {
                    return Err(invalid());
                }
                if self.buffer.len() == total {
                    let value: Value =
                        serde_json::from_slice(&self.buffer[12 + headers..total - 4])
                            .map_err(|_| invalid())?;
                    self.buffer.clear();
                    self.frame_bytes = 0;
                    events.extend(self.value(value)?);
                }
            } else if *byte == b'\n' {
                events.extend(self.line()?);
            }
        }
        Ok(events)
    }
    fn line(&mut self) -> Result<Vec<ParsedEvent>, GatewayError> {
        let line = std::str::from_utf8(&self.buffer)
            .map_err(|_| invalid())?
            .trim_end_matches('\n');
        let frame = parse_sse_line(line, &mut self.frame);
        if line.is_empty() || line == "\r" {
            self.frame_bytes = 0;
        }
        self.buffer.clear();
        match frame {
            Some(frame) if frame.data.is_empty() => Ok(Vec::new()),
            Some(frame) if frame.data == "[DONE]" => self.accept(vec![ParsedEvent::Done]),
            Some(frame) => self.value(serde_json::from_str(&frame.data).map_err(|_| invalid())?),
            None => Ok(Vec::new()),
        }
    }
    fn value(&mut self, outer: Value) -> Result<Vec<ParsedEvent>, GatewayError> {
        // Check both envelopes: a raw_response_json field must not hide an outer error.
        if let Some(error) = parse_provider_error(&outer) {
            return self.accept(vec![error]);
        }
        let raw = match outer.get("raw_response_json") {
            Some(Value::String(raw)) => serde_json::from_str(raw).map_err(|_| invalid())?,
            Some(_) => return Err(invalid()),
            None => outer,
        };
        self.accept(parse_raw_event(&raw))
    }
    fn accept(&mut self, events: Vec<ParsedEvent>) -> Result<Vec<ParsedEvent>, GatewayError> {
        let mut output = Vec::new();
        for event in events {
            match event {
                ParsedEvent::ProviderError { code, message } => {
                    return Err(super::classify_accio_provider_error(&code, &message))
                }
                ParsedEvent::Finish { reason, usage } => {
                    if let Some(reason) = reason {
                        if self
                            .reason
                            .as_ref()
                            .is_some_and(|previous| previous != &reason)
                        {
                            return Err(invalid());
                        }
                        self.reason = Some(reason);
                    }
                    // Usage-only metadata is not proof of completion. Finish is held until EOF,
                    // so a later business error or malformed tail cannot become a success receipt.
                    output.push(ParsedEvent::Start { model: None, usage });
                }
                ParsedEvent::Done => {
                    self.done = true;
                }
                other => {
                    if matches!(
                        &other,
                        ParsedEvent::Text(_)
                            | ParsedEvent::ToolStart { .. }
                            | ParsedEvent::ToolDelta { .. }
                            | ParsedEvent::ToolCall { .. }
                    ) {
                        if self.done || self.reason.is_some() {
                            return Err(invalid());
                        }
                        self.output_seen |= match &other {
                            ParsedEvent::Text(text) => !text.is_empty(),
                            _ => true,
                        };
                    }
                    match &other {
                        ParsedEvent::ToolStart { index, .. } => {
                            if !self.active_tools.insert(*index) {
                                return Err(invalid());
                            }
                            self.tool_count += 1;
                        }
                        ParsedEvent::ToolDelta { index, .. }
                            if !self.active_tools.contains(index) =>
                        {
                            return Err(invalid())
                        }
                        ParsedEvent::ToolEnd { index } => {
                            self.active_tools.remove(index);
                        }
                        ParsedEvent::ToolCall { .. } => {
                            self.tool_count += 1;
                        }
                        _ => {}
                    }
                    match &other {
                        ParsedEvent::ToolDelta { partial, .. } => {
                            self.argument_bytes = self.argument_bytes.saturating_add(partial.len())
                        }
                        ParsedEvent::ToolCall { arguments, .. } => {
                            self.argument_bytes =
                                self.argument_bytes.saturating_add(arguments.len())
                        }
                        _ => {}
                    }
                    if self.tool_count > 128 || self.argument_bytes > 1024 * 1024 {
                        return Err(invalid());
                    }
                    output.push(other);
                }
            }
        }
        Ok(output)
    }
    pub(super) fn complete(&mut self) -> Result<Vec<ParsedEvent>, GatewayError> {
        let mut events = Vec::new();
        if self.binary == Some(true) {
            if !self.buffer.is_empty() {
                return Err(invalid());
            }
        } else {
            // Retain legacy EOF-delimited complete JSON, but never parse half JSON mid-chunk.
            if !self.buffer.is_empty() {
                events.extend(self.line()?);
            }
            self.buffer.push(b'\n');
            events.extend(self.line()?);
        }
        if (!self.done && self.reason.is_none()) || !self.output_seen {
            return Err(invalid());
        }
        events.push(ParsedEvent::Finish {
            reason: Some(self.reason.clone().unwrap_or_else(|| "stop".into())),
            usage: None,
        });
        events.push(ParsedEvent::Done);
        Ok(events)
    }
}
pub(super) fn invalid() -> GatewayError {
    let mut error = GatewayError::server_error("Accio returned an invalid or incomplete response.")
        .with_code("accio_invalid_stream")
        .with_provider("accio_compatible");
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "Invalid Accio response; generation was not replayed.".into(),
    };
    error
}
