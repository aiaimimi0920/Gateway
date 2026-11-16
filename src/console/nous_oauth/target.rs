//! Management authority is a saved provider/revision/credential, never an unverified JWT subject.
use super::{Binding, LoginError};
use crate::{
    error::GatewayError, protocol::nous_portal::business, routing::config::RouteConfigSnapshot,
    state::AppState,
};
use std::sync::Arc;
pub(crate) fn validate_target(
    state: &AppState,
    binding: &Binding,
) -> Result<Arc<RouteConfigSnapshot>, GatewayError> {
    if !cfg!(feature = "line-nous-portal-oauth") {
        return Err(
            GatewayError::conflict("Nous implementation line was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(super::login_error(LoginError::Stopped));
    }
    if state
        .route_config_runtime
        .as_ref()
        .is_none_or(|r| r.coordinator().is_none())
    {
        return Err(
            GatewayError::service_unavailable("Writable console runtime is required")
                .with_code("console_mutation_not_supported"),
        );
    }
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != binding.revision() {
        return Err(GatewayError::conflict(
            "Nous login target revision changed; restart authorization",
        )
        .with_code("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider)
        .ok_or_else(business::invalid)?;
    if provider.auth_mode.as_deref() != Some(business::AUTH_MODE)
        || !provider.explicit_credentials_only
        || !provider.api_key.is_empty()
    {
        return Err(business::invalid());
    }
    let compiled = snapshot
        .get_providers()
        .into_iter()
        .find(|p| p.id == binding.provider)
        .ok_or_else(business::invalid)?;
    business::validate_shape(&compiled.payload, compiled.payload.canonical_adapter())?;
    if let Some(id) = &binding.credential {
        let context = provider
            .credentials
            .iter()
            .find(|c| c.id.as_ref() == Some(id))
            .and_then(|c| c.nous_portal.as_ref())
            .ok_or_else(business::invalid)?;
        let credential = compiled
            .credential_pool
            .iter()
            .find(|c| c.id == *id)
            .ok_or_else(business::invalid)?;
        business::validate_context(&credential.payload, context)?;
    }
    Ok(snapshot)
}
