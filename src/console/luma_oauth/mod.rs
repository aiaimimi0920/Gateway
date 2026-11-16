//! Temporary Luma authorization owns no storage authority; explicit import uses normal secret/CAS admission.
mod exchange;
mod listener;
mod persistence;
mod refresh;
pub(crate) mod refresh_publication;
mod refresh_registry;
mod registration;
mod registration_registry;
mod registration_storage;
mod registry;
pub use registration::{RegistrationStatus, RegistrationView};
mod save;
mod session;
mod target;
use crate::protocol::luma_oauth::OAuthError;
pub use registry::Registry;
pub use session::{Session, SessionStatus, SessionView};
pub(crate) use target::validate_target;

#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    owner: String,
    provider: String,
    revision: String,
    credential: Option<String>,
    client: String,
    redirect: String,
    offline_access: bool,
}
impl Binding {
    pub fn new(
        owner: &str,
        provider: &str,
        revision: &str,
        client: &str,
        redirect: &str,
    ) -> Result<Self, LoginError> {
        for (value, max) in [
            (owner, 4096),
            (provider, 256),
            (revision, 256),
            (client, 512),
        ] {
            if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
                return Err(LoginError::Unavailable);
            }
        }
        crate::protocol::luma_oauth::Authorization::callback_port(redirect)?;
        Ok(Self {
            owner: owner.into(),
            provider: provider.into(),
            revision: revision.into(),
            credential: None,
            client: client.into(),
            redirect: redirect.into(),
            offline_access: false,
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
    pub fn with_offline_access(mut self) -> Self {
        self.offline_access = true;
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("luma_login_unavailable")]
    Unavailable,
    #[error("luma_login_capacity")]
    Capacity,
    #[error("luma_login_stopped")]
    Stopped,
    #[error("luma_login_listener_unavailable")]
    Listener,
    #[error(transparent)]
    OAuth(#[from] OAuthError),
}
pub(crate) fn login_error(error: LoginError) -> crate::error::GatewayError {
    crate::error::GatewayError::bad_request(error.to_string()).with_code("luma_login_rejected")
}
impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LumaLoginBinding { context: <redacted> }")
    }
}

#[cfg(test)]
mod tests;
