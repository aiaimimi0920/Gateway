//! Bedrock Converse public API, success envelopes and endpoint paths.

mod eventstream;
mod normalization;
mod packing;

#[cfg(test)]
mod tests;

pub use packing::pack_bedrock_converse;

pub use normalization::normalize_converse;

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

pub fn default_path(model: &str, stream: bool) -> String {
    if stream {
        format!("/model/{model}/converse-stream")
    } else {
        format!("/model/{model}/converse")
    }
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
