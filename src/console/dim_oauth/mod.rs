//! DIM authorization is temporary; normal secret admission and revision CAS own persistence.
mod catalog_save;
mod listener;
mod persistence;
mod registry;
mod save;
mod session;
mod target;
use crate::protocol::dim_oauth::{OAuthError, Site};
pub use registry::Registry;
pub use session::{Session, SessionStatus, SessionView};
pub(crate) use target::validate_target;

#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    owner: String,
    provider: String,
    revision: String,
    credential: Option<String>,
    site: Site,
}
impl Binding {
    pub fn new(
        owner: &str,
        provider: &str,
        revision: &str,
        site: Site,
    ) -> Result<Self, LoginError> {
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
            site,
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
        f.write_str("DimLoginBinding { context: <redacted> }")
    }
}
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("dim_login_feature_disabled")]
    FeatureDisabled,
    #[error("dim_login_unavailable")]
    Unavailable,
    #[error("dim_login_capacity")]
    Capacity,
    #[error("dim_login_stopped")]
    Stopped,
    #[error("dim_login_listener_unavailable")]
    Listener,
    #[error(transparent)]
    OAuth(#[from] OAuthError),
}
pub(crate) fn login_error(error: LoginError) -> crate::error::GatewayError {
    crate::error::GatewayError::bad_request(error.to_string()).with_code("dim_login_rejected")
}
