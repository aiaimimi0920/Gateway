//! Canvas RouteLlmRequest wire format. Never substitute Web StreamGenerate or an API key.
use super::*;

#[path = "http_wire_limits.rs"]
mod limits;
pub(super) use limits::form;

pub(super) const RPC: &str = "q4uTj";
pub(super) const MAX_REQUEST_BYTES: usize = 24 * 1024 * 1024;

pub(super) fn operation(model: &str) -> Result<u8, GatewayError> {
    match model {
        "gemini-3-flash-preview"
        | "gemini-2.0-flash"
        | "gemini-2.5-flash"
        | "gemini-2.5-flash-preview-04-17"
        | "gemini-2.5-flash-preview-05-20"
        | "gemini-2.5-flash-preview-09-2025" => Ok(1),
        "imagen-4.0-generate-001" => Ok(2),
        "gemini-2.5-flash-image"
        | "gemini-2.5-flash-image-preview"
        | "gemini-3.1-flash-image"
        | "gemini-3.1-flash-image-preview" => Ok(4),
        "gemini-2.5-flash-preview-tts" => Ok(5),
        "gemini-3-pro-image"
        | "gemini-3-pro-image-preview"
        | "gemini-3-pro-image-preview-11-2025" => Ok(6),
        _ => Err(invalid(
            "Canvas HTTP host does not support this model.",
            "unsupported_gemini_canvas_http_model",
        )),
    }
}

pub(super) fn form_body(model: &str, body: &Value) -> Result<String, GatewayError> {
    let operation = operation(model)?;
    if !body.is_object()
        || body
            .get("model")
            .is_some_and(|value| value.as_str() != Some(model))
    {
        return Err(invalid(
            "Canvas HTTP body/model contract is invalid.",
            "invalid_gemini_canvas_http_request",
        ));
    }
    #[derive(serde::Serialize)]
    struct WithModel<'a> {
        #[serde(flatten)]
        body: &'a Value,
        model: &'a str,
    }
    // Borrow inline media instead of cloning every base64 input to inject the model.
    let encoded = if body.get("model").is_some() {
        limits::json(body, MAX_REQUEST_BYTES)
    } else {
        limits::json(&WithModel { body, model }, MAX_REQUEST_BYTES)
    }?;
    // Tuples borrow the large inner strings; each escaping layer has its own admission.
    let request = limits::json(&(None::<()>, &encoded, operation), 32 * 1024 * 1024)?;
    limits::json(
        &[[(RPC, &request, None::<()>, "generic")]],
        32 * 1024 * 1024,
    )
}

pub(super) fn rpc_error(code: i64) -> GatewayError {
    match code {
        16 => GatewayError::unauthorized("Canvas HTTP authentication expired; sign in again.")
            .with_provider(PROVIDER)
            .with_code("gemini_canvas_http_session_invalid"),
        8 => GatewayError::rate_limited("Canvas HTTP quota is exhausted.", 30_000)
            .with_provider(PROVIDER)
            .with_code("gemini_canvas_http_quota_exhausted"),
        7 => failure("gemini_canvas_http_permission_denied"),
        _ => failure("gemini_canvas_http_rpc_failed"),
    }
}

pub(super) fn parse_response(text: &str) -> Result<Value, GatewayError> {
    let malformed = || failure("gemini_canvas_http_response_invalid");
    let mut result = None;
    let mut frame_count = 0usize;
    let mut response_count = 0usize;
    for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
        if line.is_empty() || line == ")]}'" || line.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let batch: Value = serde_json::from_str(line).map_err(|_| malformed())?;
        for frame in batch.as_array().ok_or_else(malformed)? {
            frame_count += 1;
            if frame_count > 128 {
                return Err(malformed());
            }
            let frame = frame.as_array().ok_or_else(malformed)?;
            if frame.get(1).and_then(Value::as_str) != Some(RPC) {
                continue;
            }
            match frame.first().and_then(Value::as_str) {
                Some("er") => {
                    return Err(rpc_error(
                        frame.get(2).and_then(Value::as_i64).unwrap_or(13),
                    ))
                }
                Some("wrb.fr") => {}
                _ => return Err(malformed()),
            }
            if let Some(codes) = frame.get(5).and_then(Value::as_array) {
                for code in codes
                    .iter()
                    .filter_map(Value::as_i64)
                    .filter(|code| *code != 0)
                {
                    return Err(rpc_error(code));
                }
            }
            response_count += 1;
            if response_count > 1 {
                return Err(malformed());
            }
            let raw = frame.get(2).and_then(Value::as_str).ok_or_else(malformed)?;
            let envelope: Value = serde_json::from_str(raw).map_err(|_| malformed())?;
            let raw = envelope
                .get(0)
                .and_then(Value::as_str)
                .ok_or_else(malformed)?;
            let generated: Value = serde_json::from_str(raw).map_err(|_| malformed())?;
            if !generated.is_object() || generated.get("error").is_some() {
                return Err(failure("gemini_canvas_http_generation_failed"));
            }
            result = Some(generated);
        }
    }
    result.ok_or_else(malformed)
}

#[cfg(test)]
#[path = "http_wire_tests.rs"]
mod tests;
