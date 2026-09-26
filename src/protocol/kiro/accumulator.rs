//! Nonstream Kiro response accumulation and final response assembly.
use super::accumulator_content::{AccumulatedContent, AccumulationLimits};

use super::{
    build_tool_name_map, classify_kiro_provider_error, estimate_request_prompt_tokens,
    estimate_tokens, map_model, prompt_tokens_from_context, EventStreamParser, KiroEvent,
};
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, TokenUsage};

pub async fn accumulate_kiro_stream(
    response: rquest::Response,
    model: &str,
    req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayResponse, GatewayError> {
    accumulate_with_limits(response, model, req, AccumulationLimits::default()).await
}

pub(super) async fn accumulate_with_limits(
    response: rquest::Response,
    model: &str,
    req: &CanonicalRelayRequest,
    limits: AccumulationLimits,
) -> Result<CanonicalRelayResponse, GatewayError> {
    use futures::StreamExt;

    let tool_name_map = build_tool_name_map(req);
    let mut parser = EventStreamParser::default();
    let mut content = AccumulatedContent::new(limits);
    let mut prompt_tokens = estimate_request_prompt_tokens(req, model);
    let mut completion_tokens = 0u64;
    let mut context_overflow = false;
    let mut tool_calls_seen = false;

    let mut stream = Box::pin(response.bytes_stream());
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| GatewayError::server_error(format!("read kiro stream: {e}")))?;
        parser.push(chunk)?;
        while let Some(event) = parser.next_event()? {
            match event {
                KiroEvent::AssistantResponse { content: delta } => {
                    completion_tokens = completion_tokens.saturating_add(estimate_tokens(&delta));
                    content.push_text(&delta)?;
                }
                KiroEvent::ToolUse {
                    name,
                    tool_use_id,
                    input,
                    ..
                } => {
                    tool_calls_seen = true;
                    completion_tokens = completion_tokens.saturating_add(estimate_tokens(&input));
                    content.push_tool(tool_use_id, name, input, &tool_name_map)?;
                }
                KiroEvent::ContextUsage {
                    context_usage_percentage,
                } => {
                    prompt_tokens = prompt_tokens_from_context(model, context_usage_percentage)
                        .max(prompt_tokens);
                    if context_usage_percentage >= 100.0 {
                        context_overflow = true;
                    }
                }
                KiroEvent::Error {
                    error_code,
                    error_message,
                } => {
                    return Err(classify_kiro_provider_error(&error_code, &error_message));
                }
                KiroEvent::Exception {
                    exception_type,
                    message,
                } => {
                    if exception_type == "ContentLengthExceededException" {
                        context_overflow = true;
                    } else {
                        return Err(GatewayError::server_error(format!(
                            "Kiro upstream exception: {exception_type}: {message}"
                        ))
                        .with_provider("kiro_compatible")
                        .with_code(exception_type));
                    }
                }
                KiroEvent::Unknown => {}
            }
        }
    }
    parser.finish()?;

    let (text, tool_calls) = content.finish()?;

    let finish_reason = if !tool_calls.is_empty() || tool_calls_seen {
        Some("tool_calls".to_string())
    } else if context_overflow {
        Some("length".to_string())
    } else {
        Some("stop".to_string())
    };

    Ok(CanonicalRelayResponse {
        model: map_model(model),
        text,
        usage: Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.saturating_add(completion_tokens),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }),
        tool_calls,
        upstream_status: Some(200),
        finish_reason,
    })
}
