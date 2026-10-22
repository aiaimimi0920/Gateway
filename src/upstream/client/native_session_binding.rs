//! Bind the session to the effective post-refresh credential, origin and operator headers.
use super::*;
use sha2::{Digest, Sha256};

pub(super) fn key(
    payload: &ProviderAccountPayload,
    request: &reqwest::Request,
) -> Result<Scope, GatewayError> {
    if !eligible(payload) {
        return Err(GatewayError::bad_request(
            "Native session requires the official Codex OAuth provider",
        ));
    }
    let origin = request.url().origin().ascii_serialization();
    if origin != "https://chatgpt.com" {
        return Err(GatewayError::bad_request("Native session origin mismatch"));
    }
    let mut hash = Sha256::new();
    let credential = payload.credential_id.as_deref().unwrap_or("");
    for value in [origin.as_bytes(), credential.as_bytes()] {
        hash.update(value.len().to_le_bytes());
        hash.update(value);
    }
    for name in [
        "authorization",
        "api-key",
        "x-api-key",
        "chatgpt-account-id",
    ] {
        let value = request
            .headers()
            .get(name)
            .map(|v| v.as_bytes())
            .unwrap_or_default();
        hash.update(value.len().to_le_bytes());
        hash.update(value);
    }
    let mut configured: Vec<_> = payload.headers.iter().collect();
    configured.sort();
    for (name, value) in configured {
        for part in [name, value] {
            hash.update(part.len().to_le_bytes());
            hash.update(part.as_bytes());
        }
    }
    Ok(hash.finalize().into())
}
