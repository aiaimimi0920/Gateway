//! Bedrock Converse public API, success envelopes and endpoint paths.

mod eventstream;
mod exception;
mod generation;
mod generation_validation;
mod history;
mod messages;
mod normalization;
mod packing;
mod public_history;
pub(crate) mod public_tools;
mod upstream;
pub(crate) use upstream::decode_eventstream;
pub(crate) use upstream::Relay as StreamRelay;
pub(crate) use upstream::{translate_public, translate_scoped};

pub use upstream::{
    accumulate as accumulate_bedrock_eventstream,
    translate as translate_bedrock_eventstream_to_openai_with_error,
    unpack as unpack_bedrock_response,
};

#[cfg(test)]
mod tests;

pub(crate) use packing::{finish_plan, try_pack_for_plan};
pub use packing::{pack_bedrock_converse, try_pack_bedrock_converse};

pub use normalization::normalize_converse;

pub(crate) use eventstream::encode_failure as encode_native_failure;
pub(crate) use eventstream::translate_with_started_at as encode_native_stream_with_started_at;
pub use eventstream::{
    translate_openai_sse_to_bedrock_eventstream,
    translate_openai_sse_to_bedrock_eventstream_with_error,
};

use crate::protocol::canonical::CanonicalToolCall;
use crate::protocol::canonical::TokenUsage;
use serde_json::json;
use serde_json::Value;

pub fn build_converse_success(
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    let mut content = Vec::new();
    if !text.is_empty() || tool_calls.is_empty() {
        content.push(json!({"text": text}));
    }
    for tool_call in tool_calls {
        let input = tool_call
            .arguments
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .unwrap_or_else(|| json!({}));
        content.push(json!({
            "toolUse": {
                "toolUseId": tool_call.id,
                "name": tool_call.name,
                "input": input,
            }
        }));
    }

    let mut body = json!({
        "output": {
            "message": {
                "role": "assistant",
                "content": content,
            }
        },
        "stopReason": map_bedrock_finish_reason(finish_reason, tool_calls),
        "model": model,
    });

    if let Some(usage) = usage {
        body["usage"] = json!({
            "inputTokens": usage.prompt_tokens,
            "outputTokens": usage.completion_tokens,
            "totalTokens": usage.total_tokens,
        });
    }

    body
}

/// Rebuild native content from validated ordered state when the response has reasoning.
pub fn build_converse_response(
    response: &crate::protocol::canonical::CanonicalRelayResponse,
) -> Value {
    let mut body = build_converse_success(
        &response.model,
        &response.text,
        response.usage.as_ref(),
        &response.tool_calls,
        response.finish_reason.as_deref(),
    );
    if let Some(turn) = &response.bedrock_turn {
        body["output"]["message"]["content"] = turn.native_content();
    }
    body
}

pub fn default_path(model: &str, stream: bool) -> String {
    // Encode the non-greedy URI label, including ARN ':' and '/'. Preserve unreserved IDs.
    const LABEL: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');
    let model = percent_encoding::utf8_percent_encode(model, LABEL);
    if stream {
        format!("/model/{model}/converse-stream")
    } else {
        format!("/model/{model}/converse")
    }
}

pub(crate) fn explicit_auth_mode(mode: Option<&str>) -> bool {
    matches!(
        mode.map(str::trim),
        Some("bedrock_bearer" | "aws_sigv4" | "aws_sigv4_temporary")
    )
}

fn map_bedrock_finish_reason(
    finish_reason: Option<&str>,
    tool_calls: &[CanonicalToolCall],
) -> &'static str {
    match finish_reason.unwrap_or(if tool_calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    }) {
        "tool_calls" => "tool_use",
        "length" => "max_tokens",
        "content_filter" => "guardrail_intervened",
        _ => "end_turn",
    }
}
