//! Validate original wire fields before normalization can discard anything or a quote can be made.
use super::*;
pub(in super::super) fn request(req: &CanonicalRelayRequest) -> Result<Value, BusinessError> {
    if !within_limit(&req.raw_body, 50 * 1024 * 1024)
        || req
            .raw_body
            .get("stream")
            .is_some_and(|v| v.as_bool() != Some(req.stream))
    {
        return Err(BusinessError::InvalidInput);
    }
    let body = match req.endpoint_kind {
        EndpointKind::ChatCompletions => {
            RelayStep::validate(&req.raw_body)?;
            req.raw_body.clone()
        }
        EndpointKind::Responses => responses::convert(&req.raw_body)?,
        EndpointKind::Messages => messages::convert(&req.raw_body)?,
        _ => return Err(BusinessError::InvalidInput),
    };
    RelayStep::validate(&body)?;
    if !within_limit(&body, 50 * 1024 * 1024) {
        return Err(BusinessError::RequestTooLarge);
    }
    // Raw restoration must not undo policy edits made after the normal ingress adapter ran.
    // This lane rejects such edits rather than quietly bypassing them or inventing a second policy.
    let original = match req.endpoint_kind {
        EndpointKind::ChatCompletions => {
            crate::protocol::openai::normalize_chat_completions(req.raw_body.clone())
        }
        EndpointKind::Responses => {
            crate::protocol::responses::normalize_responses(req.raw_body.clone())
        }
        EndpointKind::Messages => {
            crate::protocol::anthropic::normalize_messages(req.raw_body.clone())
        }
        _ => unreachable!(),
    }
    .map_err(|_| BusinessError::InvalidInput)?;
    let current = (
        &req.messages,
        &req.tools,
        &req.tool_choice,
        &req.reasoning,
        &req.extra,
        &req.metadata,
    );
    let before = (
        &original.messages,
        &original.tools,
        &original.tool_choice,
        &original.reasoning,
        &original.extra,
        &original.metadata,
    );
    if serde_json::to_value(current).map_err(|_| BusinessError::InvalidInput)?
        != serde_json::to_value(before).map_err(|_| BusinessError::InvalidInput)?
    {
        return Err(BusinessError::InvalidInput);
    }
    Ok(body)
}
pub(super) fn text_parts(value: &Value, kind: &str) -> Result<Value, BusinessError> {
    if value.is_string() {
        return Ok(value.clone());
    }
    let parts = value
        .as_array()
        .filter(|p| p.len() <= 4096)
        .ok_or(BusinessError::InvalidInput)?;
    let mut result = Vec::with_capacity(parts.len());
    for part in parts {
        fields(part, &["type", "text"])?;
        if part["type"] != kind || !part["text"].is_string() {
            return Err(BusinessError::InvalidInput);
        }
        result.push(json!({"type":"text","text":part["text"]}));
    }
    Ok(Value::Array(result))
}
#[cfg(test)]
mod tests;
