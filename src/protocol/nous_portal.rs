//! Explicit-client Nous subscription transport, separate from Hermes profiles and agent login.
pub mod business;
mod catalog;
mod context;
mod device;
pub(crate) mod probe;
mod receipt;
pub mod runtime;
mod transport;
mod wire;

pub use catalog::{Model, ModelCatalog};
pub use context::{AuthorizationContext, AuthorizedTokens, InferenceTarget};
pub use device::{DeviceChallenge, DevicePoll};
pub use receipt::ReceivedTokens;
#[cfg(test)]
pub(crate) use transport::TEST_ORIGIN;

pub const PORTAL: &str = "https://portal.nousresearch.com";
pub const TOKEN_ENDPOINT: &str = "https://portal.nousresearch.com/api/oauth/token";
pub const INFERENCE_SCOPE: &str = "inference:invoke";

/// Only stable local classifications escape. Never expose upstream prose or secret bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("nous_portal_invalid_input")]
    InvalidInput,
    #[error("nous_portal_invalid_response")]
    InvalidResponse,
    #[error("nous_portal_invalid_access_jwt")]
    InvalidAccessJwt,
    #[error("nous_portal_scope_mismatch")]
    ScopeMismatch,
    #[error("nous_portal_subject_hint_mismatch")]
    SubjectHintMismatch,
    #[error("nous_portal_unsafe_inference_target")]
    UnsafeInferenceTarget,
    #[error("nous_portal_missing_successor")]
    MissingSuccessor,
    #[error("nous_portal_expired")]
    Expired,
    #[error("nous_portal_access_denied")]
    AccessDenied,
    #[error("nous_portal_reauthorization_required")]
    ReauthorizationRequired,
    #[error("nous_portal_upstream_blocked")]
    UpstreamBlocked,
    #[error("nous_portal_temporarily_unavailable")]
    TemporarilyUnavailable,
    #[error("nous_portal_rejected")]
    Rejected,
    #[error("nous_portal_redirect_rejected")]
    RedirectRejected,
    #[error("nous_portal_response_too_large")]
    ResponseTooLarge,
    #[error("nous_portal_transport_failed")]
    Transport,
    #[error("nous_portal_timeout")]
    Timeout,
}

/// The operator supplies a client they are permitted to use; no first-party default is embedded.
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthClient {
    client_id: String,
}
impl OAuthClient {
    pub fn new(client_id: &str) -> Result<Self, OAuthError> {
        if !atom(client_id, 512) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            client_id: client_id.into(),
        })
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    /// Only the durable grant scheduler may call this after reserving permanent consumption evidence.
    pub(crate) fn prepare_refresh(&self, refresh: &str) -> Result<RefreshGrant, OAuthError> {
        if !atom(refresh, 32_768) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(RefreshGrant {
            client: self.clone(),
            refresh: refresh.into(),
        })
    }
}

/// Consumed by one HTTP attempt. Receipt persistence must precede access-token admission.
pub(crate) struct RefreshGrant {
    client: OAuthClient,
    refresh: String,
}
impl RefreshGrant {
    pub(crate) async fn execute(self, http: &rquest::Client) -> Result<ReceivedTokens, OAuthError> {
        let reply = transport::post(
            http,
            "/api/oauth/token",
            &[
                ("grant_type", "refresh_token"),
                ("client_id", self.client.client_id()),
            ],
            Some(&self.refresh),
            transport::BUDGET,
        )
        .await?;
        reply.tokens()
    }
}
pub(crate) fn atom(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}
pub(super) fn unix_now() -> u64 {
    time::OffsetDateTime::now_utc().unix_timestamp().max(0) as u64
}
impl std::fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NousOAuthClient { explicit_client: true }")
    }
}
impl std::fmt::Debug for RefreshGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NousRefreshGrant { material: <redacted> }")
    }
}
#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
#[cfg(all(test, feature = "line-nous-portal-oauth"))]
mod wire_tests;
