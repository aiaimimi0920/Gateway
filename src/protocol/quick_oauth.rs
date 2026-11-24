//! Amazon Quick (QuickWork) social OAuth login, durable refresh, and bidirectional WebSocket chat.
//!
//! Security and boundary invariants:
//! - Only classified local error codes escape; upstream OAuth/EventStream error prose is never
//!   forwarded verbatim because it may echo bearer tokens or client secrets.
//! - OIDC dynamic registration and token exchange reject HTTP redirects and enforce bounded bodies.
//! - Credential secrets (`client_secret`, `api_key`, `refresh_token`) are redacted on management reads.

pub mod context;
pub mod eventstream;
pub mod login;
pub(crate) mod messages;
pub(crate) mod metadata;
pub mod runtime;
pub(crate) mod transport;
pub(crate) mod websocket;

pub use context::{commercial_region, AuthorizationContext};
pub use login::{AuthorizedSession, LoginStepOutcome, PendingLogin};
pub(crate) use transport::RefreshGrant;

use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};

pub const PRESET_ID: &str = "quick-oauth";
pub const AUTH_MODE: &str = "quick_oauth";
pub const PORTAL_BASE: &str = "https://quick.aws.com";
pub const DEFAULT_DATA_PLANE: &str =
    "https://cell001.dp.appintegrations.us-east-1.prod.plato.ai.aws.dev";
pub const DEFAULT_WS_HOST: &str = "cell001.dp.appintegrations.us-east-1.prod.plato.ai.aws.dev";
pub const APP_VERSION: &str = "1.2.4660";
pub const SURFACE_TYPE: &str = "QUICK_WORK_DESKTOP_WINDOWS";
pub const OIDC_CLIENT_NAME: &str = "AmazonQuick-Desktop";
pub const OIDC_SCOPE: &str = "quicksight:read_write";
pub const SUPPORTED_MODELS: &[&str] = &["smart", "fast", "balanced", "auto"];

#[cfg(test)]
tokio::task_local! {
    pub(crate) static TEST_DISCOVERY_ORIGIN: String;
    pub(crate) static TEST_OIDC_ORIGIN: String;
    pub(crate) static TEST_DATA_PLANE_ORIGIN: String;
    pub(crate) static TEST_WS_URL: String;
}

/// Stable classified OAuth and transport error codes without secret leakage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("quick_oauth_invalid_input")]
    InvalidInput,
    #[error("quick_oauth_invalid_callback")]
    InvalidCallback,
    #[error("quick_oauth_state_mismatch")]
    StateMismatch,
    #[error("quick_oauth_expired")]
    Expired,
    #[error("quick_oauth_entropy_unavailable")]
    Entropy,
    #[error("quick_oauth_listener_unavailable")]
    Listener,
    #[error("quick_oauth_invalid_response")]
    InvalidResponse,
    #[error("quick_oauth_response_too_large")]
    ResponseTooLarge,
    #[error("quick_oauth_access_denied")]
    AccessDenied,
    #[error("quick_oauth_reauthorization_required")]
    ReauthorizationRequired,
    #[error("quick_oauth_redirect_rejected")]
    RedirectRejected,
    #[error("quick_oauth_timeout")]
    Timeout,
    #[error("quick_oauth_transport_failed")]
    Transport,
}

pub(crate) fn atom(value: &str, max_len: usize) -> bool {
    !value.is_empty() && value.len() <= max_len && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

pub(crate) fn valid_token(value: &str) -> bool {
    atom(value, 32_768)
}

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
        || payload
            .extra_body
            .as_ref()
            .is_some_and(|extra| extra.contains_key(metadata::METADATA_KEY))
}

pub(crate) fn invalid_config() -> GatewayError {
    GatewayError::bad_request("Invalid Amazon Quick OAuth credential configuration")
        .with_code("quick_oauth_invalid_configuration")
}

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod tests;
