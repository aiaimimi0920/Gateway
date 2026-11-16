//! Bounded issuer key selection. An ambiguous kid fails closed even if one candidate would verify.
use super::super::{material::parse_reply, OAuthError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use serde_json::{value::RawValue, Value};

#[derive(Deserialize)]
struct KeySet {
    keys: Vec<Box<RawValue>>,
}
pub(super) fn public_key(bytes: &[u8], kid: &str) -> Result<[u8; 65], OAuthError> {
    if bytes.len() > 65_536 {
        return Err(OAuthError::InvalidIdentity);
    }
    // Keep each JWK raw until duplicate-field validation; Value alone loses that evidence.
    let set: KeySet = serde_json::from_slice(bytes).map_err(|_| OAuthError::InvalidIdentity)?;
    if set.keys.is_empty() || set.keys.len() > 32 {
        return Err(OAuthError::InvalidIdentity);
    }
    let mut selected = None;
    for raw in set.keys {
        let key = parse_reply(raw.get().as_bytes()).map_err(|_| OAuthError::InvalidIdentity)?;
        if key.get("kid").and_then(Value::as_str) == Some(kid) {
            if selected.replace(key).is_some() {
                return Err(OAuthError::InvalidIdentity);
            }
        }
    }
    let key = selected.ok_or(OAuthError::InvalidIdentity)?;
    if key.get("kty").and_then(Value::as_str) != Some("EC")
        || key.get("crv").and_then(Value::as_str) != Some("P-256")
        || key.get("alg").is_some_and(|v| v.as_str() != Some("ES256"))
        || key.get("use").is_some_and(|v| v.as_str() != Some("sig"))
        || key.get("d").is_some()
        || key.get("key_ops").is_some_and(|v| {
            !v.as_array()
                .is_some_and(|a| a.len() == 1 && a[0].as_str() == Some("verify"))
        })
    {
        return Err(OAuthError::InvalidIdentity);
    }
    let mut output = [0_u8; 65];
    output[0] = 4;
    output[1..33].copy_from_slice(&coordinate(&key, "x")?);
    output[33..65].copy_from_slice(&coordinate(&key, "y")?);
    Ok(output)
}
fn coordinate(key: &Value, field: &str) -> Result<[u8; 32], OAuthError> {
    let encoded = key
        .get(field)
        .and_then(Value::as_str)
        .ok_or(OAuthError::InvalidIdentity)?;
    URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| OAuthError::InvalidIdentity)?
        .try_into()
        .map_err(|_| OAuthError::InvalidIdentity)
}
