//! Initial device grants remain temporary until explicit save; refresh consumption has its own durable owner.
mod persistence;
mod polling;
mod registry;
mod save;
mod session;
mod target;
pub use registry::Registry;
pub use session::{Session, SessionStatus, SessionView};
pub(crate) use target::validate_target;

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
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("nous_login_unavailable")]
    Unavailable,
    #[error("nous_login_capacity")]
    Capacity,
    #[error("nous_login_stopped")]
    Stopped,
    #[error(transparent)]
    OAuth(#[from] crate::protocol::nous_portal::OAuthError),
}
pub(crate) fn login_error(error: LoginError) -> crate::error::GatewayError {
    crate::error::GatewayError::bad_request(error.to_string()).with_code("nous_login_rejected")
}
