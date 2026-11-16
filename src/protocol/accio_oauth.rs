//! Accio's PKCE and JSON grants are not generic OAuth form or a verified account identity.
//! Wire reference and its retained MIT notice: see `accio_oauth/REFERENCE.md`.
mod authorization;
mod import_material;
mod material;
#[cfg(feature = "line-accio-web-reverse-api")]
mod transport;
#[cfg(not(feature = "line-accio-web-reverse-api"))]
#[path = "accio_oauth/transport_disabled.rs"]
mod transport;
pub use authorization::{AuthorizationAttempt, CodeGrant, LoginSite};
pub(crate) use import_material::{access as import_access, ImportMaterial};
pub(crate) use material::expiry as parse_absolute_expiry;
pub use material::TokenMaterial;

pub const GATEWAY: &str = "https://phoenix-gw.alibaba.com";
pub const TOKEN_ENDPOINT: &str = "https://phoenix-gw.alibaba.com/api/oauth/token";
pub const REFRESH_ENDPOINT: &str = "https://phoenix-gw.alibaba.com/api/auth/refresh_token";

#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
tokio::task_local! {
    pub(crate) static TEST_REFRESH_ENDPOINT: String;
}
#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
tokio::task_local! {
    pub(crate) static TEST_TOKEN_ENDPOINT: String;
}

/// Fixed, local diagnostics only: no upstream prose, callback URL or credential is displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("accio_oauth_invalid_input")]
    InvalidInput,
    #[error("accio_oauth_entropy_unavailable")]
    Entropy,
    #[error("accio_oauth_invalid_callback")]
    InvalidCallback,
    #[error("accio_oauth_state_mismatch")]
    StateMismatch,
    #[error("accio_oauth_expired")]
    Expired,
    #[error("accio_oauth_access_denied")]
    AccessDenied,
    #[error("accio_oauth_transport_failed")]
    Transport,
    #[error("accio_oauth_timeout")]
    Timeout,
    #[error("accio_oauth_redirect_rejected")]
    RedirectRejected,
    #[error("accio_oauth_response_too_large")]
    ResponseTooLarge,
    #[error("accio_oauth_invalid_response")]
    InvalidResponse,
    #[error("accio_oauth_rejected")]
    Rejected,
    #[error("accio_oauth_feature_disabled")]
    FeatureDisabled,
}

/// Explicit operator-supplied client, never a hard-coded first-party client identity.
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthClient {
    client_id: String,
}
impl OAuthClient {
    pub fn new(client_id: &str) -> Result<Self, OAuthError> {
        if !valid_atom(client_id, 512) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            client_id: client_id.into(),
        })
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
}

/// One prepared HTTP attempt. The shared scheduler, not this wire owner, owns durable admission.
pub struct RefreshGrant {
    access: String,
    refresh: String,
    previous_sgcookie: Option<String>,
}
impl RefreshGrant {
    pub fn new(access: &str, refresh: &str, sgcookie: Option<&str>) -> Result<Self, OAuthError> {
        if !valid_atom(access, 16_384)
            || !valid_atom(refresh, 16_384)
            || sgcookie.is_some_and(|cookie| !valid_cookie(cookie))
        {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            access: access.into(),
            refresh: refresh.into(),
            previous_sgcookie: sgcookie.map(str::to_owned),
        })
    }
}
pub(crate) fn valid_atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}
pub(crate) fn valid_cookie(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 16_384
        && value.bytes().all(|b| (0x20..=0x7e).contains(&b))
}
impl std::fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccioOAuthClient { explicit_client: true }")
    }
}
impl std::fmt::Debug for RefreshGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccioRefreshGrant { material: <redacted> }")
    }
}

#[cfg(test)]
mod authorization_tests;
#[cfg(test)]
mod material_tests;
#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
mod transport_fixture;
#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
mod transport_safety_tests;
#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
mod transport_tests;
