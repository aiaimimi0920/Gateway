use std::collections::BTreeMap;

use crate::error::GatewayError;
use crate::protocol::accio::{
    normalize_tool_args, parse_sse_line as parse_anthropic_sse_line, ParsedEvent,
};
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall, TokenUsage};
use crate::protocol::upstream_body::collect_bounded_upstream_text;

/// Parse a single OpenAI SSE line and convert to Anthropic SSE event bytes.
///
/// Returns `None` for non-content lines (empty, comments, event: lines).
pub async fn accumulate_anthropic_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let body = collect_bounded_upstream_text(response, "Anthropic SSE body").await?;
    let mut text = String::new();
    let mut prompt_tokens = 0u64;
    let mut completion_tokens = 0u64;
    let mut cache_creation_input_tokens = None;
    let mut cache_read_input_tokens = None;
    let mut finish_reason = None;
    let mut reported_model = model.to_string();
    let mut tool_calls = Vec::new();
    let mut pending_tools: BTreeMap<i64, AnthropicPendingToolCall> = BTreeMap::new();

    for line in body.lines() {
        let Some(events) = parse_anthropic_sse_line(line.as_bytes()) else {
            continue;
        };
        for event in events {
            match event {
                ParsedEvent::ProviderError { code, message } => {
                    return Err(GatewayError::server_error(format!(
                        "Anthropic upstream stream error {code}: {message}"
                    )));
                }
                ParsedEvent::Start { model, usage } => {
                    if let Some(model) = model {
                        reported_model = model;
                    }
                    if let Some(usage) = usage {
                        prompt_tokens = prompt_tokens.max(usage.prompt_tokens);
                        completion_tokens = completion_tokens.max(usage.completion_tokens);
                        cache_creation_input_tokens = usage
                            .cache_creation_input_tokens
                            .or(cache_creation_input_tokens);
                        cache_read_input_tokens =
                            usage.cache_read_input_tokens.or(cache_read_input_tokens);
                    }
                }
                ParsedEvent::Text(delta) => text.push_str(&delta),
                ParsedEvent::ToolStart { index, id, name } => {
                    pending_tools.insert(
                        index,
                        AnthropicPendingToolCall {
                            id,
                            name,
                            arguments: String::new(),
                        },
                    );
                }
                ParsedEvent::ToolDelta { index, partial } => {
                    if let Some(call) = pending_tools.get_mut(&index) {
                        call.arguments.push_str(&partial);
                    }
                }
                ParsedEvent::ToolEnd { index } => {
                    if let Some(call) = pending_tools.remove(&index) {
                        tool_calls.push(CanonicalToolCall {
                            id: Some(call.id),
                            call_type: "function".into(),
                            name: Some(call.name),
                            arguments: Some(normalize_tool_args(&call.arguments)),
                            raw: std::collections::HashMap::new(),
                        });
                    }
                }
                ParsedEvent::ToolCall {
                    id,
                    name,
                    arguments,
                } => tool_calls.push(CanonicalToolCall {
                    id: Some(id),
                    call_type: "function".into(),
                    name: Some(name),
                    arguments: Some(normalize_tool_args(&arguments)),
                    raw: std::collections::HashMap::new(),
                }),
                ParsedEvent::Finish { reason, usage } => {
                    if let Some(reason) = reason {
                        finish_reason = Some(reason);
                    }
                    if let Some(usage) = usage {
                        prompt_tokens = prompt_tokens.max(usage.prompt_tokens);
                        completion_tokens = completion_tokens.max(usage.completion_tokens);
                        cache_creation_input_tokens = usage
                            .cache_creation_input_tokens
                            .or(cache_creation_input_tokens);
                        cache_read_input_tokens =
                            usage.cache_read_input_tokens.or(cache_read_input_tokens);
                    }
                }
                ParsedEvent::Done => {}
            }
        }
    }

    for (_, call) in pending_tools {
        tool_calls.push(CanonicalToolCall {
            id: Some(call.id),
            call_type: "function".into(),
            name: Some(call.name),
            arguments: Some(normalize_tool_args(&call.arguments)),
            raw: std::collections::HashMap::new(),
        });
    }

    let total_tokens = prompt_tokens.saturating_add(completion_tokens);
    let usage = if total_tokens > 0
        || cache_creation_input_tokens.is_some()
        || cache_read_input_tokens.is_some()
    {
        Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
        })
    } else {
        None
    };

    Ok(CanonicalRelayResponse {
        model: reported_model,
        text,
        usage,
        tool_calls,
        upstream_status: Some(200),
        finish_reason: Some(finish_reason.unwrap_or_else(|| "stop".into())),
    })
}

#[derive(Debug, Clone)]
struct AnthropicPendingToolCall {
    id: String,
    name: String,
    arguments: String,
}
