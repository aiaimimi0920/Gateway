//! Conservative current text contract: no flattening, parameter loss, or model-version guesses.
use super::failure;
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::canonical::{CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole},
};
use serde_json::{json, Value};

pub(super) fn mode(model: &str) -> Option<&'static str> {
    match model {
        "fast" | "grok-chat-fast" => Some("fast"),
        "auto" | "grok-chat-auto" => Some("auto"),
        "expert" | "grok-chat-expert" => Some("expert"),
        "heavy" | "grok-chat-heavy" => Some("heavy"),
        _ => None,
    }
}

pub(super) fn prompt<'a>(
    request: &'a CanonicalRelayRequest,
    model: &str,
) -> Result<&'a str, GatewayError> {
    let invalid = || failure("grok_web_unsupported_request", ErrorKind::BadRequest);
    if mode(model).is_none()
        || !matches!(
            request.endpoint_kind,
            EndpointKind::ChatCompletions | EndpointKind::Responses
        )
        || request.messages.len() != 1
        || !request.tools.is_empty()
        || request.tool_choice.is_some()
        || request.reasoning.is_some()
        || request.previous_response_id.is_some()
        || request.explicit_session_key.is_some()
        || !request.extra.is_empty()
        || request.raw_body.as_object().is_some_and(|body| {
            body.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "model" | "messages" | "input" | "stream" | "metadata"
                )
            })
        })
    {
        return Err(invalid());
    }
    let message = &request.messages[0];
    if message.role != MessageRole::User
        || message.content.len() != 1
        || message.name.is_some()
        || message.tool_call_id.is_some()
        || !message.tool_calls.is_empty()
    {
        return Err(invalid());
    }
    let ContentPart::Text { text } = &message.content[0] else {
        return Err(invalid());
    };
    if text.trim().is_empty() || text.len() > 64 * 1024 {
        return Err(invalid());
    }
    Ok(text)
}

pub(super) fn initial(mode: &str, event_id: &str) -> Value {
    json!({"event":{"type":"session.create","event_id":event_id,"session":{
        "model":mode,"x_grok":{
            "protocol_capabilities":["conversation_attached","custom_methods_v1"],
            "use_chunk":true,"enable_side_by_side":false,"force_side_by_side":false,
            "enable_image_generation":false,
            "disable_text_follow_ups":false,"disable_artifact":true,"force_concise":false,
            "keep_context":false,"is_temporary":true,"disable_memory":true
        }
    }}})
}

pub(super) fn turn(session: &str, prompt: &str) -> [Value; 2] {
    let id = uuid::Uuid::new_v4().to_string();
    [
        json!({"session_id":session,"event":{
            "type":"conversation.item.create","event_id":format!("evt_msg_{id}"),
            "item":{"type":"message","role":"user","x_grok":{
                "client_message_id":id,"input_chunks":[{"text":{"text":prompt}}]
            }}
        }}),
        json!({"session_id":session,"event":{"type":"response.create","event_id":format!("evt_resp_{id}")}}),
    ]
}
