//! Explicit account import. Authorization, durable custody and inference capability are separate facts.
mod completion;
mod registry;
mod save;
mod session;
#[cfg(test)]
mod tests;
use crate::{
    console::{AuthenticatedConsoleActor, ConsoleRequestContext},
    error::GatewayError,
    protocol::cursor_auth::AuthError,
    state::AppState,
};
pub(crate) use registry::Registry;
pub(crate) use save::validate_generic_update;
use serde::Deserialize;
pub(crate) use session::{Session, View};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Start {
    pub attempt_id: String,
    pub provider_id: String,
    pub expected_revision: String,
    pub credential_id: Option<String>,
    pub expected_email: Option<String>,
    pub access_token: Option<String>,
}
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Binding {
    owner: String,
    provider: String,
    revision: String,
    credential: Option<String>,
    email: Option<String>,
}
#[derive(Clone)]
pub(crate) struct Consent {
    pub actor: AuthenticatedConsoleActor,
    pub context: ConsoleRequestContext,
    pub grant: String,
}
impl Consent {
    pub(super) fn check(&self, state: &AppState) -> Result<(), GatewayError> {
        state
            .console_auth
            .verify_secret_grant(&self.context, &self.actor, &self.grant)
    }
}
pub(super) fn error(code: &'static str) -> GatewayError {
    GatewayError::bad_request(code).with_code(code)
}
impl From<AuthError> for GatewayError {
    fn from(value: AuthError) -> Self {
        error(value.0)
    }
}
