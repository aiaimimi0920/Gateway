//! Per-runtime Grok login ownership; HTTP authorization and normal secret commits remain separate.
mod device;
mod listener;
mod persistence;
mod registry;
mod save;
mod session;
use crate::protocol::grok_oauth::OAuthError;
pub use device::DeviceView;
pub use registry::Registry;
pub use session::{Session, SessionStatus, SessionView};

/// Construct from the authenticated operator/IP/host and the saved provider's exact revision.
/// This binding is not itself management authentication or permission to publish a configuration.
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
    pub fn provider(&self) -> &str {
        &self.provider
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
    /// Reauthorization is explicit, never inferred from an unverified account hint.
    pub fn with_credential(mut self, id: &str) -> Result<Self, LoginError> {
        if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
            return Err(LoginError::Unavailable);
        }
        self.credential = Some(id.into());
        Ok(self)
    }
}
pub(crate) fn login_error(error: LoginError) -> crate::error::GatewayError {
    crate::error::GatewayError::bad_request(error.to_string()).with_code("grok_login_rejected")
}
pub(crate) use persistence::validate_target;
impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrokLoginBinding { context: <redacted> }")
    }
}
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("grok_login_unavailable")]
    Unavailable,
    #[error("grok_login_capacity")]
    Capacity,
    #[error("grok_login_stopped")]
    Stopped,
    #[error("grok_login_listener_unavailable")]
    Listener,
    #[error(transparent)]
    OAuth(#[from] OAuthError),
}
#[cfg(test)]
mod device_tests;
#[cfg(test)]
mod save_tests;
#[cfg(test)]
mod tests;
