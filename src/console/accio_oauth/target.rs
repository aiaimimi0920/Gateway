//! Validate the real saved consumer, including credential overrides, before exchanging or saving.
use super::{Binding, LoginError};
use crate::{
    error::GatewayError,
    protocol::accio_oauth::GATEWAY,
    routing::{candidate::ProviderAccountPayload, config::RouteConfigSnapshot},
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
            "Accio login target revision changed; restart authorization",
        )
        .with_code("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider && p.preset.as_deref() == Some("accio"))
        .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
    // Removing credential overrides would reveal these shared values at the real merge owner.
    // A new authorization cannot claim they belong to the same account. Do not clear the provider:
    // its other consumers may still depend on them; reject before exchanging any code instead.
    if provider.auth_token.is_some()
        || provider.headers.keys().any(|key| {
            super::persistence::auth_header(key)
                || matches!(
                    key.to_ascii_lowercase().as_str(),
                    "utdid" | "x-utdid" | "cna" | "x-cna"
                )
        })
        || provider
            .extra_body
            .keys()
            .any(|key| super::persistence::auth_extra(key))
    {
        return Err(super::login_error(LoginError::Unavailable));
    }
    let compiled = snapshot
        .get_providers()
        .into_iter()
        .find(|p| p.id == provider.id)
        .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
    let payload = if let Some(id) = binding.credential.as_deref() {
        // Existence is checked by stable ID, not an array position, display name or token claim.
        let credential = provider
            .credentials
            .iter()
            .find(|c| c.id.as_deref() == Some(id))
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        if credential.grok_oauth.is_some() {
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
    if !valid_payload(payload) {
        return Err(super::login_error(LoginError::Unavailable));
    }
    Ok(snapshot)
}
fn valid_payload(payload: &ProviderAccountPayload) -> bool {
    payload.adapter == "accio_compatible"
        && url::Url::parse(&payload.base_url).ok() == url::Url::parse(GATEWAY).ok()
        && payload.responses_path.as_deref() == Some("/api/adk/llm/generateContent")
}
pub(super) fn validate_utdid(value: &str) -> Result<(), LoginError> {
    if value.is_empty() || value.len() > 1024 || !value.bytes().all(|b| (0x21..=0x7e).contains(&b))
    {
        return Err(LoginError::Unavailable);
    }
    Ok(())
}
pub(crate) fn resolve_utdid(
    state: &AppState,
    binding: &Binding,
    supplied: Option<&str>,
) -> Result<String, GatewayError> {
    let snapshot = validate_target(state, binding)?;
    let value = if let Some(value) = supplied {
        value.to_owned()
    } else {
        let credential = snapshot
            .document()
            .providers
            .iter()
            .find(|p| p.id == binding.provider)
            .and_then(|p| {
                p.credentials
                    .iter()
                    .find(|c| c.id.as_deref() == binding.credential.as_deref() && c.id.is_some())
            })
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let first = credential.headers.get("utdid");
        let second = credential.headers.get("x-utdid");
        if first.zip(second).is_some_and(|(a, b)| a != b) {
            return Err(super::login_error(LoginError::Unavailable));
        }
        first
            .or(second)
            .cloned()
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?
    };
    validate_utdid(&value).map_err(super::login_error)?;
    Ok(value)
}
