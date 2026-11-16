//! DIM protocol boundaries. A parsed catalog or prepared authorization is not a saved account.
mod authorization;
pub mod business;
mod catalog;
mod context;
mod grant;
mod identity;
pub(crate) mod ingress;
mod metadata;
mod model_catalog;
mod reply;
pub mod runtime;
mod site;
mod token_transport;
mod transport;

pub use authorization::{Authorization, CodeGrant};
pub use catalog::{parse_catalog, DeclaredModel, WireProtocol};
pub use context::{AuthorizationContext, RefreshGrant, RefreshedTokens};
pub use grant::{GrantReceipt, IdentityAssurance};
pub use identity::VerifiedIdentity;
pub use metadata::Metadata;
pub use model_catalog::ModelCatalog;
pub use site::{OAuthClient, Site};

/// Only local classified failures escape; never include upstream prose, URLs or token material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("dim_oauth_invalid_input")]
    InvalidInput,
    #[error("dim_oauth_invalid_metadata")]
    InvalidMetadata,
    #[error("dim_oauth_invalid_identity")]
    InvalidIdentity,
    #[error("dim_oauth_identity_mismatch")]
    IdentityMismatch,
    #[error("dim_oauth_scope_mismatch")]
    ScopeMismatch,
    #[error("dim_oauth_invalid_response")]
    InvalidResponse,
    #[error("dim_oauth_response_too_large")]
    ResponseTooLarge,
    #[error("dim_oauth_no_supported_models")]
    NoSupportedModels,
    #[error("dim_oauth_invalid_callback")]
    InvalidCallback,
    #[error("dim_oauth_state_mismatch")]
    StateMismatch,
    #[error("dim_oauth_expired")]
    Expired,
    #[error("dim_oauth_access_denied")]
    AccessDenied,
    #[error("dim_oauth_rejected")]
    Rejected,
    #[error("dim_oauth_entropy_unavailable")]
    Entropy,
    #[error("dim_oauth_redirect_rejected")]
    RedirectRejected,
    #[error("dim_oauth_reauthorization_required")]
    ReauthorizationRequired,
    #[error("dim_oauth_timeout")]
    Timeout,
    #[error("dim_oauth_transport_failed")]
    Transport,
}

#[cfg(test)]
mod authorization_tests;
#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod grant_tests;
#[cfg(test)]
mod identity_tests;
#[cfg(all(test, feature = "line-dim-user-oauth"))]
mod pipeline_tests;
#[cfg(all(test, feature = "line-dim-user-oauth"))]
mod runtime_tests;
#[cfg(test)]
mod signed_exchange_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transport_tests;
