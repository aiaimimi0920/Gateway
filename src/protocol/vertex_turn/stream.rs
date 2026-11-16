//! A bounded projection witness validates terminal state without retaining ordinary text twice.
use super::*;
use crate::protocol::stream_error::ProtocolStreamError;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Default)]
struct Tool {
    id: String,
    name: String,
    args: String,
}
#[derive(Default)]
struct Witness {
    text: Sha256,
    tools: BTreeMap<usize, Tool>,
    retained: usize,
    unavailable: bool,
}
impl Witness {
    fn observe(&mut self, chunk: &Value) {
        let delta = &chunk["choices"][0]["delta"];
        if let Some(text) = delta["content"].as_str() {
            self.text.update(text.as_bytes());
        }
        if self.unavailable {
            return;
        }
        if let Some(calls) = delta["tool_calls"].as_array() {
            for (fallback, call) in calls.iter().enumerate() {
                let index = call["index"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                    .unwrap_or(fallback);
                let strings = [
                    call["id"].as_str().unwrap_or(""),
                    call["function"]["name"].as_str().unwrap_or(""),
                    call["function"]["arguments"].as_str().unwrap_or(""),
                ];
                let bytes: usize = strings.iter().map(|s| s.len()).sum();
                if self.tools.len() >= 4096 && !self.tools.contains_key(&index)
                    || bytes > MAX_TURN_BYTES.saturating_sub(self.retained)
                    || call
                        .get("type")
                        .and_then(Value::as_str)
                        .is_some_and(|v| v != "function")
                {
                    // Legacy streams promise no extension. Drop only the witness on overflow;
                    // reject a later state declaration, not ordinary content without that contract.
                    self.tools.clear();
                    self.unavailable = true;
                    return;
                }
                self.retained += bytes;
                let tool = self.tools.entry(index).or_default();
                tool.id.push_str(strings[0]);
                tool.name.push_str(strings[1]);
                tool.args.push_str(strings[2]);
            }
        }
    }
    fn matches(&self, turn: &VertexAssistantTurn) -> bool {
        if self.unavailable
            || self.text.clone().finalize() != Sha256::digest(turn.visible_text().as_bytes())
        {
            return false;
        }
        let calls: Vec<_> = turn
            .parts
            .iter()
            .filter(|p| p.function_call.is_some())
            .collect();
        if calls.len() != self.tools.len() {
            return false;
        }
        calls
            .into_iter()
            .zip(self.tools.values())
            .all(|(part, tool)| {
                let call = part.function_call.as_ref().expect("filtered call");
                let args = if tool.args.is_empty() {
                    Ok(json!({}))
                } else {
                    serde_json::from_str::<Value>(&tool.args)
                };
                part.public_id.as_deref() == Some(tool.id.as_str())
                    && call.name == tool.name
                    && args.ok()
                        == Some(
                            call.args
                                .as_ref()
                                .map(|a| json!(a))
                                .unwrap_or_else(|| json!({})),
                        )
            })
    }
}
#[derive(Default)]
pub(crate) struct TerminalTurn(pub Option<VertexAssistantTurn>, Witness);
impl TerminalTurn {
    pub(crate) fn accept(&mut self, chunk: &Value) -> Result<(), ProtocolStreamError> {
        let delta = &chunk["choices"][0]["delta"];
        let mutation = delta["content"].as_str().is_some_and(|s| !s.is_empty())
            || delta["tool_calls"]
                .as_array()
                .is_some_and(|v| !v.is_empty());
        if self.0.is_some() && (mutation || chunk.get(WIRE_FIELD).is_some()) {
            return Err(error());
        }
        self.1.observe(chunk);
        let Some(_) = chunk.get(WIRE_FIELD) else {
            return Ok(());
        };
        if !matches!(
            chunk["choices"][0]["finish_reason"].as_str(),
            Some("stop" | "length" | "tool_calls")
        ) || !delta.as_object().is_some_and(Map::is_empty)
        {
            return Err(error());
        }
        let turn = read(chunk)
            .map_err(|_| error())?
            .expect("declared extension");
        if !self.1.matches(&turn) {
            return Err(error());
        }
        self.0 = Some(turn);
        self.1.tools.clear();
        Ok(())
    }
    /// Called only for our own freshly built Chat frames, never for opaque native passthrough.
    pub(crate) fn accept_output(&mut self, output: &[u8]) -> Result<(), ProtocolStreamError> {
        let text = std::str::from_utf8(output).map_err(|_| error())?;
        let raw = text.strip_prefix("data: ").ok_or_else(error)?.trim_end();
        if raw == "[DONE]" {
            return Ok(());
        }
        let chunk = serde_json::from_str(raw).map_err(|_| error())?;
        self.accept(&chunk)
    }
}
fn error() -> ProtocolStreamError {
    ProtocolStreamError::invalid_data(INVALID)
}
