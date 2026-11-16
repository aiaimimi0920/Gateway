//! Claim checks run only after ES256 verification; no claim here grants Gateway management access.
use super::{
    super::{valid_atom, OAuthClient, OAuthError},
    VerifiedIdentity,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

const CLOCK_SKEW: u64 = 60;
pub(super) fn validate(
    client: &OAuthClient,
    body: Value,
    nonce: &str,
    access_token: &str,
    now: u64,
) -> Result<VerifiedIdentity, OAuthError> {
    let text = |key: &str| {
        body.get(key)
            .and_then(Value::as_str)
            .ok_or(OAuthError::InvalidIdentity)
    };
    if text("iss")? != client.issuer
        || !valid_atom(nonce, 256)
        || !bool::from(nonce.as_bytes().ct_eq(text("nonce")?.as_bytes()))
    {
        return Err(OAuthError::InvalidIdentity);
    }
    let subject = text("sub")?;
    if subject.is_empty()
        || subject.len() > 255
        || !subject
            .bytes()
            .all(|b| b.is_ascii() && !b.is_ascii_control())
    {
        return Err(OAuthError::InvalidIdentity);
    }
    // No additional audience has been explicitly trusted by this client. Accept the string or
    // singleton-array encoding, not arbitrary extra audiences just because azp names this client.
    let aud = body.get("aud").ok_or(OAuthError::InvalidIdentity)?;
    if !(aud.as_str() == Some(client.client_id.as_str())
        || aud
            .as_array()
            .is_some_and(|a| a.len() == 1 && a[0].as_str() == Some(client.client_id.as_str())))
        || body
            .get("azp")
            .is_some_and(|v| v.as_str() != Some(client.client_id.as_str()))
    {
        return Err(OAuthError::InvalidIdentity);
    }
    let number = |key: &str| {
        body.get(key)
            .and_then(Value::as_u64)
            .ok_or(OAuthError::InvalidIdentity)
    };
    let exp = number("exp")?;
    let iat = number("iat")?;
    let latest = now
        .checked_add(CLOCK_SKEW)
        .ok_or(OAuthError::InvalidIdentity)?;
    if exp
        .checked_add(CLOCK_SKEW)
        .ok_or(OAuthError::InvalidIdentity)?
        <= now
        || iat > latest
        || exp <= iat
        || body
            .get("nbf")
            .is_some_and(|v| !v.as_u64().is_some_and(|n| n <= latest && n < exp))
    {
        return Err(OAuthError::InvalidIdentity);
    }
    if let Some(hash) = body.get("at_hash") {
        if !valid_atom(access_token, 32_768) {
            return Err(OAuthError::InvalidIdentity);
        }
        let expected = URL_SAFE_NO_PAD.encode(&Sha256::digest(access_token.as_bytes())[..16]);
        if !hash
            .as_str()
            .is_some_and(|h| bool::from(expected.as_bytes().ct_eq(h.as_bytes())))
        {
            return Err(OAuthError::InvalidIdentity);
        }
    }
    Ok(VerifiedIdentity {
        issuer: client.issuer.clone(),
        subject: subject.to_owned(),
        client_id: client.client_id.clone(),
        expires_at: exp,
    })
}
