//! Revision, site and stable credential identity are checked before discovery, exchange and save.
use super::{Binding, LoginError};
use crate::{
    error::GatewayError, protocol::dim_oauth::business, routing::config::RouteConfigSnapshot,
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
            "DIM login target revision changed; restart authorization",
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
        || !provider.explicit_credentials_only
        || !provider.api_key.is_empty()
        || provider.base_url.trim_end_matches('/') != business::base(binding.site)
    {
        return Err(super::login_error(LoginError::Unavailable));
    }
    let compiled = snapshot
        .get_providers()
        .into_iter()
        .find(|p| p.id == binding.provider)
        .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
    let payload = if let Some(id) = &binding.credential {
        let saved = provider
            .credentials
            .iter()
            .find(|c| c.id.as_deref() == Some(id.as_str()))
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let context = saved
            .dim_oauth
            .as_ref()
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        if context.site() != binding.site {
            return Err(super::login_error(LoginError::Unavailable));
        }
        let payload = &compiled
            .credential_pool
            .iter()
            .find(|c| c.id == *id)
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?
            .payload;
        business::validate_context(payload, context)?;
        payload
    } else {
        &compiled.payload
    };
    if payload.canonical_adapter() != "openai_compatible"
        || payload.auth_mode.as_deref() != Some(business::AUTH_MODE)
        || payload.base_url.trim_end_matches('/') != business::base(binding.site)
        || !payload.headers.is_empty()
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || !payload.discovered_protocols.is_empty()
    {
        return Err(super::login_error(LoginError::Unavailable));
    }
    Ok(snapshot)
}
