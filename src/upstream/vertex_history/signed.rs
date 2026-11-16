//! Restore only explicitly indexed, matching assistant turns; IDs are correlation, never vendor state.
use super::Contract;
use crate::{
    error::GatewayError,
    protocol::{
        canonical::CanonicalRelayRequest,
        vertex_turn::{self, VertexAssistantTurn},
    },
};
use serde_json::{json, Value};

fn invalid(contract: Contract) -> GatewayError {
    GatewayError::bad_request(format!(
        "Invalid or mismatched {} signed history.",
        contract.label
    ))
    .with_code(format!("{}_invalid_signed_history", contract.code_prefix))
}

pub(super) fn reject_untyped(contract: Contract, parts: &[Value]) -> Result<(), GatewayError> {
    if parts.iter().any(|p| {
        ["thought", "thoughtSignature", "thought_signature"]
            .iter()
            .any(|k| p.get(k).is_some())
    }) {
        return Err(invalid(contract));
    }
    Ok(())
}

pub(super) fn reject_override(
    contract: Contract,
    req: &CanonicalRelayRequest,
) -> Result<(), GatewayError> {
    if req.raw_body.get(vertex_turn::HISTORY_FIELD).is_some() && req.extra.contains_key("contents")
    {
        return Err(invalid(contract));
    }
    Ok(())
}

pub(super) fn apply(
    contract: Contract,
    req: &CanonicalRelayRequest,
    contents: &mut [Value],
    model: &str,
) -> Result<(), GatewayError> {
    let invalid = || invalid(contract);
    let Some(raw) = req.raw_body.get(vertex_turn::HISTORY_FIELD) else {
        return Ok(());
    };
    vertex_turn::wire::within_budget(raw, 8 * 1024 * 1024).map_err(|_| invalid())?;
    let entries = raw
        .as_array()
        .filter(|v| v.len() <= 64)
        .ok_or_else(invalid)?;
    let model_indices: Vec<_> = contents
        .iter()
        .enumerate()
        .filter(|(_, v)| v["role"] == "model")
        .map(|(i, _)| i)
        .collect();
    let mut previous = None;
    for raw in entries {
        // Validate the closed entry schema before cloning only its typed turn.
        if raw.as_object().is_none_or(|o| {
            o.len() != 2 || !o.contains_key("assistant_turn") || !o.contains_key("turn")
        }) {
            return Err(invalid());
        }
        let ordinal = raw["assistant_turn"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(invalid)?;
        let turn = vertex_turn::wire::read_turn(&raw["turn"]).map_err(|_| invalid())?;
        if previous.is_some_and(|i| ordinal <= i)
            || turn.model != model
            || turn.source != contract.source
        {
            return Err(invalid());
        }
        previous = Some(ordinal);
        let index = *model_indices.get(ordinal).ok_or_else(invalid)?;
        matches_visible(contract, &turn, &contents[index])?;
        contents[index]["parts"] = turn.google_parts();
    }
    Ok(())
}

fn matches_visible(
    contract: Contract,
    turn: &VertexAssistantTurn,
    content: &Value,
) -> Result<(), GatewayError> {
    let invalid = || invalid(contract);
    let expected = content["parts"].as_array().ok_or_else(invalid)?;
    if expected.iter().any(|p| {
        p.as_object().is_none_or(|o| {
            o.keys()
                .any(|k| !matches!(k.as_str(), "text" | "functionCall"))
        }) || p.get("text").is_some_and(|v| !v.is_string())
    }) {
        return Err(invalid());
    }
    let text: String = expected.iter().filter_map(|p| p["text"].as_str()).collect();
    if turn.visible_text() != text {
        return Err(invalid());
    }
    let expected: Vec<_> = expected
        .iter()
        .filter_map(|p| p.get("functionCall"))
        .collect();
    let calls: Vec<_> = turn
        .parts
        .iter()
        .filter(|p| p.function_call.is_some())
        .collect();
    if calls.len() != expected.len() {
        return Err(invalid());
    }
    for (part, call) in calls.into_iter().zip(expected) {
        let signed = part.function_call.as_ref().expect("filtered call");
        if call["id"].as_str() != part.public_id.as_deref()
            || call["name"].as_str() != Some(signed.name.as_str())
            || call["args"]
                != signed
                    .args
                    .as_ref()
                    .map(|v| json!(v))
                    .unwrap_or_else(|| json!({}))
        {
            return Err(invalid());
        }
    }
    Ok(())
}
