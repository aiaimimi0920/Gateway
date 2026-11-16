//! LobsterAI browser-session protocol. No desktop credential import or fabricated OAuth expiry.
mod authorization;
mod catalog;
pub(crate) mod dispatch;
mod login;
pub(crate) mod messages;
mod messages_stream;
#[cfg(test)]
mod messages_tests;
pub(crate) mod metadata;
mod probe_admission;
mod receipt;
mod received;
mod response;
pub(crate) mod runtime;
mod session;
pub(crate) mod stream;
pub(crate) mod usage;
mod validation;
pub(crate) mod virtual_model;
pub(crate) use validation::{invalid, owns_payload, response_error, stream_error};
pub(crate) mod transport;

pub use authorization::{Authorization, CodeGrant};
pub use catalog::{DeclaredModel, ModelCatalog};
pub mod management_boundary;
pub use login::PendingLogin;
pub use receipt::{ReceivedSession, RefreshGrant, SessionTokens};
pub use received::ReceivedTokens;
pub use session::{AuthorizationContext, SessionMaterial};
pub const TOKEN_ENDPOINT: &str = "https://lobsterai-server.youdao.com/api/auth/refresh";
pub const AUTH_MODE: &str = "lobsterai_session";
pub const CLIENT_ATTRIBUTION: &str = "neuro-gateway";

pub const API_ORIGIN: &str = "https://lobsterai-server.youdao.com";
pub const LOGIN_URL: &str = "https://lobsterai.youdao.com/portal#/login";

/// Attribution describes this Gateway installation, never a borrowed desktop installation.
#[derive(Clone)]
pub struct Client {
    installation_id: uuid::Uuid,
    http: rquest::Client,
}
impl Client {
    pub fn new(
        installation_id: uuid::Uuid,
        builder: rquest::ClientBuilder,
    ) -> Result<Self, LoginError> {
        if installation_id.is_nil() {
            return Err(LoginError::InvalidInput);
        }
        // A prebuilt client could carry an implicit POST retry policy. Build our transport
        // with caller network settings but an authoritative no-retry policy.
        let http = builder
            .retry(rquest::retry::Policy::never())
            .build()
            .map_err(|_| LoginError::Transport)?;
        Ok(Self {
            installation_id,
            http,
        })
    }
    pub(super) fn attribution(&self) -> serde_json::Value {
        serde_json::json!({
            "firstKeyfrom":"neuro-gateway", "latestKeyfrom":"neuro-gateway",
            "uuid":self.installation_id.to_string(),
            "version":concat!("neuro-gateway/", env!("CARGO_PKG_VERSION"))
        })
    }
}

/// Only classified local errors escape; upstream messages may contain credentials.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LoginError {
    #[error("lobsterai_invalid_input")]
    InvalidInput,
    #[error("lobsterai_invalid_callback")]
    InvalidCallback,
    #[error("lobsterai_state_mismatch")]
    StateMismatch,
    #[error("lobsterai_authorization_expired")]
    Expired,
    #[error("lobsterai_entropy_unavailable")]
    Entropy,
    #[error("lobsterai_listener_unavailable")]
    Listener,
    #[error("lobsterai_receipt_consumed")]
    ReceiptConsumed,
    #[error("lobsterai_invalid_response")]
    InvalidResponse,
    #[error("lobsterai_response_too_large")]
    ResponseTooLarge,
    #[error("lobsterai_account_context_required")]
    AccountContextRequired,
    #[error("lobsterai_enterprise_unsupported")]
    EnterpriseUnsupported,
    #[error("lobsterai_identity_mismatch")]
    IdentityMismatch,
    #[error("lobsterai_reauthorization_required")]
    ReauthorizationRequired,
    #[error("lobsterai_rejected")]
    Rejected,
    #[error("lobsterai_redirect_rejected")]
    RedirectRejected,
    #[error("lobsterai_timeout")]
    Timeout,
    #[error("lobsterai_transport_failed")]
    Transport,
}
pub(super) fn atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

#[cfg(test)]
mod inference_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod wire_tests;

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterClient { identity: <redacted>, retries: disabled }")
    }
}
