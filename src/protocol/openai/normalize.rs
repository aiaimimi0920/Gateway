use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, ContentPart, EndpointKind, MessageRole,
    ProtocolFamily,
};

use super::tool_calls::parse_openai_message_tool_calls;

/// Normalize an OpenAI `POST /v1/chat/completions` request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_chat_completions(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // ── messages ──────────────────────────────────────────────────────────

    let raw_messages = body
        .get("messages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| GatewayError::bad_request("missing or invalid `messages` array"))?;

    let mut messages: Vec<CanonicalMessage> = Vec::with_capacity(raw_messages.len());

    for raw_msg in raw_messages {
        let role_str = raw_msg
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::bad_request("message missing `role` field"))?;

        let role = parse_role(role_str)?;

        let content = parse_openai_content(raw_msg.get("content"))?;

        let name = raw_msg
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let tool_call_id = raw_msg
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let tool_calls = parse_openai_message_tool_calls(raw_msg);

        messages.push(CanonicalMessage {
            role,
            content,
            name,
            tool_call_id,
            tool_calls,
        });
    }

    // ── tools ─────────────────────────────────────────────────────────────

    let tools = parse_openai_tools(body.get("tools"));
    let tool_choice = body.get("tool_choice").cloned();

    // ── extra (passthrough) parameters ────────────────────────────────────

    // Known top-level fields that are handled explicitly above.
    const KNOWN_FIELDS: &[&str] = &[
        "model",
        "messages",
        "stream",
        "tools",
        "tool_choice",
        "user",
        "reasoning",
    ];

    let mut extra = std::collections::HashMap::new();
    // Capture ALL unknown fields into extra so vendor extensions are preserved.
    if let Value::Object(map) = &body {
        for (key, value) in map {
            if !KNOWN_FIELDS.contains(&key.as_str()) {
                extra.insert(key.clone(), value.clone());
            }
        }
    }

    // ── session / user ────────────────────────────────────────────────────

    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: model,
        stream,
        messages,
        tools,
        tool_choice,
        reasoning: body.get("reasoning").cloned(),
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra,
    })
}

/// Normalize an OpenAI `POST /v1/completions` request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_legacy_completions(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let prompt = body
        .get("prompt")
        .ok_or_else(|| GatewayError::bad_request("missing `prompt` field"))?;
    let messages = parse_prompt_messages(prompt);
    // Legacy-only controls stay in raw_body; common generation limits must
    // survive conversion to chat and other conversational protocols.
    let extra = [
        "max_tokens",
        "temperature",
        "top_p",
        "stop",
        "presence_penalty",
        "frequency_penalty",
        "seed",
    ]
    .into_iter()
    .filter_map(|key| body.get(key).map(|value| (key.to_owned(), value.clone())))
    .collect();
    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::Completions,
        requested_model: model,
        stream,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra,
    })
}

/// Normalize an OpenAI `POST /v1/embeddings` request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_embeddings(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let input = body
        .get("input")
        .ok_or_else(|| GatewayError::bad_request("missing `input` field"))?;
    let messages = parse_prompt_messages(input);
    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::Embeddings,
        requested_model: model,
        stream: false,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}

/// Normalize an OpenAI `POST /v1/audio/speech` request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_audio_speech(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let input = body
        .get("input")
        .ok_or_else(|| GatewayError::bad_request("missing `input` field"))?;
    let messages = parse_prompt_messages(input);
    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::AudioSpeech,
        requested_model: model,
        stream: false,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}

/// Normalize an OpenAI `POST /v1/audio/transcriptions` request body into a
/// [`CanonicalRelayRequest`].
///
/// The route layer converts multipart form data into a JSON object so the
/// canonical pipeline can still inspect, route, quota-check, and audit the
/// request without carrying raw multipart bytes through the hot path.
pub fn normalize_audio_transcriptions(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let file = body
        .get("file")
        .and_then(|value| value.as_object())
        .ok_or_else(|| GatewayError::bad_request("missing `file` field"))?;
    let file_name = file
        .get("file_name")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let prompt = body
        .get("prompt")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let message_seed = prompt
        .or_else(|| file_name.map(|value| format!("transcribe {}", value)))
        .unwrap_or_else(|| "audio transcription request".to_string());
    let messages = vec![CanonicalMessage {
        role: MessageRole::User,
        content: vec![ContentPart::Text { text: message_seed }],
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::AudioTranscriptions,
        requested_model: model,
        stream: false,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}

fn parse_role(role: &str) -> Result<MessageRole, GatewayError> {
    match role {
        "system" => Ok(MessageRole::System),
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "tool" | "function" => Ok(MessageRole::Tool),
        other => Err(GatewayError::bad_request(format!(
            "unknown message role: {other}"
        ))),
    }
}

fn parse_prompt_messages(value: &Value) -> Vec<CanonicalMessage> {
    match value {
        Value::String(text) => vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text { text: text.clone() }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        Value::Array(items) => items
            .iter()
            .map(|item| CanonicalMessage {
                role: MessageRole::User,
                content: vec![match item {
                    Value::String(text) => ContentPart::Text { text: text.clone() },
                    _ => ContentPart::Raw {
                        value: item.clone(),
                    },
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            })
            .collect(),
        other => vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Raw {
                value: other.clone(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
    }
}

fn parse_openai_content(content: Option<&Value>) -> Result<Vec<ContentPart>, GatewayError> {
    match content {
        None | Some(Value::Null) => Ok(vec![]),

        // Simple string content.
        Some(Value::String(s)) => Ok(vec![ContentPart::Text { text: s.clone() }]),

        // Array of content parts.
        Some(Value::Array(parts)) => {
            let mut out = Vec::with_capacity(parts.len());
            for part in parts {
                let kind = part.get("type").and_then(|v| v.as_str()).unwrap_or("text");

                match kind {
                    "text" => {
                        let text = part
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if is_plain_text_block(part) {
                            out.push(ContentPart::Text { text });
                        } else {
                            out.push(ContentPart::Raw {
                                value: part.clone(),
                            });
                        }
                    }
                    "image_url" => {
                        let url = part
                            .get("image_url")
                            .and_then(|u| u.get("url"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let detail = part
                            .get("image_url")
                            .and_then(|u| u.get("detail"))
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        out.push(ContentPart::ImageUrl {
                            image_url: url,
                            detail,
                        });
                    }
                    _ => {
                        // Preserve unknown parts as Raw.
                        out.push(ContentPart::Raw {
                            value: part.clone(),
                        });
                    }
                }
            }
            Ok(out)
        }

        Some(other) => {
            // Fallback: treat any other JSON value as a raw part.
            Ok(vec![ContentPart::Raw {
                value: other.clone(),
            }])
        }
    }
}

fn parse_openai_tools(raw: Option<&Value>) -> Vec<CanonicalTool> {
    let arr = match raw.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return vec![],
    };

    arr.iter()
        .map(|t| {
            let fn_obj = t.get("function");
            CanonicalTool {
                tool_type: t
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("function")
                    .to_string(),
                name: fn_obj
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                description: fn_obj
                    .and_then(|f| f.get("description"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                input_schema: fn_obj.and_then(|f| f.get("parameters")).cloned(),
                raw: t
                    .as_object()
                    .map(|map| {
                        map.iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect()
}

fn is_plain_text_block(part: &Value) -> bool {
    part.as_object().is_some_and(|map| {
        map.keys()
            .all(|key| matches!(key.as_str(), "type" | "text"))
    })
}
