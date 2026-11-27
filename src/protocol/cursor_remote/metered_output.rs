//! Native terminal envelopes with actual whole-request usage, never zero-filled.
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, ProtocolFamily},
};
use bytes::Bytes;
use serde_json::{json, Value};

pub(crate) fn supports(family: ProtocolFamily) -> bool {
    matches!(
        family,
        ProtocolFamily::BedrockConverse
            | ProtocolFamily::DashScope
            | ProtocolFamily::DashScopeMultimodal
    )
}

pub(crate) fn response(
    req: &CanonicalRelayRequest,
    completed: &CanonicalRelayResponse,
    elapsed: std::time::Duration,
) -> Result<Value, GatewayError> {
    if !completed.tool_calls.is_empty() {
        return Err(super::error("unmapped_tool_usage"));
    }
    let usage = completed
        .usage
        .as_ref()
        .ok_or_else(|| super::error("token_usage_unavailable"))?;
    if req.protocol_family == ProtocolFamily::BedrockConverse {
        if [
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens,
            usage.cache_read_input_tokens.unwrap_or(0),
            usage.cache_creation_input_tokens.unwrap_or(0),
        ]
        .iter()
        .any(|n| *n > i32::MAX as u64)
        {
            return Err(super::error("invalid_token_usage"));
        }
        let mut wire = json!({"inputTokens":usage.prompt_tokens,
            "outputTokens":usage.completion_tokens,"totalTokens":usage.total_tokens});
        if let Some(read) = usage.cache_read_input_tokens {
            wire["cacheReadInputTokens"] = json!(read);
        }
        if let Some(write) = usage.cache_creation_input_tokens {
            wire["cacheWriteInputTokens"] = json!(write);
        }
        return Ok(
            json!({"output":{"message":{"role":"assistant","content":[{"text":completed.text}]}},
            "stopReason":"end_turn","usage":wire,
            // Explicitly Gateway elapsed time, not a claimed AWS service measurement.
            "metrics":{"latencyMs":elapsed.as_millis().min(i64::MAX as u128) as u64}}),
        );
    }
    let multimodal = req.protocol_family == ProtocolFamily::DashScopeMultimodal;
    let output = if multimodal
        || req
            .raw_body
            .pointer("/parameters/result_format")
            .is_some_and(|v| v == "message")
    {
        json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant",
            "content":if multimodal {json!([{"text":completed.text}])} else {json!(completed.text)}}}]})
    } else {
        json!({"text":completed.text,"finish_reason":"stop"})
    };
    Ok(
        json!({"request_id":format!("cursor_{}", uuid::Uuid::new_v4().simple()),
        "output":output,"usage":{"input_tokens":usage.prompt_tokens,
            "output_tokens":usage.completion_tokens,"total_tokens":usage.total_tokens}}),
    )
}

pub(crate) fn frames(family: ProtocolFamily, value: &Value) -> Result<Vec<Bytes>, GatewayError> {
    if family == ProtocolFamily::BedrockConverse {
        return crate::protocol::bedrock_converse::encode_buffered_text(value)
            .map_err(|_| super::error("worker_protocol"));
    }
    // A single terminal phase is both cumulative and incremental. No false token timing.
    Ok(vec![Bytes::from(format!(
        "event: result\ndata: {value}\n\n"
    ))])
}
