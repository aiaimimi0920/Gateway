use serde_json::Value;
use std::collections::HashMap;

use super::defaults::GEMINI_CANVAS_VIDEO_PREVIEW_MODEL;
use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};

pub fn normalize_video_generations(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    normalize_prompt_request(
        body,
        EndpointKind::VideosGenerations,
        GEMINI_CANVAS_VIDEO_PREVIEW_MODEL,
        "Video generation requests require a prompt.",
        "missing_video_prompt",
    )
}

pub fn duration_seconds_from_request(req: &CanonicalRelayRequest) -> Option<f64> {
    req.raw_body
        .get("duration")
        .or_else(|| req.raw_body.get("duration_s"))
        .or_else(|| req.raw_body.get("durationSeconds"))
        .and_then(Value::as_f64)
}

fn normalize_prompt_request(
    body: Value,
    endpoint_kind: EndpointKind,
    default_model: &str,
    missing_message: &str,
    missing_code: &str,
) -> Result<CanonicalRelayRequest, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Media generation request body must be a JSON object")
    })?;

    if body_obj
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Err(GatewayError::bad_request(
            "Media generation endpoints do not support `stream: true`",
        )
        .with_code("media_streaming_not_supported"));
    }

    let prompt = read_prompt_body(body_obj)
        .ok_or_else(|| GatewayError::bad_request(missing_message).with_code(missing_code))?;

    let requested_model = body_obj
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| Some(default_model.to_string()));

    let explicit_session_key = body_obj
        .get("user")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind,
        requested_model,
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text { text: prompt }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: HashMap::new(),
    })
}

fn read_prompt_body(map: &serde_json::Map<String, Value>) -> Option<String> {
    for field in ["prompt", "input", "lyrics"] {
        if let Some(text) = map
            .get(field)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(text.to_string());
        }
    }

    let parts = map.get("parts")?.as_array()?;
    let mut texts = Vec::new();
    for part in parts {
        match part {
            Value::String(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    texts.push(trimmed.to_string());
                }
            }
            Value::Object(part_map) => {
                if let Some(text) = part_map
                    .get("content")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    texts.push(text.to_string());
                }
            }
            _ => {}
        }
    }

    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n"))
    }
}
