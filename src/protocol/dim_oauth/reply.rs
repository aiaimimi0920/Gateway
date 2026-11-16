//! DIM envelope policy on the shared duplicate-rejecting JSON reader.
use super::OAuthError;
use serde_json::Value;

pub(super) fn object(bytes: &[u8], max: usize) -> Result<Value, OAuthError> {
    if bytes.len() > max {
        return Err(OAuthError::ResponseTooLarge);
    }
    let value =
        crate::protocol::unique_json::parse(bytes).map_err(|_| OAuthError::InvalidResponse)?;
    if !value.is_object() || value.get("success") == Some(&Value::Bool(false)) {
        return Err(OAuthError::InvalidResponse);
    }
    Ok(value)
}
