//! Per-response event state; streaming does not retain generated text or arguments.
use super::response::{finish, invalid, usage};
use crate::{error::GatewayError, protocol::sse_parse::SseFrame};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) struct Events {
    pub terminal: bool,
    pub collected: Option<Value>,
    id: String,
    model: String,
    created: i64,
    content_seen: bool,
    tools: BTreeMap<u64, usize>,
}

impl Events {
    pub fn new(model: &str, collect: bool) -> Self {
        Self {
            terminal: false,
            collected: collect
                .then(|| json!({"model":model,"message":{"content":"","tool_calls":[]}})),
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            model: model.into(),
            created: time::OffsetDateTime::now_utc().unix_timestamp(),
            content_seen: false,
            tools: BTreeMap::new(),
        }
    }

    fn chunk(&self, delta: Value, reason: Option<&str>) -> Value {
        json!({"id":self.id,"object":"chat.completion.chunk","created":self.created,"model":self.model,
            "choices":[{"index":0,"delta":delta,"finish_reason":reason}]})
    }

    pub fn accept(&mut self, frame: SseFrame) -> Result<Option<Value>, GatewayError> {
        if frame.data.is_empty() {
            return Ok(None);
        }
        if self.terminal {
            return Err(invalid());
        }
        let raw: Value = serde_json::from_str(&frame.data).map_err(|_| invalid())?;
        let kind = raw["type"]
            .as_str()
            .or(frame.event_name.as_deref())
            .ok_or_else(invalid)?;
        if frame
            .event_name
            .as_deref()
            .is_some_and(|event| event != kind)
        {
            return Err(invalid());
        }
        let message = &raw["delta"]["message"];
        match kind {
            "message-start" => {
                if let Some(id) = raw["id"]
                    .as_str()
                    .filter(|v| !v.is_empty() && v.len() <= 256)
                {
                    self.id = id.into();
                }
                Ok(Some(self.chunk(json!({"role":"assistant"}), None)))
            }
            "content-start" | "content-delta" => {
                let Some(text) = message["content"]["text"].as_str() else {
                    return Ok(None);
                };
                if !text.trim().is_empty() {
                    self.content_seen = true;
                }
                if let Some(body) = &mut self.collected {
                    if let Value::String(current) = &mut body["message"]["content"] {
                        current.push_str(text);
                    }
                }
                Ok(Some(self.chunk(json!({"content":text}), None)))
            }
            "tool-call-start" => {
                let index = raw["index"]
                    .as_u64()
                    .filter(|v| *v < 128)
                    .ok_or_else(invalid)?;
                if self.tools.contains_key(&index) {
                    return Err(invalid());
                }
                let call = &message["tool_calls"];
                let id = call["id"]
                    .as_str()
                    .filter(|v| !v.is_empty() && v.len() <= 256)
                    .ok_or_else(invalid)?;
                let name = call["function"]["name"]
                    .as_str()
                    .filter(|v| !v.is_empty() && v.len() <= 256)
                    .ok_or_else(invalid)?;
                let args = call["function"]["arguments"].as_str().unwrap_or_default();
                let position = self.tools.len();
                self.tools.insert(index, position);
                if let Some(body) = &mut self.collected {
                    body["message"]["tool_calls"].as_array_mut().unwrap().push(json!({"id":id,"type":"function","function":{"name":name,"arguments":args}}));
                }
                Ok(Some(self.chunk(json!({"tool_calls":[{"index":index,"id":id,"type":"function","function":{"name":name,"arguments":args}}]}), None)))
            }
            "tool-call-delta" => {
                let index = raw["index"].as_u64().ok_or_else(invalid)?;
                let position = *self.tools.get(&index).ok_or_else(invalid)?;
                let args = message["tool_calls"]["function"]["arguments"]
                    .as_str()
                    .ok_or_else(invalid)?;
                if let Some(body) = &mut self.collected {
                    let field =
                        &mut body["message"]["tool_calls"][position]["function"]["arguments"];
                    if let Value::String(current) = field {
                        current.push_str(args);
                    }
                }
                Ok(Some(self.chunk(
                    json!({"tool_calls":[{"index":index,"function":{"arguments":args}}]}),
                    None,
                )))
            }
            "message-end" => {
                let delta = &raw["delta"];
                let reason = finish(&delta["finish_reason"])?;
                if !self.content_seen && self.tools.is_empty() {
                    return Err(invalid());
                }
                if delta
                    .get("error")
                    .is_some_and(|error| !error.is_null() && error != "")
                {
                    return Err(invalid());
                }
                if let Some(body) = &mut self.collected {
                    body["finish_reason"] = delta["finish_reason"].clone();
                    body["usage"] = delta["usage"].clone();
                }
                self.terminal = true;
                let mut chunk = self.chunk(json!({}), Some(reason));
                if let Some(tokens) = usage(&delta["usage"]) {
                    chunk["usage"] = json!({"prompt_tokens":tokens.prompt_tokens,
                    "completion_tokens":tokens.completion_tokens,"total_tokens":tokens.total_tokens});
                }
                Ok(Some(chunk))
            }
            "content-end" | "tool-call-end" | "tool-plan-delta" | "citation-start"
            | "citation-end" | "debug" => Ok(None),
            // Unknown extensions are not assistant text and cannot synthesize a terminal success.
            "error" => Err(invalid()),
            _ => Ok(None),
        }
    }
}
