//! Recheck the exact saved profile and revision before binding, exchanging and importing.
use super::{Binding, LoginError};
use crate::{error::GatewayError, routing::config::RouteConfigSnapshot, state::AppState};
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
    if snapshot.revision().id() != binding.revision {
        return Err(GatewayError::conflict(
            "Luma login target revision changed; restart authorization",
        )
        .with_code("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider && crate::routing::config::luma_mcp::is_provider(p))
        .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
    if let Some(id) = &binding.credential {
        let context = provider
            .credentials
            .iter()
            .find(|c| c.id.as_ref() == Some(id))
            .and_then(|c| c.luma_oauth.as_ref())
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        // Reauthorization retains registered client binding; no user/team identity is asserted.
        if context.client_id() != binding.client || context.redirect_uri() != binding.redirect {
            return Err(super::login_error(LoginError::Unavailable));
        }
    }
    Ok(snapshot)
}
