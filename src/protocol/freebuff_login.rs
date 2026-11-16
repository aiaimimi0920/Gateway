//! FreeBuff CLI browser approval, not an OAuth grant, API key issuer or management authority.
//! The fixed public reference and stricter local admission rules are in freebuff_login/REFERENCE.md.
mod challenge;
mod context;
mod material;
mod transport;
pub use challenge::{LoginChallenge, LoginPoll};
pub use context::SessionContext;
pub use material::Credentials;
pub use transport::LoginTransport;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LoginError {
    #[error("freebuff_login_invalid_input")]
    InvalidInput,
    #[error("freebuff_login_invalid_response")]
    InvalidResponse,
    #[error("freebuff_login_expired")]
    Expired,
    #[error("freebuff_login_cancelled")]
    Cancelled,
    #[error("freebuff_login_consumed")]
    Consumed,
    #[error("freebuff_login_rejected")]
    Rejected,
    #[error("freebuff_login_transport_failed")]
    Transport,
    #[error("freebuff_login_timeout")]
    Timeout,
    #[error("freebuff_login_redirect_rejected")]
    RedirectRejected,
    #[error("freebuff_login_response_too_large")]
    ResponseTooLarge,
    #[error("freebuff_login_feature_disabled")]
    FeatureDisabled,
}

pub(crate) fn atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}
pub(super) fn text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
pub(super) struct Reply {
    status: u16,
    bytes: Vec<u8>,
}
impl Reply {
    fn decode<T: serde::de::DeserializeOwned>(&self) -> Result<T, LoginError> {
        serde_json::from_slice(&self.bytes).map_err(|_| LoginError::InvalidResponse)
    }
}

#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
tokio::task_local! { pub(crate) static TEST_ORIGIN: String; }
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
tokio::task_local! { static TEST_BODY_STARTED: std::sync::Arc<tokio::sync::Notify>; }
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
pub(crate) mod fixture;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod safety_tests;
#[cfg(test)]
mod tests;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod wire_tests;
