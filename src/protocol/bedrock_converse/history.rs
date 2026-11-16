//! Preserve the native conversation prefix covered by opaque reasoning signatures.
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, ProtocolFamily},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::Value;

fn blocks(body: &Value) -> impl Iterator<Item = &Value> {
    body.get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|message| message.get("content").and_then(Value::as_array))
        .flatten()
}

fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Bedrock reasoning content.")
        .with_code("bedrock_reasoning_content")
}

pub(super) fn validate_body(body: &Value) -> Result<(), GatewayError> {
    for block in blocks(body) {
        let Some(reasoning) = block.get("reasoningContent") else {
            continue;
        };
        // ContentBlock and ReasoningContentBlock are both unions. Otherwise a
        // competing text/tool member could hide reasoning during normalization.
        if block.as_object().is_none_or(|object| object.len() != 1) {
            return Err(invalid());
        }
        let union = reasoning.as_object().ok_or_else(invalid)?;
        if union.len() != 1 {
            return Err(invalid());
        }
        if let Some(text) = union.get("reasoningText") {
            let text = text.as_object().ok_or_else(invalid)?;
            if !text.get("text").is_some_and(Value::is_string)
                || text
                    .get("signature")
                    .is_some_and(|value| !value.is_string())
            {
                return Err(invalid());
            }
        } else if let Some(redacted) = union.get("redactedContent") {
            let redacted = redacted.as_str().ok_or_else(invalid)?;
            // Validate only: do not trim, decode/re-encode or invent opaque bytes.
            STANDARD.decode(redacted).map_err(|_| invalid())?;
        } else {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn original_if_unchanged(
    req: &CanonicalRelayRequest,
) -> Result<Option<&Value>, GatewayError> {
    if req.protocol_family != ProtocolFamily::BedrockConverse
        || !blocks(&req.raw_body).any(|block| block.get("reasoningContent").is_some())
    {
        return Ok(None);
    }
    let changed = || {
        GatewayError::bad_request("Bedrock reasoning history was changed before sending.")
            .with_code("bedrock_reasoning_history_changed")
    };
    // Defaults/extra must not replace a prefix, even when they look equivalent.
    if req.extra.contains_key("messages") || req.extra.contains_key("system") {
        return Err(changed());
    }
    let original = super::normalization::normalize_converse(
        req.raw_body.clone(),
        req.requested_model.clone(),
        req.stream,
    )?;
    let projection = |messages| {
        serde_json::to_value(messages)
            .map_err(|_| GatewayError::server_error("Unable to compare Bedrock history."))
    };
    // Includes the system projection, all roles, raw reasoning and tool payloads.
    // A raw replay is allowed only after policy's current projection still agrees.
    if projection(&original.messages)? != projection(&req.messages)? {
        return Err(changed());
    }
    Ok(Some(&req.raw_body))
}
