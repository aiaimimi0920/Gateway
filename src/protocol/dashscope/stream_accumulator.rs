//! Accumulate OpenAI deltas for DashScope's default cumulative-output mode.
use crate::protocol::stream_error::ProtocolStreamError;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Accumulator {
    choices: BTreeMap<u64, Value>,
    bytes: usize,
}
impl Accumulator {
    pub(super) fn apply(&mut self, body: &mut Value) -> Result<(), ProtocolStreamError> {
        if let Some(choices) = body.get("choices").and_then(Value::as_array) {
            for choice in choices {
                let index = choice["index"].as_u64().unwrap_or(0);
                let state = self.choices.entry(index).or_insert_with(
                    || json!({"index":index,"delta":{"role":"assistant"},"finish_reason":null}),
                );
                if let Some(delta) = choice.get("delta").and_then(Value::as_object) {
                    for (key, value) in delta {
                        if matches!(key.as_str(), "content" | "reasoning_content") {
                            if let Some(text) = value.as_str() {
                                self.bytes = self.bytes.saturating_add(text.len());
                                let old = state["delta"][key].as_str().unwrap_or("");
                                state["delta"][key] = json!(format!("{old}{text}"));
                            }
                        } else if key == "tool_calls" {
                            if let Some(tools) = value.as_array() {
                                for tool in tools {
                                    let i = tool["index"].as_u64().unwrap_or(0) as usize;
                                    if i >= 128 {
                                        return Err(ProtocolStreamError::invalid_data(
                                            "Too many DashScope tool calls",
                                        ));
                                    }
                                    if !state["delta"]["tool_calls"].is_array() {
                                        state["delta"]["tool_calls"] = json!([]);
                                    }
                                    let list = state["delta"]["tool_calls"].as_array_mut().unwrap();
                                    while list.len() <= i {
                                        list.push(json!({"type":"function","function":{}}));
                                    }
                                    if let Some(id) = tool.get("id") {
                                        list[i]["id"] = id.clone();
                                    }
                                    for field in ["name", "arguments"] {
                                        if let Some(text) = tool["function"][field].as_str() {
                                            self.bytes = self.bytes.saturating_add(text.len());
                                            let old =
                                                list[i]["function"][field].as_str().unwrap_or("");
                                            list[i]["function"][field] =
                                                json!(format!("{old}{text}"));
                                        }
                                    }
                                }
                            }
                        } else {
                            state["delta"][key] = value.clone();
                        }
                    }
                }
                if choice.get("finish_reason").is_some_and(|v| !v.is_null()) {
                    state["finish_reason"] = choice["finish_reason"].clone();
                }
            }
        }
        if self.bytes > 16 * 1024 * 1024 || self.choices.len() > 128 {
            return Err(ProtocolStreamError::invalid_data(
                "DashScope cumulative response exceeded limit",
            ));
        }
        body["choices"] = Value::Array(self.choices.values().cloned().collect());
        Ok(())
    }
}
