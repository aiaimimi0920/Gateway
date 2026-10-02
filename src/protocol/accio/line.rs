use serde_json::Value;
use std::collections::HashMap;

use crate::error::{ErrorKind, FallbackHint, GatewayError};
use crate::protocol::canonical::{
    CanonicalRelayRequest, CanonicalRelayResponse, CanonicalToolCall, TokenUsage,
};
use crate::protocol::upstream_body::collect_bounded_upstream_body;

mod event_parse;
mod request;
mod request_contents;
mod stream_decode;
mod stream_translate;

pub use request::{inspect_prompt_cache_telemetry, pack_accio};
use stream_decode::drain_parsed_events;
pub(crate) use stream_decode::{parse_sse_line, ParsedEvent};
pub use stream_translate::{
    translate_accio_sse_to_openai, translate_accio_stream,
    translate_anthropic_like_stream_to_openai,
    translate_anthropic_like_stream_to_openai_with_error,
};

pub fn unpack_accio_response(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for event in event_parse::parse_raw_event(body) {
        match event {
            ParsedEvent::ProviderError { code, message } => {
                return Err(classify_accio_provider_error(&code, &message));
            }
            ParsedEvent::Text(delta) => text.push_str(&delta),
            ParsedEvent::ToolCall {
                id,
                name,
                arguments,
            } => tool_calls.push(CanonicalToolCall {
                id: Some(id),
                call_type: "function".into(),
                name: Some(name),
                arguments: Some(arguments),
                raw: HashMap::new(),
            }),
            _ => {}
        }
    }

    let finish_reason = map_finish_reason(
        body.get("finishReason")
            .or_else(|| body.get("stopReason"))
            .or_else(|| body.get("finish_reason"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                body.get("candidates")
                    .and_then(|v| v.as_array())
                    .and_then(|v| v.first())
                    .and_then(|v| v.get("finishReason"))
                    .and_then(|v| v.as_str())
            }),
    )
    .or_else(|| {
        if tool_calls.is_empty() {
            Some("stop".into())
        } else {
            Some("tool_calls".into())
        }
    });

    Ok(CanonicalRelayResponse {
        model: body
            .get("modelVersion")
            .or_else(|| body.get("model"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        text,
        usage: usage_from_value(Some(body)),
        tool_calls,
        upstream_status: Some(200),
        finish_reason,
    })
}

pub fn normalize_accio(_body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(GatewayError::bad_request(
        "Direct Accio format input not yet supported",
    ))
}

pub async fn accumulate_accio_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let body = collect_bounded_upstream_body(response, "Accio streaming adapter body").await?;
    let mut text = String::new();
    let mut prompt_tokens = 0u64;
    let mut completion_tokens = 0u64;
    let mut finish_reason = None;
    let mut reported_model = model.to_string();
    let mut tool_calls = Vec::new();
    let mut pending: HashMap<i64, PendingToolCall> = HashMap::new();

    let mut buffer = body.clone();
    for event in drain_parsed_events(&mut buffer) {
        match event {
            ParsedEvent::ProviderError { code, message } => {
                return Err(classify_accio_provider_error(&code, &message));
            }
            ParsedEvent::Start { model, usage } => {
                if let Some(model) = model {
                    reported_model = model;
                }
                if let Some(usage) = usage {
                    prompt_tokens = prompt_tokens.max(usage.prompt_tokens);
                    completion_tokens = completion_tokens.max(usage.completion_tokens);
                }
            }
            ParsedEvent::Text(delta) => text.push_str(&delta),
            ParsedEvent::ToolStart { index, id, name } => {
                pending.insert(
                    index,
                    PendingToolCall {
                        id,
                        name,
                        arguments: String::new(),
                        announced: false,
                    },
                );
            }
            ParsedEvent::ToolDelta { index, partial } => {
                if let Some(call) = pending.get_mut(&index) {
                    call.arguments.push_str(&partial);
                }
            }
            ParsedEvent::ToolEnd { index } => {
                if let Some(call) = pending.remove(&index) {
                    tool_calls.push(to_tool_call(call));
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
                raw: HashMap::new(),
            }),
            ParsedEvent::Finish { reason, usage } => {
                if let Some(reason) = reason {
                    finish_reason = Some(reason);
                }
                if let Some(usage) = usage {
                    prompt_tokens = prompt_tokens.max(usage.prompt_tokens);
                    completion_tokens = completion_tokens.max(usage.completion_tokens);
                }
            }
            ParsedEvent::Done => {}
        }
    }

    for (_, call) in pending {
        tool_calls.push(to_tool_call(call));
    }

    if text.is_empty() && tool_calls.is_empty() {
        if let Some(error) = detect_accio_html_challenge(&body) {
            return Err(error);
        }
    }

    let total_tokens = prompt_tokens + completion_tokens;
    let usage = if total_tokens > 0 {
        Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
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
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
    announced: bool,
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

pub(crate) fn normalize_tool_args(arguments: &str) -> String {
    let arguments = arguments.trim();
    if arguments.is_empty() {
        "{}".into()
    } else {
        arguments.into()
    }
}

fn map_finish_reason(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        match value {
            "end_turn" | "stop" | "stop_sequence" | "complete" | "COMPLETE" => "stop",
            "tool_use" | "tool_call" | "TOOL_CALL" | "function_call" => "tool_calls",
            "max_tokens" | "length" | "MAX_TOKENS" => "length",
            other => other,
        }
        .to_string(),
    )
}

fn usage_from_value(value: Option<&Value>) -> Option<TokenUsage> {
    let value = value?;
    let usage = value
        .get("usageMetadata")
        .or_else(|| value.get("usage"))
        .unwrap_or(value);
    let prompt_tokens = usage
        .get("promptTokenCount")
        .or_else(|| usage.get("inputTokens"))
        .or_else(|| usage.get("input_tokens"))
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let completion_tokens = usage
        .get("candidatesTokenCount")
        .or_else(|| usage.get("outputTokens"))
        .or_else(|| usage.get("output_tokens"))
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .or_else(|| usage.get("totalTokens"))
        .or_else(|| usage.get("total_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(prompt_tokens + completion_tokens);

    if prompt_tokens == 0 && completion_tokens == 0 && total_tokens == 0 {
        None
    } else {
        Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_creation_input_tokens: usage
                .get("cache_creation_input_tokens")
                .and_then(|entry| entry.as_u64()),
            cache_read_input_tokens: usage
                .get("cache_read_input_tokens")
                .and_then(|entry| entry.as_u64())
                .or_else(|| {
                    usage
                        .get("prompt_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(|entry| entry.as_u64())
                })
                .or_else(|| {
                    usage
                        .get("input_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(|entry| entry.as_u64())
                }),
        })
    }
}

fn guess_image_mime(image_url: &str) -> &'static str {
    let lower = image_url.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/png"
    }
}

pub fn detect_accio_provider_error(chunk: &[u8]) -> Option<GatewayError> {
    let mut buffer = chunk.to_vec();
    for event in drain_parsed_events(&mut buffer) {
        if let ParsedEvent::ProviderError { code, message } = event {
            return Some(classify_accio_provider_error(&code, &message));
        }
    }
    None
}

fn parse_provider_error(raw: &Value) -> Option<ParsedEvent> {
    let turn_complete = raw
        .get("turn_complete")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let code = raw
        .get("error_code")
        .or_else(|| raw.get("errorCode"))
        .and_then(value_to_string)?;
    if !turn_complete && code.is_empty() {
        return None;
    }
    if matches!(code.as_str(), "" | "0" | "200") {
        return None;
    }
    let message = raw
        .get("error_message")
        .or_else(|| raw.get("errorMessage"))
        .and_then(value_to_string)
        .unwrap_or_else(|| "unknown accio upstream error".to_string());
    Some(ParsedEvent::ProviderError { code, message })
}

fn detect_accio_html_challenge(body: &[u8]) -> Option<GatewayError> {
    let text = String::from_utf8_lossy(body);
    let lower = text.to_ascii_lowercase();
    if !lower.contains("<html") {
        return None;
    }
    if !(lower.contains("punish-component")
        || lower.contains("captcha")
        || lower.contains("baxia")
        || lower.contains("htmltocanvas"))
    {
        return None;
    }
    Some(GatewayError {
        kind: ErrorKind::ServiceUnavailable,
        message: "Accio upstream returned an anti-bot challenge page instead of an event stream."
            .to_string(),
        code: Some("accio_upstream_html_challenge".to_string()),
        http_status: Some(503),
        retryable: false,
        fallback_hint: FallbackHint::FallbackProvider {
            reason: "Accio upstream challenged the direct replay request; try another credential or a challenge-capable runtime."
                .to_string(),
        },
        provider_name: Some("accio_compatible".to_string()),
    })
}

fn classify_accio_provider_error(code: &str, message: &str) -> GatewayError {
    let lower = message.to_ascii_lowercase();
    let err = if code == "5015" || lower.contains("user not activated") {
        GatewayError {
            kind: ErrorKind::ServiceUnavailable,
            message: format!("Accio account unavailable: {}", message.trim()),
            code: None,
            http_status: Some(503),
            retryable: false,
            fallback_hint: FallbackHint::FallbackProvider {
                reason: "Accio account is not activated; try another provider account.".to_string(),
            },
            provider_name: None,
        }
    } else {
        GatewayError::server_error(format!("Accio upstream error: {}", message.trim()))
    };
    err.with_code(code.to_string())
        .with_provider("accio_compatible")
}

fn to_tool_call(call: PendingToolCall) -> CanonicalToolCall {
    CanonicalToolCall {
        id: Some(call.id),
        call_type: "function".into(),
        name: Some(call.name),
        arguments: Some(normalize_tool_args(&call.arguments)),
        raw: HashMap::new(),
    }
}

#[cfg(test)]
mod tests;
