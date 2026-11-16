//! RS256 ID-token validation is independent of access-token delivery and Gateway management auth.
use super::{reply, site::atom, OAuthClient, OAuthError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{RsaPublicKeyComponents, RSA_PKCS1_2048_8192_SHA256};
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub struct VerifiedIdentity {
    issuer: String,
    subject: String,
    client_id: String,
    expires_at: u64,
}
impl VerifiedIdentity {
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub fn subject(&self) -> &str {
        &self.subject
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
}
impl std::fmt::Debug for VerifiedIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedDimIdentity { identity: <redacted> }")
    }
}

/// A missing/foreign nonce must never be replaced with a token claim or caller-supplied subject.
pub(super) fn verify(
    client: &OAuthClient,
    token: &str,
    nonce: &str,
    access: &str,
    keys: &[u8],
    now: u64,
) -> Result<VerifiedIdentity, OAuthError> {
    if !atom(nonce, 256) {
        return Err(OAuthError::InvalidIdentity);
    }
    claims(
        client,
        signed_claims(token, keys)?,
        Some(nonce),
        access,
        now,
    )
}
fn signed_claims(token: &str, keys: &[u8]) -> Result<Value, OAuthError> {
    if !atom(token, 32_768) {
        return Err(OAuthError::InvalidIdentity);
    }
    let mut parts = token.split('.');
    let header = parts.next().ok_or(OAuthError::InvalidIdentity)?;
    let body = parts.next().ok_or(OAuthError::InvalidIdentity)?;
    let signature = parts.next().ok_or(OAuthError::InvalidIdentity)?;
    if parts.next().is_some() {
        return Err(OAuthError::InvalidIdentity);
    }
    let header = decode_object(header)?;
    if header["alg"].as_str() != Some("RS256")
        || ["crit", "b64", "jku", "jwk", "x5u", "x5c"]
            .iter()
            .any(|k| header.get(k).is_some())
    {
        return Err(OAuthError::InvalidIdentity);
    }
    let kid = header["kid"]
        .as_str()
        .filter(|v| atom(v, 256))
        .ok_or(OAuthError::InvalidIdentity)?;
    let (n, e) = select_key(keys, kid)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| OAuthError::InvalidIdentity)?;
    let signed = token.rsplit_once('.').ok_or(OAuthError::InvalidIdentity)?.0;
    RsaPublicKeyComponents { n: &n, e: &e }
        .verify(&RSA_PKCS1_2048_8192_SHA256, signed.as_bytes(), &signature)
        .map_err(|_| OAuthError::InvalidIdentity)?;
    decode_object(body)
}
fn select_key(bytes: &[u8], kid: &str) -> Result<(Vec<u8>, Vec<u8>), OAuthError> {
    let keys = reply::object(bytes, 65_536).map_err(|_| OAuthError::InvalidIdentity)?;
    let keys = keys["keys"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 32)
        .ok_or(OAuthError::InvalidIdentity)?;
    let mut matching = keys.iter().filter(|key| key["kid"].as_str() == Some(kid));
    let key = matching.next().ok_or(OAuthError::InvalidIdentity)?;
    if matching.next().is_some()
        || key["kty"].as_str() != Some("RSA")
        || key.get("alg").is_some_and(|v| v.as_str() != Some("RS256"))
        || key.get("use").is_some_and(|v| v.as_str() != Some("sig"))
        || ["d", "p", "q", "dp", "dq", "qi", "oth"]
            .iter()
            .any(|k| key.get(k).is_some())
        || key.get("key_ops").is_some_and(|v| {
            !v.as_array()
                .is_some_and(|a| a.len() == 1 && a[0].as_str() == Some("verify"))
        })
    {
        return Err(OAuthError::InvalidIdentity);
    }
    let decode = |field: &str| {
        URL_SAFE_NO_PAD
            .decode(key[field].as_str().ok_or(OAuthError::InvalidIdentity)?)
            .map_err(|_| OAuthError::InvalidIdentity)
    };
    let n = decode("n")?;
    let e = decode("e")?;
    if !(256..=1024).contains(&n.len())
        || n.first().is_none_or(|v| *v < 128)
        || e.is_empty()
        || e.len() > 5
        || e[0] == 0
    {
        return Err(OAuthError::InvalidIdentity);
    }
    Ok((n, e))
}
fn decode_object(encoded: &str) -> Result<Value, OAuthError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| OAuthError::InvalidIdentity)?;
    reply::object(&bytes, 32_768).map_err(|_| OAuthError::InvalidIdentity)
}
fn claims(
    client: &OAuthClient,
    body: Value,
    nonce: Option<&str>,
    access: &str,
    now: u64,
) -> Result<VerifiedIdentity, OAuthError> {
    if !atom(access, 32_768) {
        return Err(OAuthError::InvalidIdentity);
    }
    let text = |name: &str| body[name].as_str().ok_or(OAuthError::InvalidIdentity);
    let number = |name: &str| body[name].as_u64().ok_or(OAuthError::InvalidIdentity);
    if text("iss")? != client.site().issuer()
        || nonce.is_some_and(|expected| {
            !body["nonce"]
                .as_str()
                .is_some_and(|actual| bool::from(actual.as_bytes().ct_eq(expected.as_bytes())))
        })
    {
        return Err(OAuthError::InvalidIdentity);
    }
    let subject = text("sub")?;
    if !atom(subject, 255) {
        return Err(OAuthError::InvalidIdentity);
    }
    let aud = &body["aud"];
    if !(aud.as_str() == Some(client.client_id())
        || aud
            .as_array()
            .is_some_and(|a| a.len() == 1 && a[0].as_str() == Some(client.client_id())))
        || body
            .get("azp")
            .is_some_and(|v| v.as_str() != Some(client.client_id()))
    {
        return Err(OAuthError::InvalidIdentity);
    }
    let exp = number("exp")?;
    let iat = number("iat")?;
    let latest = now.checked_add(60).ok_or(OAuthError::InvalidIdentity)?;
    if exp.checked_add(60).is_none_or(|end| end <= now)
        || iat > latest
        || exp <= iat
        || body
            .get("nbf")
            .is_some_and(|v| !v.as_u64().is_some_and(|n| n <= latest && n < exp))
    {
        return Err(OAuthError::InvalidIdentity);
    }
    if let Some(hash) = body.get("at_hash") {
        let expected = URL_SAFE_NO_PAD.encode(&Sha256::digest(access.as_bytes())[..16]);
        if !hash
            .as_str()
            .is_some_and(|v| bool::from(v.as_bytes().ct_eq(expected.as_bytes())))
        {
            return Err(OAuthError::InvalidIdentity);
        }
    }
    Ok(VerifiedIdentity {
        issuer: client.site().issuer().into(),
        subject: subject.into(),
        client_id: client.client_id().into(),
        expires_at: exp,
    })
}

pub(super) fn verify_refresh(
    client: &OAuthClient,
    token: &str,
    access: &str,
    initial_nonce: &str,
    keys: &[u8],
    now: u64,
) -> Result<VerifiedIdentity, OAuthError> {
    let body = signed_claims(token, keys)?;
    let expected = body.get("nonce").map(|_| initial_nonce);
    claims(client, body, expected, access, now)
}
