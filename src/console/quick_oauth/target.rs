//! Validates writable local Console authority and target provider invariants for `quick-oauth`.

use super::{Binding, LoginError};
use crate::{
    error::GatewayError,
    protocol::quick_oauth::{AUTH_MODE, PORTAL_BASE, PRESET_ID},
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
            "Amazon Quick OAuth authorization requires a writable local runtime",
        )
        .with_code("quick_oauth_local_runtime_required"));
    }
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != binding.revision() {
        return Err(GatewayError::conflict(
            "Amazon Quick login target changed; restart authorization",
        )
        .with_code("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider)
        .ok_or_else(invalid)?;
    if provider.preset.as_deref() != Some(PRESET_ID)
        || (provider.auth_mode.is_some() && provider.auth_mode.as_deref() != Some(AUTH_MODE))
        || provider.base_url.trim_end_matches('/') != PORTAL_BASE
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
        let context = credential.quick_oauth.as_ref().ok_or_else(invalid)?;
        context.validate_metadata().map_err(|_| invalid())?;
    }
    Ok(snapshot)
}

fn invalid() -> GatewayError {
    super::login_error(LoginError::Unavailable)
}
