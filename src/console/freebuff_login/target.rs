//! Recheck exact saved product and revision. An approval capability never chooses the business destination.
use super::{Binding, LoginError};
use crate::{
    error::GatewayError,
    routing::config::{freebuff_session, RouteConfigSnapshot},
    state::AppState,
};
use std::sync::Arc;

pub(crate) fn validate_target(
    state: &AppState,
    binding: &Binding,
    fingerprint: &str,
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
            "FreeBuff login target revision changed; restart approval",
        )
        .with_code("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider)
        .ok_or_else(unavailable)?;
    let compiled = snapshot
        .get_providers()
        .into_iter()
        .find(|p| p.id == provider.id)
        .ok_or_else(unavailable)?;
    let (credential, payload) = if let Some(id) = binding.credential.as_deref() {
        let c = provider
            .credentials
            .iter()
            .find(|c| c.id.as_deref() == Some(id))
            .ok_or_else(unavailable)?;
        if c.freebuff_session
            .as_ref()
            .and_then(|c| c.fingerprint_id())
            .is_some_and(|id| id != fingerprint)
        {
            return Err(unavailable());
        }
        let payload = &compiled
            .credential_pool
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(unavailable)?
            .payload;
        (Some(c), payload)
    } else {
        (None, &compiled.payload)
    };
    freebuff_session::validate_target(provider, credential, payload)
        .map_err(|e| super::login_error(e.into()))?;
    Ok(snapshot)
}
pub(crate) fn resolve_fingerprint(
    state: &AppState,
    binding: &Binding,
    supplied: Option<&str>,
) -> Result<String, GatewayError> {
    let snapshot = state.route_config.snapshot();
    let existing = if let Some(id) = binding.credential.as_deref() {
        let credential = snapshot
            .document()
            .providers
            .iter()
            .find(|p| p.id == binding.provider)
            .and_then(|p| p.credentials.iter().find(|c| c.id.as_deref() == Some(id)))
            .ok_or_else(unavailable)?;
        credential
            .freebuff_session
            .as_ref()
            .and_then(|c| c.fingerprint_id())
    } else {
        None
    };
    if existing.is_some_and(|existing| supplied.is_some_and(|s| s != existing)) {
        return Err(unavailable());
    }
    let fingerprint = existing
        .or(supplied)
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if !crate::protocol::freebuff_login::atom(&fingerprint, 512) {
        return Err(unavailable());
    }
    validate_target(state, binding, &fingerprint)?;
    Ok(fingerprint)
}
fn unavailable() -> GatewayError {
    super::login_error(LoginError::Unavailable)
}
