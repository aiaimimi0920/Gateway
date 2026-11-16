//! Validate the explicit Bedrock stream carrier, not a credential or signed-history authority.
use super::{
    capture::{Capture, Completion},
    headers::Event,
    invalid,
};
use crate::protocol::{bedrock_turn, canonical::TokenUsage, stream_error::ProtocolStreamError};
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) const EVENT_FIELD: &str = "gateway_bedrock_event";
pub(crate) const TERMINAL_FIELD: &str = "gateway_bedrock_terminal";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    sequence: u64,
    event: String,
    body: Value,
    #[serde(default)]
    projection: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Terminal {
    events: u64,
}

#[derive(Default)]
pub(crate) struct Relay {
    capture: Capture,
    completion: Option<Completion>,
    active: bool,
    plain: bool,
    done: bool,
    eof: bool,
}
impl Relay {
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn hold_terminal(&self) -> bool {
        self.active && !self.eof
    }
    pub fn observe(&mut self, raw: &Value) -> Result<Option<(String, Value)>, ProtocolStreamError> {
        let event = raw.get(EVENT_FIELD);
        let terminal = raw.get(TERMINAL_FIELD);
        if event.is_none() && terminal.is_none() {
            if self.active || raw.get(bedrock_turn::WIRE_FIELD).is_some() {
                return Err(invalid("missing native stream carrier"));
            }
            self.plain = true;
            return Ok(None);
        }
        if self.plain
            || self.done
            || self.eof
            || self.completion.is_some()
            || raw.get(crate::protocol::vertex_turn::WIRE_FIELD).is_some()
        {
            return Err(invalid("conflicting native stream carrier"));
        }
        if raw.get("object").and_then(Value::as_str) != Some("chat.completion.chunk")
            || raw.get("type").is_some()
        {
            return Err(invalid("conflicting native stream format"));
        }
        let choices = raw
            .get("choices")
            .and_then(Value::as_array)
            .filter(|v| v.len() == 1)
            .ok_or_else(|| invalid("invalid native stream choice"))?;
        let choice = &choices[0];
        if choice.get("index").and_then(Value::as_u64) != Some(0) {
            return Err(invalid("invalid native choice index"));
        }
        let delta = choice
            .get("delta")
            .filter(|v| v.is_object())
            .ok_or_else(|| invalid("invalid native stream delta"))?;
        if let Some(value) = event {
            if terminal.is_some()
                || raw.get(bedrock_turn::WIRE_FIELD).is_some()
                || raw.get("usage").is_some()
                || choice.get("finish_reason").is_some_and(|v| !v.is_null())
            {
                return Err(invalid("premature native stream terminal"));
            }
            let mut envelope: Envelope = serde_json::from_value(value.clone())
                .map_err(|_| invalid("invalid native event envelope"))?;
            if envelope.sequence != self.capture.count || !envelope.body.is_object() {
                return Err(invalid("native event sequence mismatch"));
            }
            restore_projection(&mut envelope, delta)?;
            let event = Event {
                kind: envelope.event,
                body: envelope.body,
            };
            let expected = self.capture.observe(&event)?;
            if *delta != expected {
                return Err(invalid("native event projection mismatch"));
            }
            self.active = true;
            return Ok(Some((event.kind, event.body)));
        }
        if !self.active {
            return Err(invalid("native terminal without events"));
        }
        let terminal: Terminal = serde_json::from_value(terminal.unwrap().clone())
            .map_err(|_| invalid("invalid native terminal envelope"))?;
        let mut completion = std::mem::take(&mut self.capture).finish()?;
        if terminal.events != completion.count
            || *delta != json!({})
            || choice.get("finish_reason").and_then(Value::as_str) != Some(completion.reason)
            || raw.get("usage") != Some(&completion.usage_json())
        {
            return Err(invalid("native terminal witness mismatch"));
        }
        match (raw.get(bedrock_turn::WIRE_FIELD), completion.turn.as_mut()) {
            (Some(raw), Some(observed)) => {
                let supplied = bedrock_turn::wire::read_turn(raw)
                    .map_err(|_| invalid("invalid terminal turn"))?;
                if supplied.content != observed.content {
                    return Err(invalid("native terminal content mismatch"));
                }
                // A local consistency witness may accompany the exact observed content; it is not vendor authentication.
                observed.continuation = supplied.continuation;
            }
            (None, None) => {}
            _ => return Err(invalid("native terminal turn mismatch")),
        }
        self.completion = Some(completion);
        Ok(None)
    }
    pub fn done(&mut self) -> Result<(), ProtocolStreamError> {
        if self.active {
            if self.done || self.completion.is_none() {
                return Err(invalid("native DONE without unique terminal"));
            }
            self.done = true;
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Result<(), ProtocolStreamError> {
        if self.active && (!self.done || self.completion.is_none()) {
            return Err(invalid("native stream EOF before terminal"));
        }
        self.eof = true;
        Ok(())
    }
    pub fn project(&self, body: &mut Value) {
        if self.eof {
            if let Some(completion) = &self.completion {
                completion.project(body);
            }
        }
    }
    pub fn usage(&self) -> Option<TokenUsage> {
        self.completion.as_ref().map(|v| v.usage.clone())
    }
    pub fn public_event(raw: &Value, native_body: &Value) -> Vec<u8> {
        let mut event = raw[EVENT_FIELD].clone();
        event["body"] = native_body.clone();
        event
            .as_object_mut()
            .expect("validated native event envelope")
            .remove("projection");
        let mut body = json!({"type":"gateway.bedrock.event"});
        body[EVENT_FIELD] = event;
        format!("event: gateway.bedrock.event\ndata: {body}\n\n").into_bytes()
    }
}

// Visible text/arguments already exist in the standard delta. Retain them once, not twice.
pub(super) fn envelope(sequence: u64, mut event: Event) -> Value {
    let projection =
        if event.kind == "contentBlockDelta" && event.body.pointer("/delta/text").is_some() {
            event.body["delta"]["text"] = Value::Null;
            Some("content")
        } else if event.kind == "contentBlockDelta"
            && event.body.pointer("/delta/toolUse/input").is_some()
        {
            event.body["delta"]["toolUse"]["input"] = Value::Null;
            Some("tool_input")
        } else {
            None
        };
    let mut value = json!({"sequence":sequence,"event":event.kind,"body":event.body});
    if let Some(projection) = projection {
        value["projection"] = json!(projection);
    }
    value
}
fn restore_projection(envelope: &mut Envelope, delta: &Value) -> Result<(), ProtocolStreamError> {
    let Some(projection) = envelope.projection.as_deref() else {
        return Ok(());
    };
    if envelope.event != "contentBlockDelta" {
        return Err(invalid("native projection outside delta"));
    }
    let (destination, source) = match projection {
        "content" => ("/delta/text", "/content"),
        "tool_input" => ("/delta/toolUse/input", "/tool_calls/0/function/arguments"),
        _ => return Err(invalid("unknown native projection reference")),
    };
    let source = delta
        .pointer(source)
        .filter(|v| v.is_string())
        .ok_or_else(|| invalid("missing native projection source"))?;
    let target = envelope
        .body
        .pointer_mut(destination)
        .filter(|v| v.is_null())
        .ok_or_else(|| invalid("invalid native projection placeholder"))?;
    *target = source.clone();
    Ok(())
}
