//! Token parsing never establishes identity; only bounded verification can construct a receipt.
use super::{identity::VerifiedIdentity, reply, site::atom, Metadata, OAuthError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::Instant;

#[derive(Deserialize)]
pub(super) struct Tokens {
    pub(super) access_token: String,
    pub(super) refresh_token: Option<String>,
    pub(super) id_token: Option<String>,
    pub(super) expires_in: u64,
    pub(super) scope: Option<String>,
    token_type: String,
    #[serde(skip)]
    pub(super) expires_at_unix: u64,
}
impl Tokens {
    pub(super) fn parse(
        bytes: &[u8],
        require_refresh: bool,
        started_unix: u64,
    ) -> Result<Self, OAuthError> {
        let value = reply::object(bytes, 65_536)?;
        if value.get("error").is_some() {
            return Err(OAuthError::InvalidResponse);
        }
        let mut tokens: Self =
            serde_json::from_value(value).map_err(|_| OAuthError::InvalidResponse)?;
        if !tokens.token_type.eq_ignore_ascii_case("bearer")
            || !atom(&tokens.access_token, 32_768)
            || (require_refresh && tokens.refresh_token.is_none())
            || tokens
                .refresh_token
                .as_deref()
                .is_some_and(|t| !atom(t, 32_768))
            || tokens.id_token.as_deref().is_some_and(|t| !atom(t, 32_768))
            || tokens.expires_in == 0
            || tokens.expires_in > 100 * 365 * 86400
            || tokens
                .scope
                .as_ref()
                .is_some_and(|s| parse_scopes(s).is_err())
        {
            return Err(OAuthError::InvalidResponse);
        }
        tokens.expires_at_unix = started_unix
            .checked_add(tokens.expires_in)
            .ok_or(OAuthError::InvalidResponse)?;
        Ok(tokens)
    }
}

/// Endpoint identity is explicit; only `SignedIdToken` denotes JOSE-verified identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityAssurance {
    UserInfoEndpoint,
    SignedIdToken,
}

pub struct GrantReceipt {
    pub(super) metadata: Metadata,
    pub(super) tokens: Tokens,
    pub(super) subject: String,
    pub(super) user_id: String,
    pub(super) account_label: Option<String>,
    pub(super) granted_scopes: Option<Vec<String>>,
    pub(super) assurance: IdentityAssurance,
    pub(super) nonce: String,
    pub(super) deadline: Instant,
}
impl GrantReceipt {
    pub(super) fn accept(
        metadata: Metadata,
        tokens: Tokens,
        bytes: &[u8],
        verified: Option<VerifiedIdentity>,
        nonce: String,
    ) -> Result<Self, OAuthError> {
        let user = reply::object(bytes, 65_536)?;
        let text = |name: &str| {
            user[name]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control))
                .ok_or(OAuthError::InvalidIdentity)
        };
        let subject = text("sub")?;
        if !atom(subject, 255) {
            return Err(OAuthError::InvalidIdentity);
        }
        let user_id = if user.get("uid").is_some() {
            text("uid")?
        } else {
            subject
        };
        if let Some(identity) = &verified {
            if identity.subject() != subject
                || identity.issuer() != metadata.client().site().issuer()
                || identity.client_id() != metadata.client().client_id()
            {
                return Err(OAuthError::IdentityMismatch);
            }
        }
        let account_label = user
            .get("email")
            .map(|_| text("email").map(str::to_owned))
            .transpose()?;
        let granted = tokens.scope.as_deref().map(parse_scopes).transpose()?;
        if granted.as_ref().is_some_and(|scopes| {
            scopes
                .iter()
                .any(|s| !metadata.client().scopes().contains(s))
        }) {
            return Err(OAuthError::ScopeMismatch);
        }
        let now = unix_now()?;
        let remaining = tokens
            .expires_at_unix
            .checked_sub(now)
            .filter(|s| *s > 0)
            .ok_or(OAuthError::Expired)?;
        Ok(Self {
            metadata,
            tokens,
            subject: subject.into(),
            user_id: user_id.into(),
            account_label,
            granted_scopes: granted,
            assurance: if verified.is_some() {
                IdentityAssurance::SignedIdToken
            } else {
                IdentityAssurance::UserInfoEndpoint
            },
            nonce,
            deadline: Instant::now() + Duration::from_secs(remaining),
        })
    }
    pub fn is_expired(&self) -> bool {
        self.deadline <= Instant::now()
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    pub fn identity_assurance(&self) -> IdentityAssurance {
        self.assurance
    }
    pub fn subject(&self) -> &str {
        &self.subject
    }
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
    pub fn account_label(&self) -> Option<&str> {
        self.account_label.as_deref()
    }
    pub fn access_token(&self) -> &str {
        &self.tokens.access_token
    }
    pub fn refresh_token(&self) -> &str {
        self.tokens
            .refresh_token
            .as_deref()
            .expect("initial refresh required")
    }
    pub fn expires_at_unix(&self) -> u64 {
        self.tokens.expires_at_unix
    }
}
pub(super) fn parse_scopes(value: &str) -> Result<Vec<String>, OAuthError> {
    if value.len() > 4096 {
        return Err(OAuthError::InvalidResponse);
    }
    let mut scopes = Vec::new();
    for scope in value.split(' ') {
        if !atom(scope, 128)
            || scope.bytes().any(|b| matches!(b, b'"' | b'\\'))
            || scopes.iter().any(|s| s == scope)
            || scopes.len() >= 32
        {
            return Err(OAuthError::InvalidResponse);
        }
        scopes.push(scope.into());
    }
    Ok(scopes)
}
pub(super) fn unix_now() -> Result<u64, OAuthError> {
    #[cfg(test)]
    if let Ok(now) = TEST_UNIX_NOW.try_with(|now| *now) {
        return Ok(now);
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| OAuthError::InvalidResponse)
}
#[cfg(test)]
tokio::task_local! { pub(super) static TEST_UNIX_NOW: u64; }
impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DimTokens { material: <redacted> }")
    }
}
impl std::fmt::Debug for GrantReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DimGrantReceipt { material: <redacted> }")
    }
}
