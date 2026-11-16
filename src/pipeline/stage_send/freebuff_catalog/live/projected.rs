//! Protocol projection is fallible before FINISH. No observer receives run or purchase authority.
use super::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
mod frame;
mod messages;
mod responses;
#[cfg(test)]
mod tests;
struct Tool {
    identity: Value,
    pending: String,
    output: Option<usize>,
}
#[derive(Clone, Copy)]
enum Slot {
    Text,
    Tool(u64),
}
pub(super) struct Projection {
    kind: EndpointKind,
    id: String,
    model: String,
    sequence: u64,
    started: bool,
    observed: bool,
    remaining: usize,
    text: Option<usize>,
    tools: BTreeMap<u64, Tool>,
    order: Vec<Slot>,
}
impl Projection {
    pub fn new(kind: EndpointKind, model: String, id: String) -> Self {
        Self {
            kind,
            model,
            id,
            sequence: 0,
            started: false,
            observed: false,
            remaining: crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES,
            text: None,
            tools: BTreeMap::new(),
            order: Vec::new(),
        }
    }
    fn is_messages(&self) -> bool {
        self.kind == EndpointKind::Messages
    }
    fn item_id(&self, index: usize) -> String {
        format!("fb_{}_{}", self.id, index)
    }
    fn emit(
        &mut self,
        event: &str,
        mut value: Value,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        value["type"] = json!(event);
        if !self.is_messages() {
            value["sequence_number"] = json!(self.sequence);
            self.sequence += 1;
        }
        // Event names are constants, never upstream input. Measure escaped JSON before allocation.
        let framing = event.len() + 16;
        if self.remaining < framing || !protocol::within_limit(&value, self.remaining - framing) {
            return Err(BusinessError::ResponseTooLarge);
        }
        let before = out.len();
        out.extend_from_slice(b"event: ");
        out.extend_from_slice(event.as_bytes());
        out.extend_from_slice(b"\ndata: ");
        serde_json::to_writer(&mut *out, &value).map_err(|_| BusinessError::InvalidResponse)?;
        out.extend_from_slice(b"\n\n");
        self.remaining -= out.len() - before;
        Ok(())
    }
    fn start(&mut self, out: &mut Vec<u8>) -> Result<(), BusinessError> {
        if self.started {
            return Ok(());
        }
        self.started = true;
        if self.is_messages() {
            self.start_messages(out)
        } else {
            self.start_responses(out)
        }
    }
    fn text_slot(&mut self, out: &mut Vec<u8>) -> Result<usize, BusinessError> {
        if let Some(index) = self.text {
            return Ok(index);
        }
        let index = self.order.len();
        self.order.push(Slot::Text);
        self.text = Some(index);
        if self.is_messages() {
            self.text_start_messages(index, out)?;
        } else {
            self.text_start_responses(index, out)?;
        }
        Ok(index)
    }
    fn text_delta(&mut self, text: &str, out: &mut Vec<u8>) -> Result<(), BusinessError> {
        let index = self.text_slot(out)?;
        if self.is_messages() {
            self.text_delta_messages(index, text, out)
        } else {
            self.text_delta_responses(index, text, out)
        }
    }
    fn tool_start(
        &mut self,
        index: usize,
        tool: &Value,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        if self.is_messages() {
            self.tool_start_messages(index, tool, out)
        } else {
            self.tool_start_responses(index, tool, out)
        }
    }
    fn tool_delta(
        &mut self,
        index: usize,
        args: &str,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        if self.is_messages() {
            self.tool_delta_messages(index, args, out)
        } else {
            self.tool_delta_responses(index, args, out)
        }
    }
    pub fn finish(&mut self, body: Value) -> Result<Bytes, BusinessError> {
        // The same strict full-envelope converter rejects arrays as Anthropic tool input, duplicate
        // IDs and other unrepresentable native output BEFORE the original paid FINISH.
        let mut out = Vec::new();
        self.start(&mut out)?;
        if !self.observed {
            self.delta(&body["choices"][0]["message"], &mut out)?;
        }
        let mut final_body = protocol::project(body, self.kind, &self.model, &self.id)?;
        let key = if self.is_messages() {
            "content"
        } else {
            "output"
        };
        let Value::Array(extras) =
            std::mem::take(&mut final_body["gateway_freebuff"]["tool_calls"])
        else {
            return Err(BusinessError::InvalidResponse);
        };
        let mut extras = extras.into_iter();
        let parts = final_body[key]
            .as_array_mut()
            .ok_or(BusinessError::InvalidResponse)?;
        let mut text = None;
        let mut tools = BTreeMap::new();
        for part in std::mem::take(parts) {
            if matches!(part["type"].as_str(), Some("text" | "message")) {
                text = Some(part);
            } else {
                let id = part["id"]
                    .as_str()
                    .ok_or(BusinessError::InvalidResponse)?
                    .to_owned();
                tools.insert(id, (part, extras.next().expect("converter tool metadata")));
            }
        }
        if self.order.is_empty() {
            self.text_slot(&mut out)?;
        }
        let mut ordered = Vec::with_capacity(self.order.len());
        let mut ordered_extras = Vec::new();
        for (index, slot) in self.order.clone().into_iter().enumerate() {
            let mut part = match slot {
                Slot::Text => text.take().unwrap_or_else(|| {
                    if self.is_messages() { json!({"type":"text","text":""}) }
                    else { json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":""}]}) }
                }),
                Slot::Tool(native) => {
                    let id = self.tools[&native].identity["id"].as_str().expect("started tool identity");
                    let (part, extra) = tools.remove(id).ok_or(BusinessError::InvalidResponse)?;
                    ordered_extras.push(extra);
                    part
                }
            };
            if !self.is_messages() {
                part["id"] = json!(self.item_id(index));
                part["status"] = json!("completed");
            }
            if self.is_messages() {
                self.part_done_messages(index, &part, &mut out)?;
            } else {
                self.part_done_responses(index, &part, &mut out)?;
            }
            ordered.push(part);
        }
        if text.is_some() || !tools.is_empty() {
            return Err(BusinessError::InvalidResponse);
        }
        final_body["id"] = json!(self.id);
        final_body[key] = json!(ordered);
        final_body["gateway_freebuff"]["tool_calls"] = json!(ordered_extras);
        if self.is_messages() {
            self.finish_messages(final_body, &mut out)?;
        } else {
            self.finish_responses(final_body, &mut out)?;
        }
        Ok(Bytes::from(out))
    }
}
