use super::ParsedXfyunFrame;
use crate::error::GatewayError;
use crate::protocol::canonical::TokenUsage;
use crate::protocol::sse_parse::format_sse_event;
use bytes::Bytes;
use serde_json::{json, Value};
use std::collections::VecDeque;
use tokio_tungstenite::tungstenite::Message;

pub(super) fn parse_ws_message(
    message: Result<Message, tokio_tungstenite::tungstenite::Error>,
    model: &str,
) -> Result<ParsedXfyunFrame, GatewayError> {
    match message {
        Ok(Message::Text(text)) => parse_frame_text(text.as_ref(), model),
        Ok(Message::Binary(bytes)) => {
            let text = String::from_utf8(bytes.to_vec()).map_err(|error| {
                GatewayError::service_unavailable(format!(
                    "XFYun native WebSocket returned invalid UTF-8 frame: {error}"
                ))
                .with_code("xfyun_websocket_invalid_utf8")
                .with_provider("xfyun_websocket_compatible")
            })?;
            parse_frame_text(&text, model)
        }
        Ok(Message::Close(_)) => Ok(ParsedXfyunFrame {
            done: true,
            ..ParsedXfyunFrame::default()
        }),
        Ok(Message::Ping(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => {
            Ok(ParsedXfyunFrame::default())
        }
        Err(error) => Err(GatewayError::service_unavailable(format!(
            "XFYun native WebSocket transport error: {error}"
        ))
        .with_code("xfyun_websocket_transport_failed")
        .with_provider("xfyun_websocket_compatible")),
    }
}

pub(super) fn parse_frame_text(text: &str, _model: &str) -> Result<ParsedXfyunFrame, GatewayError> {
    let value: Value = serde_json::from_str(text).map_err(|error| {
        GatewayError::service_unavailable(format!(
            "XFYun native WebSocket returned invalid JSON frame: {error}"
        ))
        .with_code("xfyun_websocket_invalid_json")
        .with_provider("xfyun_websocket_compatible")
    })?;
    let code = value
        .pointer("/header/code")
        .and_then(|entry| entry.as_i64())
        .unwrap_or(0);
    if code != 0 {
        let message = value
            .pointer("/header/message")
            .and_then(|entry| entry.as_str())
            .unwrap_or("unknown upstream error");
        return Err(classify_protocol_error(code, message));
    }

    let mut content_fragments = Vec::new();
    if let Some(items) = value
        .pointer("/payload/choices/text")
        .and_then(|entry| entry.as_array())
    {
        for item in items {
            if let Some(content) = item.get("content").and_then(|entry| entry.as_str()) {
                if !content.is_empty() {
                    content_fragments.push(content.to_string());
                }
            }
        }
    }

    let status = value
        .pointer("/payload/choices/status")
        .and_then(|entry| entry.as_u64())
        .unwrap_or(0);
    let usage = parse_usage(value.pointer("/payload/usage/text"));

    if content_fragments.is_empty() && status == 0 && usage.is_none() {
        // Native XFYun WebSocket surfaces can emit transient empty status=0
        // frames before the first text delta. Treat those as no-op keepalive
        // frames instead of aborting the caller-visible stream.
        return Ok(ParsedXfyunFrame::default());
    }

    Ok(ParsedXfyunFrame {
        content_fragments,
        usage,
        done: status == 2,
    })
}

fn classify_protocol_error(code: i64, message: &str) -> GatewayError {
    let normalized = message.to_ascii_lowercase();
    let error = if normalized.contains("auth")
        || normalized.contains("signature")
        || normalized.contains("apikey")
        || normalized.contains("api key")
        || normalized.contains("secret")
        || normalized.contains("appid")
        || normalized.contains("permission")
    {
        GatewayError::unauthorized(format!("XFYun native WebSocket auth failed: {message}"))
            .with_code(format!("xfyun_websocket_{code}"))
    } else if normalized.contains("qps")
        || normalized.contains("rate")
        || normalized.contains("limit")
        || normalized.contains("too many")
    {
        GatewayError::rate_limited(
            format!("XFYun native WebSocket rate limited: {message}"),
            1000,
        )
        .with_code(format!("xfyun_websocket_{code}"))
    } else {
        GatewayError::service_unavailable(format!(
            "XFYun native WebSocket upstream error {code}: {message}"
        ))
        .with_code(format!("xfyun_websocket_{code}"))
    };
    error.with_provider("xfyun_websocket_compatible")
}

fn parse_usage(value: Option<&Value>) -> Option<TokenUsage> {
    let value = value?;
    let prompt_tokens =
        read_usage_field(value, &["prompt_tokens", "question_tokens", "input_tokens"])?;
    let completion_tokens = read_usage_field(
        value,
        &["completion_tokens", "answer_tokens", "output_tokens"],
    )
    .unwrap_or(0);
    let total_tokens = read_usage_field(value, &["total_tokens"])
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    })
}

fn read_usage_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(|entry| entry.as_u64()))
}

pub(super) fn build_openai_sse_frames(
    parsed: &ParsedXfyunFrame,
    model: &str,
    response_id: &str,
    created_at: i64,
) -> VecDeque<Bytes> {
    let mut frames = VecDeque::new();
    for fragment in &parsed.content_fragments {
        let payload = json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created_at,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {
                    "content": fragment,
                },
                "finish_reason": Value::Null,
            }],
        });
        frames.push_back(Bytes::from(format_sse_event(None, &payload.to_string())));
    }

    if parsed.done {
        let mut payload = json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created_at,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop",
            }],
        });
        if let Some(usage) = &parsed.usage {
            payload["usage"] = json!({
                "prompt_tokens": usage.prompt_tokens,
                "completion_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens,
            });
        }
        frames.push_back(Bytes::from(format_sse_event(None, &payload.to_string())));
        frames.push_back(Bytes::from_static(b"data: [DONE]\n\n"));
    }

    frames
}
