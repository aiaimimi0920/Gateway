//! Registered client secrets have their own absolute expiry and immutable directory/scope binding.
use super::{atom, transport::post, DeviceChallenge, Directory, OAuthError, Scope, TokenMaterial};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::time::Instant;

#[derive(Clone)]
pub struct ClientRegistration {
    pub(super) directory: Directory,
    pub(super) scopes: Vec<Scope>,
    pub(super) client_id: String,
    pub(super) secret: String,
    pub(super) expires_at: OffsetDateTime,
    pub(super) deadline: Instant,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Registered {
    client_id: String,
    client_secret: String,
    client_id_issued_at: i64,
    client_secret_expires_at: i64,
    error: Option<String>,
}
impl ClientRegistration {
    pub async fn register(
        http: &rquest::Client,
        directory: Directory,
        scopes: &[Scope],
    ) -> Result<Self, OAuthError> {
        if scopes.is_empty()
            || scopes.len() > 5
            || scopes
                .iter()
                .enumerate()
                .any(|(index, scope)| scopes[..index].contains(scope))
        {
            return Err(OAuthError::InvalidInput);
        }
        let reply = post(http, &directory, "/client/register", json!({"clientName":"Neuro Gateway",
            "clientType":"public","scopes":scopes,"grantTypes":["urn:ietf:params:oauth:grant-type:device_code","refresh_token"]}), Duration::from_secs(15)).await?;
        Self::parse(directory, scopes.to_vec(), reply.success()?)
    }
    fn parse(
        directory: Directory,
        scopes: Vec<Scope>,
        wire: Registered,
    ) -> Result<Self, OAuthError> {
        if !atom(&wire.client_id, 4096)
            || !atom(&wire.client_secret, 16_384)
            || wire.error.is_some()
            || wire.client_id_issued_at <= 0
            || wire.client_id_issued_at >= wire.client_secret_expires_at
        {
            return Err(OAuthError::InvalidResponse);
        }
        let expires_at = OffsetDateTime::from_unix_timestamp(wire.client_secret_expires_at)
            .map_err(|_| OAuthError::InvalidResponse)?;
        let remaining = (expires_at - OffsetDateTime::now_utc())
            .try_into()
            .map_err(|_| OAuthError::RegistrationExpired)?;
        let deadline = Instant::now()
            .checked_add(remaining)
            .ok_or(OAuthError::InvalidResponse)?;
        let result = Self {
            directory,
            scopes,
            client_id: wire.client_id,
            secret: wire.client_secret,
            expires_at,
            deadline,
        };
        result.remaining()?;
        Ok(result)
    }
    pub fn directory(&self) -> &Directory {
        &self.directory
    }
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn client_secret(&self) -> &str {
        &self.secret
    }
    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
    pub(super) fn remaining(&self) -> Result<Duration, OAuthError> {
        let wall: Duration = (self.expires_at - OffsetDateTime::now_utc())
            .try_into()
            .map_err(|_| OAuthError::RegistrationExpired)?;
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .min(wall);
        if remaining.is_zero() {
            Err(OAuthError::RegistrationExpired)
        } else {
            Ok(remaining)
        }
    }
    pub async fn begin_device(&self, http: &rquest::Client) -> Result<DeviceChallenge, OAuthError> {
        let started = Instant::now();
        let reply = post(
            http,
            &self.directory,
            "/device_authorization",
            json!({"clientId":self.client_id,
            "clientSecret":self.secret,"startUrl":self.directory.start_url()}),
            self.remaining()?.min(Duration::from_secs(15)),
        )
        .await?;
        self.remaining()?;
        DeviceChallenge::parse(reply.success()?, self.clone(), started)
    }
    pub fn refresh_grant(&self, refresh: &str) -> Result<RefreshGrant, OAuthError> {
        self.remaining()?;
        if !atom(refresh, 32_768) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(RefreshGrant {
            registration: self.clone(),
            refresh: refresh.into(),
        })
    }
}
pub struct RefreshGrant {
    registration: ClientRegistration,
    refresh: String,
}
impl RefreshGrant {
    /// Consumed even on ambiguous failure; durable scheduler admission belongs to its existing owner.
    pub async fn execute(self, http: &rquest::Client) -> Result<TokenMaterial, OAuthError> {
        let client = &self.registration;
        let started = OffsetDateTime::now_utc();
        let reply = post(
            http,
            &client.directory,
            "/token",
            json!({"clientId":client.client_id,
            "clientSecret":client.secret,"grantType":"refresh_token","refreshToken":self.refresh}),
            client.remaining()?.min(Duration::from_secs(15)),
        )
        .await?;
        client.remaining()?;
        TokenMaterial::parse(&reply, started)
    }
}
impl std::fmt::Debug for ClientRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KiroClientRegistration { material: <redacted> }")
    }
}
impl std::fmt::Debug for RefreshGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KiroRefreshGrant { material: <redacted> }")
    }
}
