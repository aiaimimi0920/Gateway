//! Validate the saved consumer at admission/save; no arbitrary endpoint or new device on reauth.
use super::{legacy, Binding, LoginError};
use crate::{
    error::GatewayError, protocol::kiro_oauth::business, routing::config::RouteConfigSnapshot,
    state::AppState,
};
use std::sync::Arc;

pub(crate) fn validate_target(
    state: &AppState,
    binding: &Binding,
    machine: &str,
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
            "Kiro login target revision changed; restart authorization",
        )
        .with_code("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider && p.preset.as_deref() == Some("kiro"))
        .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
    // Shared auth cannot be attributed to the new account, and clearing it could break other consumers.
    if !provider.api_key.is_empty() || provider.auth_token.is_some() {
        return Err(super::login_error(LoginError::Unavailable));
    }
    let compiled = snapshot
        .get_providers()
        .into_iter()
        .find(|p| p.id == provider.id)
        .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
    let mut is_legacy = false;
    let payload = if let Some(id) = binding.credential.as_deref() {
        let credential = provider
            .credentials
            .iter()
            .find(|c| c.id.as_deref() == Some(id))
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        if credential.grok_oauth.is_some()
            || credential.accio_sgcookie.is_some()
            || credential
                .kiro_oauth
                .as_ref()
                .is_some_and(|c| c.machine_id() != machine)
        {
            return Err(super::login_error(LoginError::Unavailable));
        }
        is_legacy = credential.kiro_oauth.is_none();
        // Shared auth copies cannot be removed without changing sibling consumers.
        if is_legacy
            && provider
                .extra_body
                .keys()
                .any(|name| legacy::retired_extra(name))
        {
            return Err(super::login_error(LoginError::Unavailable));
        }
        &compiled
            .credential_pool
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?
            .payload
    } else {
        &compiled.payload
    };
    if is_legacy {
        legacy::validate(payload, machine)
    } else {
        business::validate_material_payload(payload, machine)
    }
    .map_err(|e| super::login_error(e.into()))?;
    Ok(snapshot)
}
pub(crate) fn resolve_machine(
    state: &AppState,
    binding: &Binding,
    supplied: Option<&str>,
) -> Result<String, GatewayError> {
    let machine = if let Some(id) = binding.credential.as_deref() {
        let snapshot = state.route_config.snapshot();
        let provider = snapshot
            .document()
            .providers
            .iter()
            .find(|p| p.id == binding.provider)
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let credential = provider
            .credentials
            .iter()
            .find(|c| c.id.as_deref() == Some(id))
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let existing = if let Some(context) = &credential.kiro_oauth {
            context.machine_id().to_owned()
        } else {
            let compiled = snapshot
                .get_providers()
                .into_iter()
                .find(|p| p.id == provider.id)
                .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
            let payload = &compiled
                .credential_pool
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| super::login_error(LoginError::Unavailable))?
                .payload;
            legacy::machine(payload).map_err(|e| super::login_error(e.into()))?
        };
        if supplied.is_some_and(|value| {
            crate::protocol::kiro::normalize_machine_id(value).as_deref() != Some(existing.as_str())
        }) {
            return Err(super::login_error(LoginError::Unavailable));
        }
        existing
    } else {
        supplied
            .and_then(crate::protocol::kiro::normalize_machine_id)
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?
    };
    validate_target(state, binding, &machine)?;
    Ok(machine)
}
