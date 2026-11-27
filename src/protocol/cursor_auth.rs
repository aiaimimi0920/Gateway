//! Cursor account authorization only. No generation, refresh, SDK or local Cursor runtime.
mod transport;
pub(crate) use transport::client;
#[cfg(test)]
pub(crate) use transport::TEST_ORIGIN;
#[cfg(test)]
pub(crate) mod tests;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("{0}")]
pub(crate) struct AuthError(pub &'static str);
pub(crate) struct Challenge {
    pub url: String,
    pub uuid: String,
    verifier: String,
}
impl Challenge {
    pub fn new() -> Result<Self, AuthError> {
        let mut bytes = [0; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| AuthError("cursor_random_failed"))?;
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let uuid = uuid::Uuid::new_v4().to_string();
        let mut url = url::Url::parse("https://www.cursor.com/loginDeepControl").unwrap();
        url.query_pairs_mut().extend_pairs([
            (
                "challenge",
                URL_SAFE_NO_PAD
                    .encode(Sha256::digest(verifier.as_bytes()))
                    .as_str(),
            ),
            ("uuid", uuid.as_str()),
            ("mode", "login"),
            ("redirectTarget", "cli"),
        ]);
        Ok(Self {
            url: url.to_string(),
            uuid,
            verifier,
        })
    }
}
#[derive(Deserialize)]
pub(crate) struct Claims {
    pub sub: String,
    pub exp: i64,
}
pub(crate) fn claims(token: &str) -> Result<Claims, AuthError> {
    let invalid = || AuthError("cursor_invalid_token");
    if token.len() > 32768 {
        return Err(invalid());
    }
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
    {
        return Err(invalid());
    }
    let decoded = URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| invalid())?;
    let claims: Claims = serde_json::from_slice(&decoded).map_err(|_| invalid())?;
    if claims.sub.is_empty()
        || claims.sub.len() > 256
        || !claims
            .sub
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_|.-".contains(&b))
        || time::OffsetDateTime::from_unix_timestamp(claims.exp).is_err()
    {
        return Err(invalid());
    }
    Ok(claims)
}
pub(crate) fn require_fresh(exp: i64) -> Result<(), AuthError> {
    if exp
        <= time::OffsetDateTime::now_utc()
            .unix_timestamp()
            .saturating_add(60)
    {
        return Err(AuthError("cursor_token_expired"));
    }
    Ok(())
}
pub(crate) fn normalize_email(email: &str) -> Result<String, AuthError> {
    let value = email.trim().to_lowercase();
    if value.is_empty()
        || value.len() > 320
        || !value.contains('@')
        || value.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(AuthError("cursor_invalid_expected_email"));
    }
    Ok(value)
}
pub(crate) struct Material {
    pub token: String,
    pub subject: String,
    pub email: String,
    pub exp: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    auth_id: String,
    email: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginReply {
    access_token: Option<String>,
    api_key: Option<String>,
}

pub(crate) async fn poll(
    http: &rquest::Client,
    challenge: &Challenge,
) -> Result<Option<String>, AuthError> {
    let mut url = transport::target("/auth/poll");
    url.query_pairs_mut()
        .extend_pairs([("uuid", &challenge.uuid), ("verifier", &challenge.verifier)]);
    let (status, body) = transport::read(http.get(url.as_str())).await?;
    if status == 202 || status == 404 {
        return Ok(None);
    }
    if status != 200 {
        return Err(AuthError("cursor_login_rejected"));
    }
    let reply: LoginReply =
        serde_json::from_slice(&body).map_err(|_| AuthError("cursor_invalid_login_response"))?;
    if reply.access_token.is_some()
        && reply.api_key.is_some()
        && reply.access_token != reply.api_key
    {
        return Err(AuthError("cursor_ambiguous_login_token"));
    }
    let token = reply
        .access_token
        .or(reply.api_key)
        .ok_or(AuthError("cursor_missing_login_token"))?;
    // Receipt is a separate fact from validity. verify() rejects invalid material before GetMe.
    Ok(Some(token))
}
pub(crate) async fn verify(
    http: &rquest::Client,
    token: String,
    expected_email: Option<&str>,
) -> Result<Material, AuthError> {
    let claims = claims(&token)?;
    require_fresh(claims.exp)?;
    // Older clients may add an email constraint; the authenticated identity is always authoritative.
    let expected = expected_email.map(normalize_email).transpose()?;
    let request = http
        .post(transport::target("/aiserver.v1.DashboardService/GetMe").as_str())
        .bearer_auth(&token)
        .header("connect-protocol-version", "1")
        .header("content-type", "application/json")
        .body("{}");
    let (status, bytes) = transport::read(request).await?;
    if status != 200 {
        return Err(AuthError("cursor_identity_rejected"));
    }
    let me: Identity = serde_json::from_slice(&bytes)
        .map_err(|_| AuthError("cursor_invalid_identity_response"))?;
    let email =
        normalize_email(&me.email).map_err(|_| AuthError("cursor_invalid_identity_response"))?;
    if me.auth_id != claims.sub || expected.as_ref().is_some_and(|value| value != &email) {
        return Err(AuthError("cursor_account_identity_mismatch"));
    }
    Ok(Material {
        token,
        subject: claims.sub,
        email,
        exp: claims.exp,
    })
}
