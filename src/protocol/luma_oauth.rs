//! Luma MCP authorization is independent of App cookies and Agents REST keys.
mod authorization;
mod context;
mod material;
mod received;
mod refresh;
mod registration;
mod token_response;
mod transport;
pub use authorization::{Authorization, CodeGrant};
pub use context::AuthorizationContext;
pub(crate) use material::valid_access;
pub use material::TokenMaterial;
pub use received::ReceivedRefresh;
pub(crate) use refresh::valid_refresh;
pub use refresh::{RefreshGrant, RefreshOutcome};
pub use registration::{RegisteredClient, RegistrationRequest, REGISTRATION_ENDPOINT};

pub const ISSUER: &str = "https://agents.lumalabs.ai";
pub const RESOURCE: &str = "https://agents.lumalabs.ai/mcp";
pub const AUTHORIZE_ENDPOINT: &str = "https://agents.lumalabs.ai/mcp/oauth/authorize";
pub const TOKEN_ENDPOINT: &str = "https://auth.lumalabs.ai/oauth2/token";

/// Fixed local errors only; provider prose, codes, callback URLs and secrets never escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("luma_oauth_invalid_input")]
    InvalidInput,
    #[error("luma_oauth_invalid_callback")]
    InvalidCallback,
    #[error("luma_oauth_state_mismatch")]
    StateMismatch,
    #[error("luma_oauth_entropy_failed")]
    Entropy,
    #[error("luma_oauth_expired")]
    Expired,
    #[error("luma_oauth_access_denied")]
    AccessDenied,
    #[error("luma_oauth_rejected")]
    Rejected,
    #[error("luma_oauth_transport_failed")]
    Transport,
    #[error("luma_oauth_timeout")]
    Timeout,
    #[error("luma_oauth_response_too_large")]
    ResponseTooLarge,
    #[error("luma_oauth_invalid_response")]
    InvalidResponse,
    #[error("luma_oauth_redirect_rejected")]
    RedirectRejected,
}

fn atom(value: &str, limit: usize) -> bool {
    !value.is_empty()
        && value.len() <= limit
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod offline_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "accio_oauth/transport_fixture.rs"]
mod transport_fixture;
