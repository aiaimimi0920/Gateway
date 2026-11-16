//! Explicit Google desktop OAuth for Full Vertex; never load another product's client or ADC.
mod authorization;
pub(crate) mod business;
mod context;
pub(crate) mod ingress;
mod material;
mod target;
mod transport;
#[cfg(feature = "line-google-agent-platform-official")]
mod transport_http;
pub use authorization::{AuthorizationAttempt, CodeGrant};
pub use context::{AuthorizationContext, GrantType};
pub use material::TokenMaterial;
pub use target::VertexTarget;

pub const ISSUER: &str = "https://accounts.google.com";
pub const AUTHORIZE_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
pub const CLOUD_PLATFORM_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

/// Only local fixed diagnostics escape; no callback, credential or upstream prose is displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("vertex_oauth_invalid_input")]
    InvalidInput,
    #[error("vertex_oauth_entropy_unavailable")]
    Entropy,
    #[error("vertex_oauth_invalid_callback")]
    InvalidCallback,
    #[error("vertex_oauth_state_mismatch")]
    StateMismatch,
    #[error("vertex_oauth_access_denied")]
    AccessDenied,
    #[error("vertex_oauth_expired")]
    Expired,
    #[error("vertex_oauth_transport_failed")]
    Transport,
    #[error("vertex_oauth_timeout")]
    Timeout,
    #[error("vertex_oauth_redirect_rejected")]
    RedirectRejected,
    #[error("vertex_oauth_response_too_large")]
    ResponseTooLarge,
    #[error("vertex_oauth_invalid_response")]
    InvalidResponse,
    #[error("vertex_oauth_rejected")]
    Rejected,
    #[error("vertex_oauth_reauthorization_required")]
    ReauthorizationRequired,
    #[error("vertex_oauth_missing_refresh_token")]
    MissingRefreshToken,
    #[error("vertex_oauth_insufficient_scope")]
    InsufficientScope,
    #[error("vertex_oauth_feature_disabled")]
    FeatureDisabled,
}

/// Caller supplies their own desktop client. A secret, when supplied, stays private material.
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthClient {
    client_id: String,
    client_secret: Option<String>,
}
impl OAuthClient {
    pub fn new(client_id: &str, client_secret: Option<&str>) -> Result<Self, OAuthError> {
        if !valid_atom(client_id, 1024) || client_secret.is_some_and(|s| !valid_atom(s, 4096)) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            client_id: client_id.into(),
            client_secret: client_secret.map(str::to_owned),
        })
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
}

/// A prepared single attempt, not durable admission. The existing scheduler must own that fence.
pub struct RefreshGrant {
    client: OAuthClient,
    refresh_token: String,
    refresh_expires_at: Option<time::OffsetDateTime>,
}
impl RefreshGrant {
    pub fn new(
        client: OAuthClient,
        refresh_token: &str,
        refresh_expires_at: Option<time::OffsetDateTime>,
    ) -> Result<Self, OAuthError> {
        if !valid_atom(refresh_token, 32_768) {
            return Err(OAuthError::InvalidInput);
        }
        if refresh_expires_at.is_some_and(|expiry| expiry <= time::OffsetDateTime::now_utc()) {
            return Err(OAuthError::Expired);
        }
        Ok(Self {
            client,
            refresh_token: refresh_token.into(),
            refresh_expires_at,
        })
    }
}
fn valid_atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}
impl std::fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VertexOAuthClient { explicit_client: true, material: <redacted> }")
    }
}
impl std::fmt::Debug for RefreshGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VertexRefreshGrant { material: <redacted> }")
    }
}
#[cfg(test)]
mod authorization_tests;
#[cfg(test)]
mod business_tests;
#[cfg(test)]
mod context_rotation_tests;
#[cfg(test)]
mod context_tests;
#[cfg(all(test, feature = "line-google-agent-platform-official"))]
mod context_wire_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod target_tests;
#[cfg(all(test, feature = "line-google-agent-platform-official"))]
mod transport_boundary_tests;
#[cfg(all(test, feature = "line-google-agent-platform-official"))]
#[path = "accio_oauth/transport_fixture.rs"]
mod transport_fixture;
#[cfg(test)]
pub(crate) mod transport_test_support;
#[cfg(all(test, feature = "line-google-agent-platform-official"))]
mod transport_tests;
