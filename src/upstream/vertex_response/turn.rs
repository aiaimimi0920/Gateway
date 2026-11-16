//! Preserve original complete text/function Parts; signed Preview fragments have no proven association.
use super::{Contract, GatewayError, Value};
use crate::protocol::{
    canonical::CanonicalToolCall,
    vertex_turn::{self, VertexAssistantTurn},
};
use serde_json::json;

pub(super) fn capture(
    contract: Contract,
    parts: &[Value],
    calls: &[CanonicalToolCall],
    model: &str,
) -> Result<Option<VertexAssistantTurn>, GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    let has_state = parts.iter().any(|p| {
        p.get("thought").is_some()
            || p.get("thoughtSignature").is_some()
            || p.get("thought_signature").is_some()
    });
    let complete = parts.iter().all(|p| {
        p.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                matches!(
                    k.as_str(),
                    "text" | "functionCall" | "thought" | "thoughtSignature"
                )
            })
        }) && match (p.get("text"), p.get("functionCall")) {
            (Some(text), None) => text.is_string(),
            (None, Some(call)) => call
                .as_object()
                .is_some_and(|o| o.keys().all(|k| matches!(k.as_str(), "name" | "args"))),
            _ => false,
        }
    });
    if !complete {
        if has_state {
            return Err(invalid(
                "Unsupported signed or reasoning Part representation.",
            ));
        }
        return Ok(None);
    }
    let mixed = !calls.is_empty() && parts.iter().any(|p| p.get("text").is_some());
    if parts.is_empty() || !has_state && !mixed {
        return Ok(None);
    }
    let mut index = 0;
    let projected = parts
        .iter()
        .map(|p| {
            let mut value = json!({});
            if let Some(text) = p.get("text") {
                value["text"] = text.clone();
            }
            if let Some(call) = p.get("functionCall") {
                let canonical = calls
                    .get(index)
                    .ok_or_else(|| invalid("Incomplete ordered tool projection."))?;
                value["function_call"] = call.clone();
                value["public_id"] = json!(canonical.id);
                index += 1;
            }
            if let Some(thought) = p.get("thought") {
                value["thought"] = thought.clone();
            }
            if let Some(signature) = p.get("thoughtSignature") {
                value["thought_signature"] = signature.clone();
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, GatewayError>>()?;
    let value = json!({"source":contract.source,"model":model,"parts":projected});
    vertex_turn::wire::read_turn(&value)
        .map(Some)
        .map_err(|_| invalid("Invalid ordered assistant state."))
}
