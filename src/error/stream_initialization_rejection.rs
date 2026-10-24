//! Recognize a relay's explicit pre-generation model refusal inside an HTTP 502.
use super::{sanitize_provider_error_message, ErrorKind, FallbackHint, GatewayError};
use serde_json::Value;

pub(super) fn classify(
    status: u16,
    body: &str,
    provider_name: Option<&str>,
) -> Option<GatewayError> {
    // Do not infer safe replay from arbitrary 5xx text, or parse unbounded nested errors.
    if status != 502 || body.len() > 64 * 1024 {
        return None;
    }
    let value: Value = serde_json::from_str(body).ok()?;
    let outer = value.get("error")?;
    let message = outer.get("message")?.as_str()?;
    let detail = if let Some(detail) = message.strip_prefix("stream_initialization_failed: ") {
        detail
    } else if outer.get("code").and_then(Value::as_str) == Some("stream_initialization_failed")
        || outer.get("type").and_then(Value::as_str) == Some("stream_initialization_failed")
    {
        message
    } else {
        return None;
    };
    // This chain explicitly failed to create a stream; never match a post-output error.
    let detail = detail.strip_prefix(
        "Failed to create stream: inference request failed: failed to generate stream from Vercel: failed to invoke model '",
    )?;
    let (model, nested) =
        detail.split_once("' with streaming: request failed with status 404: ")?;
    if model.is_empty() || model.len() > 1024 {
        return None;
    }
    let nested: Value = serde_json::from_str(nested).ok()?;
    let rejection = nested.get("error")?;
    if rejection.get("type").and_then(Value::as_str) != Some("model_not_found")
        || rejection.pointer("/param/modelId").and_then(Value::as_str) != Some(model)
        || rejection.get("message").and_then(Value::as_str)
            != Some(format!("Model '{model}' not found").as_str())
    {
        return None;
    }
    Some(GatewayError {
        kind: ErrorKind::ModelNotFound,
        message: sanitize_provider_error_message(message),
        code: Some("upstream_stream_initialization_model_not_found".into()),
        // Preserve the actual relay status; the nested 404 is not the HTTP response.
        http_status: Some(status),
        retryable: false,
        fallback_hint: FallbackHint::FallbackProvider {
            reason: "Upstream rejected the model before stream creation; try another candidate."
                .into(),
        },
        provider_name: provider_name.map(str::to_owned),
        upstream_retry_after_ms: None,
    })
}
