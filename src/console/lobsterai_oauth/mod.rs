//! Browser sessions stay temporary until explicit save; the durable refresh owner remains separate.
mod catalog_save;
mod exchange;
mod persistence;
mod registry;
mod save;
mod session;
mod target;
pub use registry::Registry;
pub use session::{Session, SessionStatus, SessionView};
pub(crate) use target::stored;
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
    #[error("lobsterai_login_unavailable")]
    Unavailable,
    #[error("lobsterai_login_capacity")]
    Capacity,
    #[error("lobsterai_login_stopped")]
    Stopped,
    #[error(transparent)]
    OAuth(#[from] crate::protocol::lobsterai::LoginError),
}
pub(crate) fn login_error(error: LoginError) -> crate::error::GatewayError {
    crate::error::GatewayError::bad_request(error.to_string()).with_code("lobsterai_login_rejected")
}
