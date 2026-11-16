//! Copilot's buffered single-turn admission contract; this does not enable routing.
use crate::error::{FallbackHint, GatewayError};
use crate::protocol::canonical::{
    CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use serde_json::Value;

/// Borrows the filtered canonical text and the route-selected model, not credentials.
#[derive(Debug, PartialEq, Eq)]
pub struct TextRequest<'a> {
    pub model: &'a str,
    pub prompt: &'a str,
}

/// Validate before starting a runtime. Raw checks catch options discarded by normalization.
/// Caller metadata is not an identity channel; streaming awaits its own execution contract.
pub fn single_turn<'a>(
    request: &'a CanonicalRelayRequest,
    selected_model: &'a str,
) -> Result<TextRequest<'a>, GatewayError> {
    if request.protocol_family != ProtocolFamily::OpenAi
        || request.stream
        || request.messages.len() != 1
        || !request.tools.is_empty()
        || request.tool_choice.is_some()
        || request.reasoning.is_some()
        || request.metadata.is_some()
        || request.previous_response_id.is_some()
        || request.explicit_session_key.is_some()
        || !request.extra.is_empty()
        || selected_model.trim().is_empty()
        || selected_model.len() > 512
        || !valid_wire(&request.raw_body, request.endpoint_kind)
    {
        return Err(unsupported());
    }
    let message = &request.messages[0];
    if message.role != MessageRole::User
        || message.name.is_some()
        || message.tool_call_id.is_some()
        || !message.tool_calls.is_empty()
        || message.content.len() != 1
    {
        return Err(unsupported());
    }
    let ContentPart::Text { text } = &message.content[0] else {
        return Err(unsupported());
    };
    if text.trim().is_empty() || text.len() > 64 * 1024 {
        return Err(unsupported());
    }
    Ok(TextRequest {
        model: selected_model,
        prompt: text,
    })
}

fn valid_wire(body: &Value, endpoint: EndpointKind) -> bool {
    let field = match endpoint {
        EndpointKind::ChatCompletions => "messages",
        EndpointKind::Responses => "input",
        _ => return false,
    };
    if !only_keys(body, &["model", field, "stream"])
        || body.get("model").is_some_and(|v| {
            !v.as_str()
                .is_some_and(|s| !s.trim().is_empty() && s.len() <= 512)
        })
        || body.get("stream").is_some_and(|v| v != &Value::Bool(false))
    {
        return false;
    }
    let Some(input) = body.get(field) else {
        return false;
    };
    if endpoint == EndpointKind::Responses && input.is_string() {
        return true;
    }
    let Some(items) = input.as_array().filter(|items| items.len() == 1) else {
        return false;
    };
    let message = &items[0];
    let responses = endpoint == EndpointKind::Responses;
    let keys: &[&str] = if responses {
        &["role", "content", "type"]
    } else {
        &["role", "content"]
    };
    if !only_keys(message, keys)
        || message.get("role").and_then(Value::as_str) != Some("user")
        || (responses
            && message
                .get("type")
                .is_some_and(|v| v.as_str() != Some("message")))
    {
        return false;
    }
    let Some(content) = message.get("content") else {
        return false;
    };
    if content.is_string() {
        return true;
    }
    let Some(parts) = content.as_array().filter(|parts| parts.len() == 1) else {
        return false;
    };
    let part = &parts[0];
    only_keys(part, &["type", "text"])
        && part.get("type").and_then(Value::as_str)
            == Some(if responses { "input_text" } else { "text" })
        && part.get("text").is_some_and(Value::is_string)
}

fn only_keys(value: &Value, allowed: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.keys().all(|key| allowed.contains(&key.as_str())))
}

fn unsupported() -> GatewayError {
    let mut error =
        GatewayError::bad_request("Copilot 当前仅支持非流式单轮 user 纯文本；不支持此请求组合。")
            .with_code("copilot_unsupported_request");
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Unsupported Copilot requests must not be resubmitted.".into(),
    };
    error
}
