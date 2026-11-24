//! Managed Console OAuth login sessions for Amazon Quick (`quick-oauth`).
//! Sessions hold temporary authorization material in memory only until the operator explicitly saves.

mod persistence;
mod registry;
mod save;
mod session;
mod target;

pub use registry::Registry;
pub use session::{LoginStep, Session, SessionStatus, SessionView};
pub(crate) use target::validate_target;

#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    pub(super) owner: String,
    pub(super) provider: String,
    pub(super) revision: String,
    pub(super) credential: Option<String>,
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
    #[error("quick_oauth_login_unavailable")]
    Unavailable,
    #[error("quick_oauth_login_capacity")]
    Capacity,
    #[error("quick_oauth_login_stopped")]
    Stopped,
    #[error(transparent)]
    OAuth(#[from] crate::protocol::quick_oauth::OAuthError),
}

pub(crate) fn login_error(error: LoginError) -> crate::error::GatewayError {
    crate::error::GatewayError::bad_request(error.to_string())
        .with_code("quick_oauth_login_rejected")
}
