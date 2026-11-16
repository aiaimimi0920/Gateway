//! Kiro management device capabilities; ordinary SecretPatch/CAS owns durable credentials.
mod legacy;
mod operations;
mod persistence;
mod registry;
mod save;
mod session;
mod target;
use crate::protocol::kiro_oauth::OAuthError;
pub use registry::Registry;
pub use session::{Session, SessionStatus, SessionView};
pub(crate) use target::{resolve_machine, validate_target};

/// Management identity and stable destination, never an identity inferred from opaque OIDC tokens.
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
        f.write_str("KiroLoginBinding { context: <redacted> }")
    }
}
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("kiro_login_unavailable")]
    Unavailable,
    #[error("kiro_login_capacity")]
    Capacity,
    #[error("kiro_login_stopped")]
    Stopped,
    #[error(transparent)]
    OAuth(#[from] OAuthError),
}
pub(crate) fn login_error(error: LoginError) -> crate::error::GatewayError {
    crate::error::GatewayError::bad_request(error.to_string()).with_code("kiro_login_rejected")
}

#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
mod runtime_tests;
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
mod save_tests;
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
mod tests;
