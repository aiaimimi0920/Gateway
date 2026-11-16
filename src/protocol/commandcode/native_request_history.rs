//! 保留原生历史；不可逆的 rich history 修改不能靠有损 pack 猜测重建。
use super::{
    invalid,
    native_request::{equal, normalize, rebuilt},
    GatewayError,
};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, MessageRole};
use serde_json::{json, Value};

pub(super) fn sync(
    req: &CanonicalRelayRequest,
    original: &CanonicalRelayRequest,
    body: &mut Value,
    packed: &Value,
) -> Result<(), GatewayError> {
    if equal(&req.messages, &original.messages) {
        return Ok(());
    }
    let responses = req.endpoint_kind == EndpointKind::Responses;
    let (history, top) = if responses {
        ("input", "instructions")
    } else {
        ("messages", "system")
    };
    let mut history_body = req.raw_body.clone();
    history_body.as_object_mut().unwrap().remove(top);
    let old_history = normalize(req, history_body)?;
    // A top-level policy edit must not rebuild signatures, opaque items or tool-result flags.
    let count = old_history.messages.len();
    if req.messages.len() >= count {
        let prefix = req.messages.len() - count;
        if equal(&req.messages[prefix..], &old_history.messages[..])
            && req.messages[..prefix]
                .iter()
                .all(|m| m.role == MessageRole::System)
        {
            body.as_object_mut().unwrap().remove(top);
            if prefix == 0 {
                return Ok(());
            }
            let mut policy = req.clone();
            policy.messages = req.messages[..prefix].to_vec();
            policy.extra.clear();
            let policy_body = rebuilt(&policy, "", false);
            if responses {
                let mut items = policy_body["input"].as_array().cloned().unwrap_or_default();
                match &body[history] {
                    Value::Array(values) => items.extend(values.clone()),
                    Value::String(text) => items
                        .push(json!({"role":"user","content":[{"type":"input_text","text":text}]})),
                    _ => return Err(invalid()),
                }
                body[history] = json!(items);
            } else {
                // Pack each policy message to retain all system messages, not just the first.
                let mut blocks = Vec::new();
                for message in &policy.messages {
                    let mut one = policy.clone();
                    one.messages = vec![message.clone()];
                    let value = rebuilt(&one, "", false);
                    match &value["system"] {
                        Value::Array(parts) => blocks.extend(parts.clone()),
                        Value::String(text) => blocks.push(json!({"type":"text","text":text})),
                        _ => return Err(invalid()),
                    }
                }
                body[top] = json!(blocks);
            }
            return Ok(());
        }
    }
    // Generic canonical representations are not injective for rich native items.
    // Until an edit has an unambiguous wire identity, reject rather than revive/drop content.
    for source in [original, req] {
        let mut history_only = source.clone();
        history_only.tools.clear();
        history_only.tool_choice = None;
        history_only.reasoning = None;
        history_only.previous_response_id = None;
        history_only.extra.clear();
        history_only.raw_body =
            json!({history: source.raw_body[history], top: source.raw_body[top]});
        super::request::validate(&history_only)?;
        if !responses
            && source
                .messages
                .iter()
                .skip(1)
                .any(|m| m.role == MessageRole::System)
        {
            return Err(invalid());
        }
    }
    body.as_object_mut().unwrap().remove(top);
    body[history] = packed[history].clone();
    if let Some(value) = packed.get(top) {
        body[top] = value.clone();
    }
    Ok(())
}
