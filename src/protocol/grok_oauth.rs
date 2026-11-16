//! Grok Build OAuth is separate from xAI API keys and grok.com browser sessions.
mod authorization;
pub mod business;
mod context;
mod grants;
mod material;
mod oidc;
mod principal_hint;
mod receipt;
mod transport;
pub use authorization::Authorization;
pub use context::{AuthorizationContext, TOKEN_ENDPOINT};
pub use material::{DeviceChallenge, DevicePoll, UnverifiedTokens};
pub use oidc::VerifiedIdentity;
pub use principal_hint::{PrincipalConstraint, PrincipalHint};
pub use receipt::{GrantAssurance, GrantFlow, GrantPoll, GrantReceipt};
pub const ISSUER: &str = "https://auth.x.ai";
pub const BUSINESS_BASE_URL: &str = "https://cli-chat-proxy.grok.com/v1";

/// Only fixed local errors escape; OAuth prose, codes, URLs and tokens are not diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("grok_oauth_invalid_input")]
    InvalidInput,
    #[error("grok_oauth_invalid_callback")]
    InvalidCallback,
    #[error("grok_oauth_state_mismatch")]
    StateMismatch,
    #[error("grok_oauth_transport_failed")]
    Transport,
    #[error("grok_oauth_timeout")]
    Timeout,
    #[error("grok_oauth_response_too_large")]
    ResponseTooLarge,
    #[error("grok_oauth_invalid_response")]
    InvalidResponse,
    #[error("grok_oauth_invalid_identity")]
    InvalidIdentity,
    #[error("grok_oauth_principal_mismatch")]
    PrincipalMismatch,
    #[error("grok_oauth_redirect_rejected")]
    RedirectRejected,
    #[error("grok_oauth_rejected")]
    Rejected,
    #[error("grok_oauth_reauthorization_required")]
    ReauthorizationRequired,
    #[error("grok_oauth_access_denied")]
    AccessDenied,
    #[error("grok_oauth_expired")]
    Expired,
}

/// Principal is authorization context, not a reason to merge personal/team quota pools.
#[derive(Clone, PartialEq, Eq)]
pub struct Principal {
    kind: String,
    id: String,
}
impl Principal {
    pub fn new(kind: &str, id: &str) -> Result<Self, OAuthError> {
        if !valid_atom(kind, 128) || !valid_atom(id, 512) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            kind: kind.into(),
            id: id.into(),
        })
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// The operator must supply a client they may use. No borrowed first-party client identity.
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthClient {
    client_id: String,
    scope: String,
    principal: Option<Principal>,
    issuer: String,
}
impl OAuthClient {
    pub fn new(client_id: &str, scopes: &[&str]) -> Result<Self, OAuthError> {
        let mut seen = std::collections::HashSet::new();
        if !valid_atom(client_id, 512)
            || scopes.is_empty()
            || scopes.len() > 32
            || scopes.iter().any(|scope| {
                !valid_atom(scope, 128)
                    || scope.bytes().any(|b| matches!(b, b'"' | b'\\'))
                    || !seen.insert(*scope)
            })
        {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            client_id: client_id.into(),
            scope: scopes.join(" "),
            principal: None,
            issuer: ISSUER.into(),
        })
    }
    pub fn with_principal(mut self, principal: Principal) -> Self {
        self.principal = Some(principal);
        self
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn principal(&self) -> Option<&Principal> {
        self.principal.as_ref()
    }
    #[cfg(test)]
    pub(crate) fn test_issuer(mut self, issuer: &str) -> Self {
        let url = url::Url::parse(issuer).unwrap();
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert!(url.port().is_some());
        self.issuer = issuer.into();
        self
    }
}
pub(super) fn valid_atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}
impl std::fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrokOAuthClient { explicit_client: true, principal: <redacted> }")
    }
}
#[cfg(test)]
mod tests;
