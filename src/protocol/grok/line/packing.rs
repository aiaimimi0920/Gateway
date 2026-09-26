//! Grok request payloads and model modes, independent of stream decoding.

use rquest::Method;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, ContentPart, MessageRole};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub(super) fn resolve_grok_model(model: &str) -> (&str, Option<&str>) {
    match model {
        "grok-3" => ("grok-3", Some("MODEL_MODE_FAST")),
        "grok-3-thinking" => ("grok-3", Some("MODEL_MODE_EXPERT")),
        "grok-3-heavy" => ("grok-3", Some("MODEL_MODE_HEAVY")),
        "grok-4" => ("grok-4", Some("MODEL_MODE_FAST")),
        "grok-4-thinking" => ("grok-4", Some("MODEL_MODE_EXPERT")),
        _ => (model, Some("MODEL_MODE_FAST")),
    }
}

pub fn pack_grok(req: &CanonicalRelayRequest, model: &str) -> Value {
    let message = combine_messages_to_text(&req.messages);
    let (model_name, model_mode) = resolve_grok_model(model);
    let mut payload = json!({
        "message": message,
        "modelName": model_name,
        "disableMemory": true,
        "disableSearch": false,
        "disableSelfHarmShortCircuit": false,
        "disableTextFollowUps": false,
        "enableImageGeneration": false,
        "enableImageStreaming": false,
        "enableSideBySide": true,
        "fileAttachments": [],
        "imageAttachments": [],
        "forceConcise": false,
        "forceSideBySide": false,
        "imageGenerationCount": 2,
        "isAsyncChat": false,
        "isReasoning": false,
        "returnImageBytes": false,
        "returnRawGrokInXaiRequest": false,
        "sendFinalMetadata": true,
        "temporary": true,
        "toolOverrides": {},
        "deviceEnvInfo": {
            "darkModeEnabled": false,
            "devicePixelRatio": 2,
            "screenHeight": 1329,
            "screenWidth": 2056,
            "viewportHeight": 1083,
            "viewportWidth": 2056,
        },
        "responseMetadata": {
            "requestModelDetails": { "modelId": model_name }
        }
    });
    if let Some(mode) = model_mode {
        payload["modelMode"] = Value::String(mode.to_string());
    }
    payload
}

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    let path = payload
        .chat_completions_path
        .as_deref()
        .unwrap_or("/rest/app-chat/conversations/new");
    Ok(RequestPlan {
        method: Method::POST,
        url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
        query: Vec::new(),
        body: Some(pack_grok(req, model)),
        response_kind: req.endpoint_kind,
    })
}

fn combine_messages_to_text(messages: &[crate::protocol::canonical::CanonicalMessage]) -> String {
    let mut parts = Vec::new();
    for msg in messages {
        let role_label = match msg.role {
            MessageRole::System => "System",
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::Tool => "Tool",
        };
        let text: String = msg
            .content
            .iter()
            .filter_map(|part| match part {
                ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            if messages.len() == 1 && msg.role == MessageRole::User {
                parts.push(text);
            } else {
                parts.push(format!("{}: {}", role_label, text));
            }
        }
    }
    parts.join("\n\n")
}
