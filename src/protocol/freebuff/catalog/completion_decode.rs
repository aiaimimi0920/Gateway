//! Native JSON and incremental SSE validation, separate from canonical projection and billing.
mod accounting;
mod json;
mod sse;
mod tools;
use super::BusinessError;
pub(super) use accounting::{Accounting, MAX_SAFE_INTEGER};
pub(super) use json::decode_json;
use serde_json::Value;
pub(super) use sse::StreamDecoder;
pub(crate) use tools::resolve_index;
pub(super) const BODY_LIMIT: usize =
    crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES;
fn invalid() -> BusinessError {
    BusinessError::InvalidResponse
}
fn optional_string(value: Option<&Value>) -> Result<Option<&str>, BusinessError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        _ => Err(invalid()),
    }
}
fn terminal(value: &Value) -> Result<&str, BusinessError> {
    let value = value.as_str().ok_or_else(invalid)?;
    match value {
        "stop" | "length" | "content_filter" | "tool_calls" | "function_call" => Ok(value),
        _ => Err(invalid()),
    }
}
