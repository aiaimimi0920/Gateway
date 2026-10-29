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
        .chain(std::iter::once(CircuitKey {
            provider: provider.into(),
            group: "provider:availability".into(),
            model: String::new(),
        }))
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

/// Keep temporary restrictions in the authorized queue; manual flags and balance stay hard.
pub(crate) fn subject_queueable(
    state: &AppState,
    route: &RouteConfigSnapshot,
    provider: &str,
    credential: Option<&str>,
    model: &str,
) -> bool {
    if crate::request_wait::RequestWaitPolicy::for_provider(route, provider).mode
        == crate::request_wait::RequestWaitMode::Disabled
        && !state.lifecycle.model_circuits.network_blocked()
    {
        return subject_allowed(state, route, provider, credential, model);
    }
    // A gateway-wide outage is not a missing provider. Retain eligible subjects
    // through authorization so send admission owns the exact network error:
    // disabled waiting fails immediately; enabled waiting uses the original budget.
    // check_candidate and CircuitAttempt still forbid every actual send.
    !state
        .lifecycle
        .model_circuits
        .nonrecoverable_blocked(&keys(route, provider, credential, model))
        && crate::site_health::runtime::subject_allowed(state, route, provider, credential, model)
}
pub(crate) fn candidate_queueable(
    state: &AppState,
    candidate: &RouteCandidate,
    model: &str,
) -> bool {
    subject_queueable(
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
    if let Some(error) = state.lifecycle.model_circuits.network_error() {
        return Err(error);
    }
    let route = state.route_config.snapshot();
    let credential = candidate
        .provider_credential_id
        .as_deref()
        .or(candidate.payload.credential_id.as_deref());
    let (code, message) = if state.lifecycle.model_circuits.nonrecoverable_blocked(&keys(
        &route,
        &candidate.provider_account_id,
        credential,
        model,
    )) {
        (
            "upstream_balance_unavailable",
            "Upstream account balance is unavailable",
        )
    } else if !crate::site_health::runtime::subject_allowed(
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
    CircuitAttempt::scoped(
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
        Some(CircuitKey {
            provider: candidate.provider_account_id.clone(),
            group: candidate
                .provider_credential_id
                .as_deref()
                .or(candidate.payload.credential_id.as_deref())
                .map_or_else(
                    || format!("{}::default", candidate.provider_account_id),
                    str::to_owned,
                ),
            model: model.into(),
        }),
    )
}
