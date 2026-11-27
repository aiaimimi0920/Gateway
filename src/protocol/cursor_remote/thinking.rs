//! Only the first-party public thinking text, never a summary, signature or ciphertext.
//! History stores a digest and (for Responses) item identity, not another text copy.
use super::tool_schema::invalid;
use crate::{error::GatewayError, protocol::canonical::EndpointKind};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub(super) const MAX_PHASE: usize = 16 * 1024;
pub(super) const MAX_RUN: usize = 64 * 1024;

#[derive(Clone)]
pub(crate) struct PublicThinking {
    id: String,
    text: String,
}
impl std::fmt::Debug for PublicThinking {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicThinking")
            .field("bytes", &self.text.len())
            .finish()
    }
}
impl PublicThinking {
    pub(super) fn from_worker(event: &Value) -> Result<Option<Self>, GatewayError> {
        let Some(value) = event.get("thinking") else {
            return Ok(None); // Older private workers omit the optional field.
        };
        let text = value
            .as_str()
            .filter(|text| text.len() <= MAX_PHASE)
            .ok_or_else(|| super::error("worker_protocol"))?;
        Ok((!text.is_empty()).then(|| Self {
            id: format!("rs_cursor_{}", uuid::Uuid::new_v4().simple()),
            text: text.into(),
        }))
    }
    pub(crate) fn text(&self) -> &str {
        &self.text
    }
    pub(crate) fn item(&self, completed: bool) -> Value {
        let content = if completed {
            vec![json!({"type":"reasoning_text","text":self.text})]
        } else {
            vec![]
        };
        json!({"id":self.id,"type":"reasoning","summary":[],"content":content,
            "status":if completed {"completed"} else {"in_progress"}})
    }
    pub(super) fn history_item(&self, chat: bool) -> Value {
        marker(&self.text, (!chat).then_some(self.id.as_str()))
    }
    pub(crate) fn project(&self, body: &mut Value, endpoint: EndpointKind) {
        if endpoint == EndpointKind::ChatCompletions {
            body["choices"][0]["message"]["reasoning_content"] = json!(self.text);
        } else {
            body["output"]
                .as_array_mut()
                .expect("Responses pack produces output")
                .insert(0, self.item(true));
        }
    }
    /// Buffered, verified phase only. The Responses owner assigns sequence numbers.
    pub(crate) fn events(&self, index: usize) -> Vec<Value> {
        vec![
            json!({"type":"response.output_item.added","output_index":index,"item":self.item(false)}),
            json!({"type":"response.content_part.added","output_index":index,"item_id":self.id,
                "content_index":0,"part":{"type":"reasoning_text","text":""}}),
            json!({"type":"response.reasoning_text.delta","output_index":index,"item_id":self.id,
                "content_index":0,"delta":self.text}),
            json!({"type":"response.reasoning_text.done","output_index":index,"item_id":self.id,
                "content_index":0,"text":self.text}),
            json!({"type":"response.content_part.done","output_index":index,"item_id":self.id,
                "content_index":0,"part":{"type":"reasoning_text","text":self.text}}),
            json!({"type":"response.output_item.done","output_index":index,"item":self.item(true)}),
        ]
    }
}

fn marker(text: &str, id: Option<&str>) -> Value {
    let mut item = json!({"type":"cursor_public_thinking","sha256":format!("{:x}", Sha256::digest(text.as_bytes()))});
    if let Some(id) = id {
        item["id"] = json!(id);
    }
    item
}

/// Validate public echo before hashing. Never accept a caller-supplied digest.
pub(super) fn history(item: &Value, chat: bool) -> Result<Value, GatewayError> {
    let fields = item.as_object().ok_or_else(invalid)?;
    // Codex retains public reasoning content but omits output-only status and
    // serializes absent ciphertext as null. Neither changes the bound marker.
    if fields.iter().any(|(key, value)| match key.as_str() {
        "type" | "summary" | "content" | "status" => false,
        "id" if !chat => false,
        "encrypted_content" if !chat => !value.is_null(),
        _ => true,
    }) || item["type"] != "reasoning"
        || item["summary"] != json!([])
        || fields
            .get("status")
            .map_or(chat, |value| value != "completed")
    {
        return Err(invalid());
    }
    let parts = item["content"]
        .as_array()
        .filter(|parts| parts.len() == 1)
        .ok_or_else(invalid)?;
    let part = parts[0].as_object().ok_or_else(invalid)?;
    let text = part
        .get("text")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty() && text.len() <= MAX_PHASE)
        .ok_or_else(invalid)?;
    if part.len() != 2 || parts[0]["type"] != "reasoning_text" {
        return Err(invalid());
    }
    let id = if chat {
        None
    } else {
        Some(
            item["id"]
                .as_str()
                .filter(|id| id.starts_with("rs_cursor_") && id.len() <= 64)
                .ok_or_else(invalid)?,
        )
    };
    Ok(marker(text, id))
}

pub(super) fn chat_item(value: &Value) -> Result<Value, GatewayError> {
    let text = value
        .as_str()
        .filter(|text| !text.is_empty() && text.len() <= MAX_PHASE)
        .ok_or_else(invalid)?;
    Ok(json!({"type":"reasoning","summary":[],"status":"completed",
        "content":[{"type":"reasoning_text","text":text}]}))
}

#[cfg(test)]
mod tests;
