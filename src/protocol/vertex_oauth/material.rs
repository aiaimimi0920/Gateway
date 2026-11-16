//! Endpoint-delivered OAuth material is not a verified Google user or a Vertex entitlement.
use super::{valid_atom, OAuthError, CLOUD_PLATFORM_SCOPE};
use serde::{de::IgnoredAny, Deserialize, Deserializer};
use time::OffsetDateTime;

pub struct TokenMaterial {
    pub(super) access_token: String,
    pub(super) refresh_token: Option<String>,
    pub(super) expires_in: u64,
    pub(super) expires_at: OffsetDateTime,
    pub(super) refresh_expires_at: Option<OffsetDateTime>,
    pub(super) scope_reported: bool,
}
// Ignore unknown fields per Google's contract; serde still rejects duplicate known fields.
#[derive(Deserialize)]
struct Reply {
    access_token: String,
    token_type: String,
    expires_in: u64,
    #[serde(default, deserialize_with = "present")]
    refresh_token: Option<String>,
    #[serde(default, deserialize_with = "present")]
    refresh_token_expires_in: Option<u64>,
    #[serde(default, deserialize_with = "present")]
    scope: Option<String>,
    #[serde(default, deserialize_with = "present")]
    error: Option<IgnoredAny>,
}
// An omitted field is optional; explicit null is not a valid token, scope or duration.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
impl TokenMaterial {
    pub(super) fn parse(
        bytes: &[u8],
        require_refresh: bool,
        started: OffsetDateTime,
    ) -> Result<Self, OAuthError> {
        if bytes.len() > 65_536 {
            return Err(OAuthError::ResponseTooLarge);
        }
        let reply: Reply =
            serde_json::from_slice(bytes).map_err(|_| OAuthError::InvalidResponse)?;
        if reply.error.is_some()
            || !valid_atom(&reply.access_token, 32_768)
            || !reply.token_type.eq_ignore_ascii_case("Bearer")
            || reply
                .refresh_token
                .as_deref()
                .is_some_and(|token| !valid_atom(token, 32_768))
        {
            return Err(OAuthError::InvalidResponse);
        }
        if require_refresh && reply.refresh_token.is_none() {
            return Err(OAuthError::MissingRefreshToken);
        }
        if reply
            .scope
            .as_deref()
            .is_some_and(|scope| scope != CLOUD_PLATFORM_SCOPE)
        {
            return Err(OAuthError::InsufficientScope);
        }
        Ok(Self {
            access_token: reply.access_token,
            refresh_token: reply.refresh_token,
            expires_in: reply.expires_in,
            expires_at: expiry(started, reply.expires_in)?,
            refresh_expires_at: reply
                .refresh_token_expires_in
                .map(|seconds| expiry(started, seconds))
                .transpose()?,
            scope_reported: reply.scope.is_some(),
        })
    }
    pub fn access_token(&self) -> &str {
        &self.access_token
    }
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }
    pub fn expires_in(&self) -> u64 {
        self.expires_in
    }
    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
    pub fn refresh_expires_at(&self) -> Option<OffsetDateTime> {
        self.refresh_expires_at
    }
    pub fn reported_scope(&self) -> Option<&'static str> {
        self.scope_reported.then_some(CLOUD_PLATFORM_SCOPE)
    }
}
fn expiry(started: OffsetDateTime, seconds: u64) -> Result<OffsetDateTime, OAuthError> {
    let seconds = i64::try_from(seconds).map_err(|_| OAuthError::InvalidResponse)?;
    if seconds <= 0 {
        return Err(OAuthError::InvalidResponse);
    }
    let expiry = started
        .checked_add(time::Duration::seconds(seconds))
        .ok_or(OAuthError::InvalidResponse)?;
    if expiry <= OffsetDateTime::now_utc() {
        return Err(OAuthError::Expired);
    }
    Ok(expiry)
}
impl std::fmt::Debug for TokenMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VertexTokenMaterial { material: <redacted>, identity: unverified }")
    }
}
