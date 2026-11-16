//! An opaque MCP bearer is not a verified identity, team, subscription or quota assertion.
use super::OAuthError;
use serde::Deserialize;
use time::OffsetDateTime;

pub struct TokenMaterial {
    access: String,
    refresh: Option<String>,
    offline_access: bool,
    expires_at: Option<OffsetDateTime>,
    client_id: String,
    redirect_uri: String,
}

#[derive(Deserialize)]
struct Reply {
    access_token: String,
    token_type: String,
    expires_in: Option<u64>,
    error: Option<serde::de::IgnoredAny>,
}

impl TokenMaterial {
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh.as_deref()
    }
    pub fn offline_access(&self) -> bool {
        self.offline_access
    }
    pub(super) fn with_offline_access(mut self, offline_access: bool) -> Self {
        self.offline_access = offline_access;
        self
    }
    pub(super) fn parse_offline(
        bytes: &[u8],
        started_at: OffsetDateTime,
        client_id: String,
        redirect_uri: String,
    ) -> Result<Self, OAuthError> {
        let mut material = Self::parse(bytes, started_at, client_id, redirect_uri)?;
        material.refresh =
            Some(super::refresh::response_refresh(bytes)?.ok_or(OAuthError::InvalidResponse)?);
        material.offline_access = true;
        Ok(material)
    }
    pub fn access_token(&self) -> &str {
        &self.access
    }
    pub fn expires_at(&self) -> Option<OffsetDateTime> {
        self.expires_at
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub(super) fn restore(
        access: &str,
        expires_at: Option<OffsetDateTime>,
        client_id: &str,
        redirect_uri: &str,
    ) -> Result<Self, OAuthError> {
        if !valid_access(access) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            access: access.into(),
            refresh: None,
            offline_access: false,
            expires_at,
            client_id: client_id.into(),
            redirect_uri: redirect_uri.into(),
        })
    }

    pub(super) fn parse(
        bytes: &[u8],
        started_at: OffsetDateTime,
        client_id: String,
        redirect_uri: String,
    ) -> Result<Self, OAuthError> {
        let material = Self::parse_stored(bytes, started_at, client_id, redirect_uri)?;
        if material
            .expires_at
            .is_some_and(|at| at <= OffsetDateTime::now_utc())
        {
            return Err(OAuthError::Expired);
        }
        Ok(material)
    }

    // Historical receipt admission is time-independent; use sites still enforce bearer freshness.
    pub(super) fn parse_stored(
        bytes: &[u8],
        started_at: OffsetDateTime,
        client_id: String,
        redirect_uri: String,
    ) -> Result<Self, OAuthError> {
        // Typed deserialization rejects duplicate known fields; unknown id/refresh tokens are discarded.
        let reply: Reply =
            serde_json::from_slice(bytes).map_err(|_| OAuthError::InvalidResponse)?;
        if reply.error.is_some()
            || !reply.token_type.eq_ignore_ascii_case("bearer")
            || !valid_access(&reply.access_token)
        {
            return Err(OAuthError::InvalidResponse);
        }
        // Anchor at send time, not parse time: network delay cannot extend the server's lifetime.
        let expires_at = reply
            .expires_in
            .map(|seconds| {
                let seconds = i64::try_from(seconds)
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or(OAuthError::InvalidResponse)?;
                started_at
                    .checked_add(time::Duration::seconds(seconds))
                    .ok_or(OAuthError::InvalidResponse)
            })
            .transpose()?;
        Ok(Self {
            access: reply.access_token,
            refresh: None,
            offline_access: false,
            expires_at,
            client_id,
            redirect_uri,
        })
    }
}

pub(crate) fn valid_access(access: &str) -> bool {
    !access.is_empty()
        && access.len() <= 32_768
        && access
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~+/=".contains(&b))
}

impl std::fmt::Debug for TokenMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LumaTokenMaterial { material: <redacted>, identity: unverified }")
    }
}
