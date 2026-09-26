use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn pack_create_chat(model: &str) -> Value {
    let timestamp = current_timestamp_secs();
    json!({
        "title": format!("api_{timestamp}"),
        "models": [model],
        "chat_mode": "normal",
        "chat_type": "t2t",
        "timestamp": timestamp,
    })
}

pub fn pack_qwen_web(
    req: &CanonicalRelayRequest,
    model: &str,
    chat_id: &str,
) -> Result<Value, GatewayError> {
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
    ) {
        return Err(GatewayError::bad_request(
            "Qwen Web adapters currently support only text chat endpoints.",
        )
        .with_code("unsupported_qwen_web_endpoint"));
    }
    let combined_text = combine_messages_for_qwen_web(&req.messages)?;
    if combined_text.trim().is_empty() {
        return Err(GatewayError::bad_request(
            "Qwen Web requests require at least one text message part.",
        )
        .with_code("qwen_web_missing_text"));
    }
    let timestamp = current_timestamp_secs();
    let message_id = uuid::Uuid::new_v4().to_string();
    let child_message_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let session_id = uuid::Uuid::new_v4().to_string();
    let feature_config = json!({
        "thinking_enabled": false,
        "output_schema": "phase",
        "research_mode": "normal",
        "web_search_enabled": false,
        "function_calling": !req.tools.is_empty(),
    });

    Ok(json!({
        "stream": true,
        "version": "2.1",
        "incremental_output": true,
        "chat_id": chat_id,
        "chat_mode": "normal",
        "model": model,
        "parent_id": Value::Null,
        "chat_type": "t2t",
        "messages": [{
            "fid": message_id,
            "parentId": Value::Null,
            "childrenIds": [child_message_id],
            "role": "user",
            "content": combined_text,
            "user_action": "chat",
            "files": [],
            "timestamp": timestamp,
            "models": [model],
            "chat_type": "t2t",
            "feature_config": feature_config,
            "extra": {
                "meta": {
                    "subChatType": "t2t"
                }
            },
            "sub_chat_type": "t2t",
            "parent_id": Value::Null
        }],
        "session_id": session_id,
        "id": request_id,
        "sub_chat_type": "t2t",
    }))
}

fn combine_messages_for_qwen_web(messages: &[CanonicalMessage]) -> Result<String, GatewayError> {
    let mut rendered = Vec::new();
    for message in messages {
        let mut parts = Vec::new();
        for part in &message.content {
            match part {
                ContentPart::Text { text } => {
                    if !text.trim().is_empty() {
                        parts.push(text.trim().to_string());
                    }
                }
                ContentPart::ImageUrl { .. } => {
                    return Err(GatewayError::bad_request(
                        "Qwen Web adapters do not currently support image content.",
                    )
                    .with_code("unsupported_qwen_web_image"));
                }
                ContentPart::Json { .. } | ContentPart::Raw { .. } => {
                    return Err(GatewayError::bad_request(
                        "Qwen Web adapters do not currently support structured non-text content.",
                    )
                    .with_code("unsupported_qwen_web_non_text"));
                }
            }
        }
        let text = parts.join("\n");
        if text.is_empty() {
            continue;
        }
        if messages.len() == 1 && matches!(message.role, MessageRole::User) {
            rendered.push(text);
        } else {
            rendered.push(format!("{}:{}", qwen_role_label(message.role), text));
        }
    }
    Ok(rendered.join(";"))
}

fn qwen_role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(u64::MAX)
}
