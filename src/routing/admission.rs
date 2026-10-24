//! Hard admission precedes pool selection and soft ranking. It never changes manual enable flags.
use super::{
    candidate::RouteCandidate,
    config::RouteConfigSnapshot,
    model_circuit::{CircuitAttempt, CircuitKey},
};
use crate::state::AppState;
use std::sync::Arc;

pub(crate) fn keys(
    route: &RouteConfigSnapshot,
    provider: &str,
    credential: Option<&str>,
    model: &str,
) -> Vec<CircuitKey> {
    let default = format!("{provider}::default");
    let subject = credential.unwrap_or(&default);
    let mut groups: Vec<_> = route
        .effective_account_groups()
        .iter()
        .filter(|g| g.provider_credential_ids.iter().any(|id| id == subject))
        .map(|g| format!("group:{}", g.id))
        .collect();
    // Database/Redis credentials outside the local group catalogue stay isolated.
    if groups.is_empty() {
        groups.push(credential.map_or_else(|| "default".into(), |id| format!("credential:{id}")));
    }
    // Balance belongs to the concrete credential, across models and entitlement groups.
    // Keep this separate from model/group faults so a funded sibling stays available.
    let balance = CircuitKey {
        provider: provider.into(),
        group: format!("balance:{subject}"),
        model: String::new(),
    };
    groups
        .into_iter()
        .map(|group| CircuitKey {
            provider: provider.into(),
            group,
            model: model.into(),
        })
        .chain(std::iter::once(balance))
        .collect()
}

pub(crate) fn subject_allowed(
    state: &AppState,
    route: &RouteConfigSnapshot,
    provider: &str,
    credential: Option<&str>,
    model: &str,
) -> bool {
    keys(route, provider, credential, model)
        .iter()
        .all(|key| state.lifecycle.model_circuits.available(key))
        && crate::site_health::runtime::subject_allowed(state, route, provider, credential, model)
}

pub(crate) fn candidate_allowed(state: &AppState, candidate: &RouteCandidate, model: &str) -> bool {
    subject_allowed(
        state,
        &state.route_config.snapshot(),
        &candidate.provider_account_id,
        candidate
            .provider_credential_id
            .as_deref()
            .or(candidate.payload.credential_id.as_deref()),
        model,
    )
}

pub(crate) fn check_candidate(
    state: &AppState,
    candidate: &RouteCandidate,
    model: &str,
) -> Result<(), crate::error::GatewayError> {
    let route = state.route_config.snapshot();
    let credential = candidate
        .provider_credential_id
        .as_deref()
        .or(candidate.payload.credential_id.as_deref());
    let (code, message) = if !crate::site_health::runtime::subject_allowed(
        state,
        &route,
        &candidate.provider_account_id,
        credential,
        model,
    ) {
        (
            "site_health_route_restricted",
            "Site health policy restricted this upstream model",
        )
    } else if keys(&route, &candidate.provider_account_id, credential, model)
        .iter()
        .any(|key| !state.lifecycle.model_circuits.available(key))
    {
        (
            "model_circuit_open",
            "Model is temporarily disabled by failure circuit",
        )
    } else {
        return Ok(());
    };
    let mut error = crate::error::GatewayError::service_unavailable(message).with_code(code);
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::FallbackProvider {
        reason: code.into(),
    };
    Err(error)
}

pub(crate) fn attempt(
    state: &AppState,
    candidate: &RouteCandidate,
    model: &str,
) -> Arc<CircuitAttempt> {
    CircuitAttempt::new(
        Arc::clone(&state.lifecycle.model_circuits),
        keys(
            &state.route_config.snapshot(),
            &candidate.provider_account_id,
            candidate
                .provider_credential_id
                .as_deref()
                .or(candidate.payload.credential_id.as_deref()),
            model,
        ),
    )
}
