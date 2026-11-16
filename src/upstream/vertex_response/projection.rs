//! Project complete single-candidate turns without Gemini's lossy tool deduplication.
use super::{Contract, GatewayError, TokenUsage, Value};
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall};
use std::collections::{HashMap, HashSet};

pub(super) fn project(
    contract: Contract,
    body: &Value,
    fallback_model: &str,
    usage: Option<TokenUsage>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    let candidate = &body["candidates"][0];
    let reason = candidate["finishReason"].as_str().unwrap();
    let filtered = matches!(
        reason,
        "SAFETY"
            | "RECITATION"
            | "BLOCKLIST"
            | "PROHIBITED_CONTENT"
            | "SPII"
            | "MODEL_ARMOR"
            | "IMAGE_SAFETY"
            | "IMAGE_PROHIBITED_CONTENT"
            | "IMAGE_RECITATION"
    );
    if !filtered && !matches!(reason, "STOP" | "MAX_TOKENS") {
        return Err(invalid("Upstream generation did not finish successfully."));
    }
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut vertex_turn = None;
    if !filtered {
        let mut functions = super::function_stream::FunctionStream::new(contract);
        // Summaries are retained only in the source-scoped ordered turn, never ordinary answer text.
        for part in candidate["content"]["parts"].as_array().unwrap() {
            // Never reintroduce Full Vertex's old generic decoder's lossy empty-success fallback.
            if part.as_object().unwrap().keys().any(|k| {
                !matches!(
                    k.as_str(),
                    "text" | "functionCall" | "thought" | "thoughtSignature"
                )
            }) || part.get("text").is_some() == part.get("functionCall").is_some()
            {
                return Err(invalid("Unsupported content Part representation."));
            }
            if part.get("thought").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            if let Some(call) = part.get("functionCall") {
                if part.get("text").is_some() {
                    return Err(invalid("Ambiguous text/function part."));
                }
                if let Some(call) = functions.accept(call)? {
                    tool_calls.push(tool_call(contract, &call)?);
                }
            } else if let Some(value) = part.get("text").and_then(Value::as_str) {
                text.push_str(value);
            }
        }
        functions.finish()?;
        assign_call_ids(contract, &mut tool_calls)?;
        vertex_turn = super::turn::capture(
            contract,
            candidate["content"]["parts"].as_array().unwrap(),
            &tool_calls,
            fallback_model,
        )?;
    }
    // A safety finish invalidates the candidate, including content emitted before the finish frame.
    // This buffered bridge can withhold those parts; never expose them as executable calls.
    let finish = if filtered {
        "content_filter"
    } else if reason == "MAX_TOKENS" {
        "length"
    } else if tool_calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    };
    Ok(CanonicalRelayResponse {
        bedrock_turn: None,
        vertex_turn,
        model_declaration: None,
        model: body
            .get("modelVersion")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(fallback_model)
            .to_owned(),
        text,
        sources: Vec::new(),
        usage,
        tool_calls,
        upstream_status: Some(200),
        finish_reason: Some(finish.into()),
    })
}

fn tool_call(contract: Contract, raw: &Value) -> Result<CanonicalToolCall, GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    let name = raw
        .get("name")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| invalid("Invalid function name."))?;
    let id = raw
        .get("id")
        .map(|v| {
            v.as_str()
                .filter(|v| !v.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| invalid("Invalid function call ID."))
        })
        .transpose()?;
    let arguments = match raw.get("args") {
        Some(value) if value.is_object() => value.to_string(),
        // The assembler only emits complete calls; ordinary no-parameter calls may omit args.
        None => "{}".to_owned(),
        _ => return Err(invalid("Invalid function arguments.")),
    };
    Ok(CanonicalToolCall {
        id,
        call_type: "function".into(),
        name: Some(name.to_owned()),
        arguments: Some(arguments),
        raw: HashMap::new(),
    })
}

fn assign_call_ids(
    contract: Contract,
    calls: &mut [CanonicalToolCall],
) -> Result<(), GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    let mut used = HashSet::new();
    for id in calls.iter().filter_map(|call| call.id.as_ref()) {
        if !used.insert(id.clone()) {
            return Err(invalid("Duplicate function call ID."));
        }
    }
    // The nonce prevents identity reuse across turns. One canonical ID is then reused by every
    // public envelope/argument chunk; the ordinal also avoids any explicit-ID collision.
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let mut ordinal = 0usize;
    for call in calls.iter_mut().filter(|call| call.id.is_none()) {
        loop {
            let id = format!("call_{}_{nonce}_{ordinal}", contract.code_prefix);
            ordinal += 1;
            if used.insert(id.clone()) {
                call.id = Some(id);
                break;
            }
        }
    }
    Ok(())
}
