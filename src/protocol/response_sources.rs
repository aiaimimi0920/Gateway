//! Bounded answer-level source lists, not verified inline citation spans.
use crate::error::{FallbackHint, GatewayError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub(crate) const WIRE_FIELD: &str = "gateway_sources";
pub(crate) const INVALID: &str = "Invalid Gateway source-list extension.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseSource {
    pub name: String,
    pub url: String,
    pub snippet: String,
}

/// Read only the declared Gateway extension; never guess provider citation schemas.
/// Check budgets before cloning strings; URLs are data, never fetched here.
pub(crate) fn read(body: &Value) -> Result<Option<Vec<ResponseSource>>, GatewayError> {
    let Some(value) = body.get(WIRE_FIELD) else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .filter(|v| v.len() <= 64)
        .ok_or_else(invalid)?;
    let sources = values
        .iter()
        .map(|source| {
            let name = bounded_string(&source["name"], 1024)?;
            let url = bounded_string(&source["url"], 2048)?;
            let snippet = bounded_string(&source["snippet"], 8192)?;
            let parsed = url::Url::parse(url).map_err(|_| invalid())?;
            if !matches!(parsed.scheme(), "http" | "https")
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err(invalid());
            }
            Ok(ResponseSource {
                name: name.into(),
                url: url.into(),
                snippet: snippet.into(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(sources))
}

fn bounded_string(value: &Value, limit: usize) -> Result<&str, GatewayError> {
    value
        .as_str()
        .filter(|s| s.len() <= limit)
        .ok_or_else(invalid)
}

fn invalid() -> GatewayError {
    let mut error = GatewayError::server_error(INVALID).with_code("invalid_response_sources");
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: INVALID.into(),
    };
    error
}

/// Owners pass a freshly built JSON object. Empty lists leave legacy wire unchanged.
pub(crate) fn project(body: &mut Value, sources: &[ResponseSource]) {
    if !sources.is_empty() {
        body[WIRE_FIELD] = json!(sources);
    }
}
