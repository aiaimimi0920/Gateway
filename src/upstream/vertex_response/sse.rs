//! Single-candidate SSE framing and finish/usage verification, before any product projection.
use super::{Contract, GatewayError, TokenUsage, Value, VerifiedResponse};
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};
use serde_json::json;

pub(super) fn parse(contract: Contract, body: &[u8]) -> Result<VerifiedResponse, GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    let text = std::str::from_utf8(body).map_err(|_| invalid("Invalid UTF-8."))?;
    if !text.ends_with('\n') {
        return Err(invalid("Incomplete SSE line."));
    }
    let mut parser = SseParseState::new();
    let mut merged = json!({"candidates":[{"index":0,"content":{"role":"model","parts":[]}}]});
    let mut seen = false;
    let mut ended = false;
    let mut usage = None;
    for line in text.split_terminator('\n') {
        let Some(frame) = parse_sse_line(line, &mut parser) else {
            continue;
        };
        if frame.event_name.as_deref() == Some("error") {
            return Err(invalid("Upstream error event."));
        }
        if frame.data.is_empty() {
            continue;
        }
        let mut raw: Value =
            serde_json::from_str(&frame.data).map_err(|_| invalid("Invalid SSE JSON."))?;
        if !raw.is_object() || raw.get("error").is_some_and(|v| !v.is_null()) {
            return Err(invalid("Upstream error or invalid response object."));
        }
        if raw
            .get("promptFeedback")
            .and_then(|v| v.get("blockReason"))
            .and_then(Value::as_str)
            .is_some_and(|v| !matches!(v, "" | "BLOCKED_REASON_UNSPECIFIED"))
        {
            return Err(contract.prompt_blocked());
        }
        if let Some(value) = raw.get("modelVersion") {
            if !value.is_string() {
                return Err(invalid("Invalid model version."));
            }
            merged["modelVersion"] = value.clone();
        }
        if let Some(value) = raw.get("usageMetadata") {
            if let Some(reported) = parse_usage(contract, value)? {
                usage = Some(reported);
            }
        }
        let Some(candidates) = raw.get_mut("candidates") else {
            continue;
        };
        let candidates = candidates
            .as_array_mut()
            .ok_or_else(|| invalid("Invalid candidates."))?;
        // The canonical envelope has one turn; multiple choices must never be silently concatenated.
        if candidates.len() > 1 {
            return Err(invalid("Multiple candidates are not yet supported."));
        }
        let Some(candidate) = candidates.first_mut() else {
            continue;
        };
        if !candidate.is_object()
            || candidate
                .get("index")
                .is_some_and(|v| v.as_u64() != Some(0))
        {
            return Err(invalid("Unsupported candidate index."));
        }
        seen = true;
        let reason = candidate
            .get("finishReason")
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("Invalid finish reason."))
            })
            .transpose()?;
        let finish = reason
            .as_deref()
            .filter(|v| !matches!(*v, "" | "FINISH_REASON_UNSPECIFIED"));
        if finish.is_some_and(|v| {
            !matches!(
                v,
                "STOP"
                    | "MAX_TOKENS"
                    | "SAFETY"
                    | "RECITATION"
                    | "OTHER"
                    | "BLOCKLIST"
                    | "PROHIBITED_CONTENT"
                    | "SPII"
                    | "MALFORMED_FUNCTION_CALL"
                    | "MODEL_ARMOR"
                    | "IMAGE_SAFETY"
                    | "IMAGE_PROHIBITED_CONTENT"
                    | "IMAGE_RECITATION"
                    | "IMAGE_OTHER"
                    | "UNEXPECTED_TOOL_CALL"
                    | "NO_IMAGE"
            )
        }) {
            return Err(invalid("Unknown finish reason."));
        }
        let parts = match candidate.get_mut("content") {
            None => Vec::new(),
            Some(content) => {
                if !content.is_object() {
                    return Err(invalid("Invalid candidate content."));
                }
                match content.get_mut("parts") {
                    None => Vec::new(),
                    Some(parts) => match parts.take() {
                        Value::Array(parts) => parts,
                        _ => return Err(invalid("Invalid candidate parts.")),
                    },
                }
            }
        };
        if ended && (!parts.is_empty() || finish.is_some()) {
            return Err(invalid("Candidate data after finish."));
        }
        let target = merged["candidates"][0]["content"]["parts"]
            .as_array_mut()
            .unwrap();
        for part in parts {
            if !part.is_object()
                || part.get("thought").is_some_and(|v| !v.is_boolean())
                || part.get("text").is_some_and(|v| !v.is_string())
            {
                return Err(invalid("Invalid content part."));
            }
            target.push(part);
        }
        if let Some(reason) = finish {
            merged["candidates"][0]["finishReason"] = json!(reason);
            ended = true;
        }
    }
    if !parser.data_lines.is_empty() || parser.event_name.is_some() || !seen || !ended {
        return Err(invalid("Missing complete candidate finish."));
    }
    Ok(VerifiedResponse {
        body: merged,
        usage,
    })
}
fn parse_usage(contract: Contract, value: &Value) -> Result<Option<TokenUsage>, GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    if !value.is_object() {
        return Err(invalid("Invalid usage metadata."));
    }
    let counter = |name| match value.get(name) {
        None => Ok(0),
        Some(value) => value
            .as_u64()
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or_else(|| invalid("Invalid token count.")),
    };
    let usage = TokenUsage {
        prompt_tokens: counter("promptTokenCount")?,
        completion_tokens: counter("candidatesTokenCount")?,
        total_tokens: counter("totalTokenCount")?,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: value
            .get("cachedContentTokenCount")
            .map(|_| counter("cachedContentTokenCount"))
            .transpose()?,
    };
    // Absence is unknown, not a manufactured zero bill. A reported total includes more than text tokens.
    Ok(value.get("totalTokenCount").map(|_| usage))
}
