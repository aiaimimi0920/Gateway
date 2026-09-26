use super::payload_fields::{read_payload_array, read_payload_string, read_required_app_id};
use super::signing::build_signed_websocket_url;
use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
use rquest::Method;
use serde_json::{json, Map, Value};

pub fn supports_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses
    )
}

pub fn pack_request(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let app_id = read_required_app_id(payload)?;
    let uid = req
        .explicit_session_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| read_payload_string(payload.extra_body.as_ref(), &["uid", "userId"]))
        .unwrap_or_else(|| "gateway".to_string());

    let mut header = Map::new();
    header.insert("app_id".to_string(), Value::String(app_id));
    header.insert("uid".to_string(), Value::String(uid));
    if let Some(patch_id) =
        read_payload_array(payload.extra_body.as_ref(), &["patchId", "patch_id"])
    {
        header.insert("patch_id".to_string(), patch_id);
    }

    let mut chat = Map::new();
    chat.insert("domain".to_string(), Value::String(model.to_string()));

    if let Some(value) = request_value(req, &["temperature"]) {
        chat.insert("temperature".to_string(), value.clone());
    }
    if let Some(value) = request_value(req, &["top_p"]) {
        chat.insert("top_p".to_string(), value.clone());
    }
    if let Some(value) = request_value(req, &["top_k"]) {
        chat.insert("top_k".to_string(), value.clone());
    }
    if let Some(value) = request_value(req, &["max_tokens", "max_output_tokens"]) {
        chat.insert("max_tokens".to_string(), value.clone());
    }
    for (key, value) in &req.extra {
        if matches!(
            key.as_str(),
            "temperature" | "top_p" | "top_k" | "max_tokens" | "max_output_tokens"
        ) {
            continue;
        }
        if let Some(normalized) = normalize_parameter_field_name(key) {
            chat.entry(normalized.to_string())
                .or_insert_with(|| value.clone());
        }
    }

    let text_messages = req
        .messages
        .iter()
        .filter_map(pack_message)
        .collect::<Vec<_>>();
    if text_messages.is_empty() {
        return Err(GatewayError::bad_request(
            "XFYun native WebSocket requests require at least one text turn",
        )
        .with_code("xfyun_websocket_missing_messages")
        .with_provider("xfyun_websocket_compatible"));
    }

    Ok(json!({
        "header": Value::Object(header),
        "parameter": {
            "chat": Value::Object(chat),
        },
        "payload": {
            "message": {
                "text": text_messages,
            }
        }
    }))
}

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    if !supports_endpoint(req.endpoint_kind) {
        return Err(GatewayError::bad_request(
            "XFYun native WebSocket adapters support only chat/messages/responses style requests",
        )
        .with_code("unsupported_xfyun_websocket_endpoint"));
    }

    Ok(RequestPlan {
        method: Method::POST,
        url: build_signed_websocket_url(payload)?,
        query: Vec::new(),
        body: Some(pack_request(payload, req, model)?),
        response_kind: EndpointKind::ChatCompletions,
    })
}

fn pack_message(message: &CanonicalMessage) -> Option<Value> {
    let role = match message.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "user",
    };
    let content = render_message_text(message)?;
    Some(json!({
        "role": role,
        "content": content,
    }))
}

fn render_message_text(message: &CanonicalMessage) -> Option<String> {
    let mut fragments = Vec::new();
    for part in &message.content {
        match part {
            ContentPart::Text { text } => {
                if !text.trim().is_empty() {
                    fragments.push(text.clone());
                }
            }
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                let text = if let Some(text) = value.get("text").and_then(|entry| entry.as_str()) {
                    text.to_string()
                } else {
                    value.to_string()
                };
                if !text.trim().is_empty() {
                    fragments.push(text);
                }
            }
            ContentPart::ImageUrl { image_url, .. } => {
                fragments.push(format!("[image omitted: {image_url}]"));
            }
        }
    }
    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join("\n"))
    }
}

fn request_value<'a>(req: &'a CanonicalRelayRequest, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(value) = req.raw_body.get(*key) {
            return Some(value);
        }
        if let Some(value) = req.extra.get(*key) {
            return Some(value);
        }
    }
    None
}

fn normalize_parameter_field_name(key: &str) -> Option<&'static str> {
    match key.to_ascii_lowercase().as_str() {
        "temperature" => Some("temperature"),
        "top_p" | "topp" => Some("top_p"),
        "top_k" | "topk" => Some("top_k"),
        "max_tokens" | "maxoutputtokens" | "max_output_tokens" => Some("max_tokens"),
        _ => None,
    }
}
