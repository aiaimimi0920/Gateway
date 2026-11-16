//! CLI browser approval under ordinary management authority, with SecretPatch/CAS-owned persistence.
mod operations;
mod persistence;
mod registry;
mod save;
mod session;
mod target;
pub use registry::Registry;
pub use session::{Session, SessionStatus, SessionView};
pub(crate) use target::{resolve_fingerprint, validate_target};

#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    owner: String,
    provider: String,
    revision: String,
    credential: Option<String>,
}
impl Binding {
    pub fn new(owner: &str, provider: &str, revision: &str) -> Result<Self, LoginError> {
        for (value, max) in [(owner, 4096), (provider, 256), (revision, 256)] {
            if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
                return Err(LoginError::Unavailable);
            }
        }
        Ok(Self {
            owner: owner.into(),
            provider: provider.into(),
            revision: revision.into(),
            credential: None,
        })
    }
    pub fn with_credential(mut self, id: &str) -> Result<Self, LoginError> {
        if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
            return Err(LoginError::Unavailable);
        }
        self.credential = Some(id.into());
        Ok(self)
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
}
impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffLoginBinding { context: <redacted> }")
    }
}
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("freebuff_login_unavailable")]
    Unavailable,
    #[error("freebuff_login_capacity")]
    Capacity,
    #[error("freebuff_login_stopped")]
    Stopped,
    #[error(transparent)]
    Protocol(#[from] crate::protocol::freebuff_login::LoginError),
}
pub(crate) fn login_error(error: LoginError) -> crate::error::GatewayError {
    crate::error::GatewayError::bad_request(error.to_string()).with_code("freebuff_login_rejected")
}

#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod local_tests;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod tests;

#[cfg(all(test, not(feature = "line-freebuff-web-reverse-api")))]
mod disabled_tests {
    use super::*;
    #[tokio::test]
    async fn disabled_freebuff_management_cannot_create_an_enabled_login() {
        let registry = Registry::default();
        let binding = Binding::new("owner", "provider", "revision").unwrap();
        let result = registry.start(
            binding,
            crate::protocol::freebuff_login::LoginTransport::new().unwrap(),
            "device".into(),
        );
        assert!(matches!(
            result,
            Err(LoginError::Protocol(
                crate::protocol::freebuff_login::LoginError::FeatureDisabled
            ))
        ));
        registry.join().await;
    }
}
