//! Non-deserializable receipts originate only from this client's fixed token endpoint.
use super::{OAuthClient, OAuthError, PrincipalHint, UnverifiedTokens, VerifiedIdentity};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantFlow {
    Code,
    Device,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantAssurance {
    SignedEndUser,
    TokenEndpointOnly,
}
pub struct GrantReceipt {
    pub(super) client: OAuthClient,
    pub(super) tokens: UnverifiedTokens,
    pub(super) identity: Option<VerifiedIdentity>,
    pub(super) principal: Option<PrincipalHint>,
    pub(super) flow: GrantFlow,
    received: Instant,
    expires_at: Option<u64>,
}
impl GrantReceipt {
    pub(super) fn new(
        client: &OAuthClient,
        tokens: UnverifiedTokens,
        flow: GrantFlow,
    ) -> Result<Self, OAuthError> {
        let received = Instant::now();
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| OAuthError::InvalidInput)?
            .as_secs();
        let expires_at = tokens
            .expires_in
            .map(|seconds| wall.checked_add(seconds).ok_or(OAuthError::InvalidResponse))
            .transpose()?;
        Ok(Self {
            client: client.clone(),
            tokens,
            identity: None,
            principal: None,
            flow,
            received,
            expires_at,
        })
    }
    pub fn client(&self) -> &OAuthClient {
        &self.client
    }
    pub fn tokens(&self) -> &UnverifiedTokens {
        &self.tokens
    }
    pub fn identity(&self) -> Option<&VerifiedIdentity> {
        self.identity.as_ref()
    }
    pub fn principal_hint(&self) -> Option<&PrincipalHint> {
        self.principal.as_ref()
    }
    pub fn flow(&self) -> GrantFlow {
        self.flow
    }
    pub fn assurance(&self) -> GrantAssurance {
        if self.identity.is_some() {
            GrantAssurance::SignedEndUser
        } else {
            GrantAssurance::TokenEndpointOnly
        }
    }
    pub fn expires_at_unix_secs(&self) -> Option<u64> {
        self.expires_at
    }
    pub fn is_expired(&self) -> bool {
        self.tokens
            .expires_in
            .is_some_and(|s| self.received.elapsed() >= Duration::from_secs(s))
    }
}
impl std::fmt::Debug for GrantReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrokGrantReceipt { material: <redacted> }")
    }
}
#[derive(Debug)]
pub enum GrantPoll {
    Pending,
    SlowDown,
    Authorized(GrantReceipt),
}
