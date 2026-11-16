//! Kiro AWS public-client OIDC wire; not a profile entitlement or verified user identity.
//! Official contracts and fixed reference boundaries: `kiro_oauth/REFERENCE.md`.
pub(crate) mod business;
pub mod runtime;
mod runtime_target;
pub use runtime_target::RuntimeTarget;
mod context;
mod device;
mod directory;
mod material;
mod registration;
#[cfg(feature = "line-kiro-official-vendor-api")]
mod transport;
#[cfg(not(feature = "line-kiro-official-vendor-api"))]
#[path = "kiro_oauth/transport_disabled.rs"]
mod transport;
pub use context::{AuthMethod, AuthorizationContext};
pub use device::{DeviceChallenge, DevicePoll};
pub use directory::{Directory, Scope};
pub use material::TokenMaterial;
pub use registration::{ClientRegistration, RefreshGrant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("kiro_oauth_invalid_input")]
    InvalidInput,
    #[error("kiro_oauth_registration_expired")]
    RegistrationExpired,
    #[error("kiro_oauth_expired")]
    Expired,
    #[error("kiro_oauth_access_denied")]
    AccessDenied,
    #[error("kiro_oauth_reauthorization_required")]
    ReauthorizationRequired,
    #[error("kiro_oauth_transport_failed")]
    Transport,
    #[error("kiro_oauth_timeout")]
    Timeout,
    #[error("kiro_oauth_redirect_rejected")]
    RedirectRejected,
    #[error("kiro_oauth_response_too_large")]
    ResponseTooLarge,
    #[error("kiro_oauth_invalid_response")]
    InvalidResponse,
    #[error("kiro_oauth_rejected")]
    Rejected,
    #[error("kiro_oauth_feature_disabled")]
    FeatureDisabled,
}

pub(super) fn atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

/// A bounded wire reply, decoded into typed structs so duplicate security fields are rejected.
pub(super) struct Reply {
    status: u16,
    bytes: Vec<u8>,
}
impl Reply {
    fn decode<T: serde::de::DeserializeOwned>(&self) -> Result<T, OAuthError> {
        serde_json::from_slice(&self.bytes).map_err(|_| OAuthError::InvalidResponse)
    }
    fn success<T: serde::de::DeserializeOwned>(&self) -> Result<T, OAuthError> {
        if !(200..300).contains(&self.status) {
            return Err(self.rejection());
        }
        self.decode()
    }
    fn error_code(&self) -> Option<String> {
        #[derive(serde::Deserialize)]
        struct ErrorReply {
            error: String,
        }
        self.decode::<ErrorReply>().ok().map(|reply| reply.error)
    }
    fn rejection(&self) -> OAuthError {
        if !matches!(self.status, 400 | 401) {
            return OAuthError::Rejected;
        }
        match self.error_code().as_deref() {
            Some("invalid_client" | "invalid_grant" | "unauthorized_client") => {
                OAuthError::ReauthorizationRequired
            }
            Some("access_denied") => OAuthError::AccessDenied,
            Some("expired_token") => OAuthError::Expired,
            _ => OAuthError::Rejected,
        }
    }
}

#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
tokio::task_local! { pub(crate) static TEST_ORIGIN: String; }
// Observe client-side response ownership, not merely arrival at the synthetic server.
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
tokio::task_local! { static TEST_BODY_STARTED: std::sync::Arc<tokio::sync::Notify>; }
#[cfg(test)]
mod context_tests;
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
pub(crate) mod fixture;
#[cfg(test)]
mod material_tests;
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
mod transport_safety_tests;
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
mod transport_tests;
