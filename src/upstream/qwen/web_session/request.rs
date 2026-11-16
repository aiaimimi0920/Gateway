//! Only known, lossless single-user text is admitted; remote history cannot be invented locally.
use super::failure;
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::canonical::{CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole},
};
use serde_json::{json, Value};
pub(super) fn validate<'a>(
    request: &'a CanonicalRelayRequest,
    model: &str,
) -> Result<&'a str, GatewayError> {
    let invalid = || {
        failure(
            "qwen_web_current_unsupported_request",
            ErrorKind::BadRequest,
        )
    };
    if model.trim().is_empty()
        || model.len() > 256
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
        || request.raw_body.as_object().is_some_and(|o| {
            o.keys().any(|k| {
                !matches!(
                    k.as_str(),
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
pub(super) fn create(model: &str) -> Value {
    json!({"chatId":"","models":[model],"timestamp":time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000,"chat_type":"t2t","chat_mode":"normal"})
}
pub(super) fn completion(model: &str, chat: &str, text: &str) -> Value {
    let message = uuid::Uuid::new_v4().to_string();
    json!({"stream":true,"version":"2.1","incremental_output":true,"chatId":chat,"parentId":"","chat_id":chat,"chat_mode":"normal","model":model,"parent_id":null,
        "messages":[{"fid":message,"parentId":null,"childrenIds":[],"role":"user","content":text,"user_action":"chat","files":[],"timestamp":time::OffsetDateTime::now_utc().unix_timestamp(),"models":[model],"chat_type":"t2t",
            "feature_config":{"thinking_enabled":false,"output_schema":"phase","research_mode":"normal","web_search_enabled":false,"auto_search":false,"function_calling":false},
            "extra":{"meta":{"subChatType":"t2t"}},"sub_chat_type":"t2t","parent_id":null}],"timestamp":time::OffsetDateTime::now_utc().unix_timestamp()})
}
