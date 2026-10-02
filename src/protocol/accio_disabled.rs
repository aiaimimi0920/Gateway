use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};

use crate::error::{ErrorKind, FallbackHint, GatewayError};
use crate::implementation_lines;
use crate::protocol::anthropic::PromptCacheTelemetry;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, TokenUsage};

#[path = "accio/line/event_parse.rs"]
mod event_parse;
#[path = "accio/line/stream_decode.rs"]
mod stream_decode;

use stream_decode::drain_parsed_events;
pub(crate) use stream_decode::{parse_sse_line, ParsedEvent};

pub fn pack_accio(_req: &CanonicalRelayRequest, _model: &str, _stream: bool) -> Value {
    json!({ "compiledOut": true, "provider": "accio_compatible" })
}

pub fn inspect_prompt_cache_telemetry(
    _req: &CanonicalRelayRequest,
    _model: &str,
) -> PromptCacheTelemetry {
    PromptCacheTelemetry::default()
}

pub fn unpack_accio_response(_body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(
        implementation_lines::accio_web_reverse_api_compiled_out_error(
            "response decoding requested for a compiled-out Accio line",
        ),
    )
}

pub fn normalize_accio(_body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(
        implementation_lines::accio_web_reverse_api_compiled_out_error(
            "direct Accio ingress requested for a compiled-out Accio line",
        ),
    )
}

pub fn translate_accio_sse_to_openai(
    _line: &[u8],
    _model: &str,
    _response_id: &str,
    _created: i64,
) -> Option<Vec<u8>> {
    None
}

pub async fn accumulate_accio_stream(
    _response: rquest::Response,
    _model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(
        implementation_lines::accio_web_reverse_api_compiled_out_error(
            "stream accumulation requested for a compiled-out Accio line",
        ),
    )
}

pub fn translate_anthropic_like_stream_to_openai(
    _inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    _model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::empty()
}

pub fn translate_anthropic_like_stream_to_openai_with_error<E: Send + 'static>(
    _inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    _model: String,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    futures::stream::empty()
}

pub fn translate_accio_stream(
    _inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    _model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::empty()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{EndpointKind, ProtocolFamily};
    use futures::StreamExt;
    use std::collections::HashMap;

    fn request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("claude-sonnet-4-6".into()),
            stream: false,
            messages: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn assert_compiled_out(error: GatewayError, detail: &str) {
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains(detail));
    }

    #[test]
    fn disabled_request_and_response_contracts_are_explicit() {
        let request = request();
        assert_eq!(
            pack_accio(&request, "claude-sonnet-4-6", true),
            json!({ "compiledOut": true, "provider": "accio_compatible" })
        );
        assert_eq!(
            inspect_prompt_cache_telemetry(&request, "claude-sonnet-4-6"),
            PromptCacheTelemetry::default()
        );
        assert_compiled_out(
            unpack_accio_response(&json!({})).unwrap_err(),
            "response decoding requested",
        );
        assert_compiled_out(
            normalize_accio(json!({})).unwrap_err(),
            "direct Accio ingress requested",
        );
        assert!(translate_accio_sse_to_openai(b"data: {}", "model", "id", 0).is_none());
    }

    #[tokio::test]
    async fn disabled_stream_translators_are_empty() {
        let input = futures::stream::empty::<Result<Bytes, rquest::Error>>();
        let translated = translate_anthropic_like_stream_to_openai(input, "model".into());
        assert!(translated.collect::<Vec<_>>().await.is_empty());

        let input = futures::stream::empty::<Result<Bytes, rquest::Error>>();
        let translated = translate_accio_stream(input, "model".into());
        assert!(translated.collect::<Vec<_>>().await.is_empty());
    }

    #[test]
    fn shared_parser_contract_remains_available_when_accio_is_disabled() {
        let line = br#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let events = parse_sse_line(line).unwrap();
        assert!(matches!(events.as_slice(), [ParsedEvent::Text(text)] if text == "Hello"));
        assert!(matches!(
            parse_sse_line(b"data: [DONE]").unwrap().as_slice(),
            [ParsedEvent::Done]
        ));
        assert_eq!(
            normalize_tool_args("  {\"city\":\"Hangzhou\"}  "),
            "{\"city\":\"Hangzhou\"}"
        );
        assert_eq!(normalize_tool_args("  "), "{}");
    }

    #[test]
    fn provider_error_detection_remains_available_when_accio_is_disabled() {
        let line = br#"data:{"turn_complete":true,"error_code":"5015","error_message":"user not activated"}"#;
        let error = detect_accio_provider_error(line).unwrap();
        assert_eq!(error.kind, ErrorKind::ServiceUnavailable);
        assert_eq!(error.http_status, Some(503));
        assert_eq!(error.code.as_deref(), Some("5015"));
        assert_eq!(error.provider_name.as_deref(), Some("accio_compatible"));
        assert!(matches!(
            error.fallback_hint,
            FallbackHint::FallbackProvider { .. }
        ));
        assert!(error.message.contains("Accio account unavailable"));
    }
}
