//! Serialize runtime health reset/undo, while keeping route configuration and quota separate.
mod reset;
use crate::{
    db::{
        self,
        provider_credential_model_states::control::{self, HealthRow},
    },
    error::GatewayError,
    state::AppState,
};
use parking_lot::Mutex;
use std::{collections::HashMap, time::Instant};

#[derive(Debug, Default)]
pub(crate) struct HealthControl {
    pub publication: tokio::sync::Mutex<()>,
    pub gate: tokio::sync::Mutex<Journal>,
    reset_at: Mutex<HashMap<String, Instant>>,
}
#[derive(Debug, Default)]
pub(crate) struct Journal {
    epoch: Option<u64>,
    rows: HashMap<String, (Option<HealthRow>, HealthRow)>,
}
impl HealthControl {
    pub(crate) fn feedback_allowed(&self, provider: &str, started: Instant) -> bool {
        self.reset_at
            .lock()
            .get(provider)
            .is_none_or(|at| started > *at)
    }
}
pub(crate) fn is_transport_message(message: &str) -> bool {
    [
        "Network error:",
        "Connection error:",
        "Request timed out:",
        "Upstream request timed out",
        "Native WebSocket connection ",
        "stream terminated before completion",
        "Model response was interrupted.",
        "Search response was interrupted.",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}
pub(crate) fn transient_failure(input: &db::RecordCredentialModelFailureInput) -> bool {
    matches!(
        input.classification.class,
        crate::provider_failure::ProviderFailureClass::ProviderTransient
            | crate::provider_failure::ProviderFailureClass::Unknown
    ) && !input.classification.permanent
        && (input.upstream_status.is_none()
            || input
                .error_message
                .as_deref()
                .is_some_and(is_transport_message))
}
async fn read(state: &AppState, id: &str) -> Result<Option<HealthRow>, GatewayError> {
    if let Some(local) = &state.local_runtime {
        local.read_health_row(id).await
    } else if let Some(pool) = &state.pg_pool {
        control::read(pool, id).await
    } else {
        Ok(None)
    }
}

pub(crate) async fn record_failure(
    state: &AppState,
    mut input: db::RecordCredentialModelFailureInput,
    started: Option<Instant>,
) -> Result<(), GatewayError> {
    let mut journal = state.lifecycle.health_control.gate.lock().await;
    if let Some(local) = &state.local_runtime {
        input.provider_account_id = local
            .canonical_model_provider(&input.provider_account_id)
            .await?;
    }
    if started.is_some_and(|at| {
        !state
            .lifecycle
            .health_control
            .feedback_allowed(&input.provider_account_id, at)
    }) {
        return Ok(());
    }
    let document = state.route_config.snapshot();
    if document
        .document()
        .providers
        .iter()
        .any(|p| p.id == input.provider_account_id && !reset::dispatch_enabled(p))
    {
        return Ok(());
    }
    // Confirmed authentication, model and balance refusals are independent of egress health.
    let transient = transient_failure(&input);
    if transient
        && started.is_some_and(|at| !state.lifecycle.model_circuits.network_feedback_allowed(at))
    {
        return Ok(());
    }
    let epoch = state
        .lifecycle
        .model_circuits
        .network_epoch()
        .filter(|_| transient);
    if journal.epoch != epoch && epoch.is_some() {
        journal.rows.clear();
        journal.epoch = epoch;
    }
    let id = db::provider_credential_model_state_id(
        &input.provider_account_id,
        input.provider_credential_id.as_deref(),
        input.provider_credential_ref.as_deref(),
        input.protocol_profile.as_deref(),
        &input.model,
    );
    let before = if epoch.is_some() {
        read(state, &id).await?
    } else {
        None
    };
    if epoch.is_some() && journal.rows.len() >= 8192 && !journal.rows.contains_key(&id) {
        return Err(GatewayError::service_unavailable(
            "Egress health journal is full",
        ));
    }
    if epoch.is_some() {
        let after = if let Some(local) = &state.local_runtime {
            local.record_window_failure(input, before.as_ref()).await?
        } else if let Some(pool) = &state.pg_pool {
            control::record_window_failure(pool, input, before.as_ref()).await?
        } else {
            None
        };
        if let Some(after) = after {
            let checkpoint = journal
                .rows
                .entry(id)
                .or_insert_with(|| (before.clone(), after.clone()));
            if before.as_ref().map(|row| &row.version) != Some(&checkpoint.1.version) {
                // An intervening owner write is a newer baseline, not part of our undo.
                checkpoint.0 = before;
            }
            checkpoint.1 = after;
        }
    } else {
        let changed = if let Some(local) = &state.local_runtime {
            local.model_failure(input).await?.is_some()
        } else if let Some(pool) = &state.pg_pool {
            db::record_provider_credential_model_failure(pool, input)
                .await?
                .is_some()
        } else {
            false
        };
        if changed {
            journal.rows.remove(&id);
        }
    }
    Ok(())
}

pub(crate) async fn complete_probe(
    state: &AppState,
    epoch: u64,
    reachable: bool,
) -> Result<(), GatewayError> {
    let mut journal = state.lifecycle.health_control.gate.lock().await;
    if state.lifecycle.model_circuits.network_epoch() != Some(epoch) {
        return Ok(());
    }
    let unstable = state.lifecycle.model_circuits.network_report()["status"]
        == "temporary_network_fluctuation";
    if reachable && unstable && journal.epoch == Some(epoch) {
        for (before, expected) in journal.rows.values() {
            if let Some(local) = &state.local_runtime {
                local.restore_health_row(expected, before.as_ref()).await?;
            } else if let Some(pool) = &state.pg_pool {
                control::restore(pool, expected, before.as_ref()).await?;
            }
        }
    }
    state
        .lifecycle
        .model_circuits
        .complete_network_probe(epoch, reachable);
    if reachable {
        journal.rows.clear();
        journal.epoch = None;
    }
    Ok(())
}
pub(crate) use reset::apply_provider_toggles;
