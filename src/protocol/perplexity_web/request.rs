//! 固定 pinned auto/concise profile；不拼接多轮、不注入 caller extra 或伪装工具支持。
use super::input_failure;
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole},
};
use serde_json::{json, Value};

pub fn pack_single_turn(request: &CanonicalRelayRequest) -> Result<Value, GatewayError> {
    pack_single_turn_for_model(
        request,
        request.requested_model.as_deref().unwrap_or("turbo"),
    )
}

/// The route resolves public aliases; validate the selected policy without rewriting caller input.
pub(crate) fn pack_single_turn_for_model(
    request: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let unsupported = || {
        input_failure(
            "perplexity_web_unsupported_request",
            "Perplexity Web 当前仅支持 auto/concise 单轮纯文本；不支持此请求组合。",
        )
    };
    if !matches!(
        request.endpoint_kind,
        EndpointKind::ChatCompletions | EndpointKind::Responses
    ) || request.messages.len() != 1
        || !request.tools.is_empty()
        || request.tool_choice.is_some()
        || request.reasoning.is_some()
        || request.previous_response_id.is_some()
        || request.explicit_session_key.is_some()
        || !request.extra.is_empty()
        || model != "turbo"
        || request.raw_body.as_object().is_some_and(|body| {
            body.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "model" | "messages" | "input" | "stream" | "metadata"
                )
            })
        })
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
    Ok(json!({
        "query_str":text,
        "params":{
            "attachments":[],
            "frontend_context_uuid":uuid::Uuid::new_v4().to_string(),
            "frontend_uuid":uuid::Uuid::new_v4().to_string(),
            "is_incognito":false,
            "language":"en-US",
            "last_backend_uuid":null,
            "mode":"concise",
            "model_preference":"turbo",
            "source":"default",
            "sources":["web"],
            "version":"2.18"
        }
    }))
}
