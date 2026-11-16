//! Reject unsupported/lossy Key input before the legacy packer can repair or discard it.
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, ContentPart, MessageRole};
use std::collections::HashSet;

pub(super) fn validate(req: &CanonicalRelayRequest) -> Result<(), GatewayError> {
    if req.previous_response_id.is_some()
        || req
            .messages
            .iter()
            .rev()
            .find(|m| m.role != MessageRole::System)
            .is_none_or(|m| m.role == MessageRole::Assistant)
    {
        return Err(invalid());
    }
    let mut pending = HashSet::new();
    let mut seen = HashSet::new();
    let declared: HashSet<&str> = req
        .tools
        .iter()
        .filter_map(|tool| tool.name.as_deref())
        .collect();
    for message in &req.messages {
        for part in &message.content {
            match part {
                ContentPart::Text { .. } => {}
                ContentPart::ImageUrl { image_url, detail }
                    if message.role == MessageRole::User =>
                {
                    // Only default image admission is evidenced; no CLI wire for resolution control.
                    if detail.as_deref().is_some_and(|detail| detail != "auto") {
                        return Err(invalid());
                    }
                    // Match the actual packer's scheme admission; never validate a form it drops.
                    if !image_url.starts_with("data:") {
                        return Err(invalid());
                    }
                    let Some((header, _)) = image_url.split_once(',') else {
                        return Err(invalid());
                    };
                    let header = header.to_ascii_lowercase();
                    if !header.starts_with("data:image/") || !header.ends_with(";base64") {
                        return Err(invalid());
                    }
                }
                _ => return Err(invalid()),
            }
        }
        if message.role != MessageRole::Assistant && !message.tool_calls.is_empty() {
            return Err(invalid());
        }
        match message.role {
            MessageRole::User if !pending.is_empty() => return Err(invalid()),
            MessageRole::Tool => {
                let id = message
                    .tool_call_id
                    .as_deref()
                    .filter(|id| !id.is_empty())
                    .ok_or_else(invalid)?;
                if !pending.remove(id) {
                    return Err(invalid());
                }
            }
            MessageRole::Assistant => {
                for call in &message.tool_calls {
                    let id = call
                        .id
                        .as_deref()
                        .filter(|id| !id.is_empty())
                        .ok_or_else(invalid)?;
                    let name = call
                        .name
                        .as_deref()
                        .filter(|name| !name.is_empty())
                        .ok_or_else(invalid)?;
                    if !matches!(call.call_type.as_str(), "function" | "function_call")
                        || !declared.contains(name)
                        || !seen.insert(id)
                    {
                        return Err(invalid());
                    }
                    let arguments: serde_json::Value =
                        serde_json::from_str(call.arguments.as_deref().ok_or_else(invalid)?)
                            .map_err(|_| invalid())?;
                    if !arguments.is_object() {
                        return Err(invalid());
                    }
                    pending.insert(id);
                }
            }
            _ => {}
        }
    }
    if !pending.is_empty() {
        return Err(invalid());
    }
    Ok(())
}

fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Kiro CLI Key input contains unsupported content or incomplete tool history.",
    )
    .with_code("kiro_api_key_unsupported_input")
}
