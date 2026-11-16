//! Accio management capabilities; normal secret admission and configuration runtime own persistence.
mod persistence;
mod registry;
mod save;
mod session;
mod target;
use crate::protocol::accio_oauth::OAuthError;
pub use registry::Registry;
pub use session::{Session, SessionStatus, SessionView};
pub(crate) use target::{resolve_utdid, validate_target};

/// Authenticated management owner and saved target, never an inferred Accio account identity.
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
        f.write_str("AccioLoginBinding { context: <redacted> }")
    }
}
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("accio_login_unavailable")]
    Unavailable,
    #[error("accio_login_capacity")]
    Capacity,
    #[error("accio_login_stopped")]
    Stopped,
    #[error(transparent)]
    OAuth(#[from] OAuthError),
}
pub(crate) fn login_error(error: LoginError) -> crate::error::GatewayError {
    crate::error::GatewayError::bad_request(error.to_string()).with_code("accio_login_rejected")
}

#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
mod save_tests;
#[cfg(test)]
mod tests;
