//! Opaque token material; unknown access lifetime stays unknown instead of inventing 3600 seconds.
use super::{atom, OAuthError, Reply};
use serde::Deserialize;
use time::OffsetDateTime;

pub struct TokenMaterial {
    access: String,
    refresh: Option<String>,
    expires_at: Option<OffsetDateTime>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tokens {
    access_token: String,
    refresh_token: Option<String>,
    token_type: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
}
impl TokenMaterial {
    pub fn access_token(&self) -> &str {
        &self.access
    }
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh.as_deref()
    }
    pub fn expires_at(&self) -> Option<OffsetDateTime> {
        self.expires_at
    }
    pub(super) fn parse(reply: &Reply, started: OffsetDateTime) -> Result<Self, OAuthError> {
        let wire: Tokens = reply.success()?;
        if !atom(&wire.access_token, 32_768)
            || wire
                .refresh_token
                .as_deref()
                .is_some_and(|token| !atom(token, 32_768))
            || wire
                .token_type
                .as_deref()
                .is_some_and(|kind| !kind.eq_ignore_ascii_case("Bearer"))
            || wire.error.is_some()
        {
            return Err(OAuthError::InvalidResponse);
        }
        let expires_at = wire
            .expires_in
            .map(|seconds| {
                let seconds = i64::try_from(seconds).map_err(|_| OAuthError::InvalidResponse)?;
                if seconds <= 0 {
                    return Err(OAuthError::InvalidResponse);
                }
                started
                    .checked_add(time::Duration::seconds(seconds))
                    .ok_or(OAuthError::InvalidResponse)
            })
            .transpose()?;
        Ok(Self {
            access: wire.access_token,
            refresh: wire.refresh_token,
            expires_at,
        })
    }
}
impl std::fmt::Debug for TokenMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KiroTokenMaterial { material: <redacted>, identity: unverified }")
    }
}
