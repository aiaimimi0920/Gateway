//! Local writable configuration is the authority; browser identity never grants management access.
use super::{Binding, LoginError};
use crate::{
    error::GatewayError,
    protocol::lobsterai::{Client, SessionMaterial, SessionTokens, AUTH_MODE},
    routing::config::RouteConfigSnapshot,
    state::AppState,
};
use std::sync::Arc;
pub(crate) fn validate_target(
    state: &AppState,
    binding: &Binding,
) -> Result<Arc<RouteConfigSnapshot>, GatewayError> {
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(super::login_error(LoginError::Stopped));
    }
    if state.config.storage_mode != crate::config::GatewayStorageMode::Local
        || state.pg_pool.is_some()
        || state
            .route_config_runtime
            .as_ref()
            .is_none_or(|r| r.coordinator().is_none())
    {
        return Err(GatewayError::service_unavailable(
            "LobsterAI authorization requires a writable local runtime",
        )
        .with_code("lobsterai_local_runtime_required"));
    }
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != binding.revision() {
        return Err(GatewayError::conflict(
            "LobsterAI login target changed; restart authorization",
        )
        .with_code("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider)
        .ok_or_else(invalid)?;
    if provider.auth_mode.as_deref() != Some(AUTH_MODE)
        || provider.base_url != crate::protocol::lobsterai::management_boundary::BASE
        || !provider.explicit_credentials_only
    {
        return Err(invalid());
    }
    if let Some(id) = &binding.credential {
        let credential = provider
            .credentials
            .iter()
            .find(|c| c.id.as_ref() == Some(id))
            .ok_or_else(invalid)?;
        let context = credential.lobsterai.as_ref().ok_or_else(invalid)?;
        context
            .validate_binding(
                credential.api_key.as_deref().unwrap_or_default(),
                credential.refresh_token.as_deref().unwrap_or_default(),
            )
            .map_err(|_| invalid())?;
    }
    Ok(snapshot)
}
pub(super) fn client(
    snapshot: &RouteConfigSnapshot,
    binding: &Binding,
) -> Result<Client, GatewayError> {
    let installation = binding
        .credential
        .as_ref()
        .and_then(|id| {
            snapshot
                .document()
                .providers
                .iter()
                .find(|p| p.id == binding.provider)?
                .credentials
                .iter()
                .find(|c| c.id.as_ref() == Some(id))?
                .lobsterai
                .as_ref()
                .map(|c| c.installation_id())
        })
        .unwrap_or_else(uuid::Uuid::new_v4);
    Client::new(installation, rquest::Client::builder()).map_err(|e| super::login_error(e.into()))
}
pub(crate) fn stored(
    credential: &crate::routing::config::ProviderCredentialYaml,
) -> Result<SessionTokens, GatewayError> {
    let context = credential.lobsterai.clone().ok_or_else(invalid)?;
    let client = Client::new(context.installation_id(), rquest::Client::builder())
        .map_err(|e| super::login_error(e.into()))?;
    SessionTokens::restore(
        client,
        SessionMaterial {
            context,
            access_token: credential.api_key.clone().ok_or_else(invalid)?,
            refresh_token: credential.refresh_token.clone().ok_or_else(invalid)?,
        },
    )
    .map_err(|e| super::login_error(e.into()))
}
fn invalid() -> GatewayError {
    super::login_error(LoginError::Unavailable)
}
