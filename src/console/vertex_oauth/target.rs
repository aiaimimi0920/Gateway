//! Saved provider, stable credential ID and the original revision are checked before exchange/save.
use super::{Binding, LoginError};
use crate::{
    error::GatewayError,
    protocol::vertex_oauth::{business, VertexTarget},
    routing::config::RouteConfigSnapshot,
    state::AppState,
};
use std::sync::Arc;
pub(crate) fn validate_target(
    state: &AppState,
    binding: &Binding,
    target: &VertexTarget,
) -> Result<Arc<RouteConfigSnapshot>, GatewayError> {
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
    target
        .validate()
        .map_err(|e| super::login_error(e.into()))?;
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != binding.revision() {
        return Err(GatewayError::conflict(
            "Vertex login target revision changed; restart authorization",
        )
        .with_code("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider)
        .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
    if provider.auth_mode.as_deref() != Some(business::AUTH_MODE)
        || !provider.api_key.is_empty()
        || provider.preset.as_deref().is_none_or(|p| {
            !matches!(
                p,
                "google-agent-platform"
                    | "google-agent-platform-official-api"
                    | "vertex-gemini"
                    | "vertex-official-api"
            )
        })
    {
        return Err(super::login_error(LoginError::Unavailable));
    }
    let compiled = snapshot
        .get_providers()
        .into_iter()
        .find(|p| p.id == binding.provider)
        .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
    let payload = if let Some(id) = &binding.credential {
        if !provider
            .credentials
            .iter()
            .any(|c| c.id.as_deref() == Some(id.as_str()))
        {
            return Err(super::login_error(LoginError::Unavailable));
        }
        &compiled
            .credential_pool
            .iter()
            .find(|c| c.id == *id)
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?
            .payload
    } else {
        &compiled.payload
    };
    business::validate(payload, payload.canonical_adapter())?;
    // New accounts capture their own quota project in the credential context. Raw parent
    // headers cannot be inherited: configure() derives that header from this context.
    let matches = if binding.credential.is_some() {
        &business::target(payload)? == target
    } else {
        payload.headers.is_empty()
            && payload.base_url
                == target
                    .base_url()
                    .map_err(|e| super::login_error(e.into()))?
    };
    if !matches {
        return Err(super::login_error(LoginError::Unavailable));
    }
    Ok(snapshot)
}
