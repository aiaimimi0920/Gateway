//! Fixed bounded worker; the current database step completes or times out before join.
use super::*;
use crate::state::AppState;
use std::{sync::Arc, time::Duration};

const PERIOD: Duration = Duration::from_secs(10);
const STEP_BUDGET: Duration = Duration::from_secs(5);

pub(crate) fn spawn(state: &Arc<AppState>) {
    if Store::from_state(state).is_none() || state.lifecycle.is_draining() {
        return;
    }
    let weak = Arc::downgrade(state);
    let stop = state.shutdown.clone();
    state.lifecycle.background_tasks.spawn(
        crate::runtime::background_tasks::Worker::SapResponseRecovery,
        &state.shutdown,
        async move {
            let mut interval = tokio::time::interval(PERIOD);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut cursor = String::new();
            loop {
                tokio::select! {
                    biased;
                    _ = stop.wait() => break,
                    _ = interval.tick() => {},
                }
                let Some(state) = weak.upgrade() else { break };
                if state.lifecycle.is_draining() || stop.is_requested() {
                    break;
                }
                if let Err(error) = tick(&state, &mut cursor).await {
                    tracing::warn!(code = ?error.code, "SAP ledger recovery pass unavailable");
                }
            }
        },
    );
}

pub(in crate::protocol::sap_aicore::responses_lifecycle::store) async fn tick(
    state: &AppState,
    cursor: &mut String,
) -> Result<(), GatewayError> {
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Ok(());
    }
    let Some(store) = Store::from_state(state) else {
        return Ok(());
    };
    let rows = bounded(store.recoverable(cursor)).await?;
    let full = rows.len() == PAGE;
    for row in rows {
        if state.shutdown.is_requested() || state.lifecycle.is_draining() {
            return Ok(());
        }
        // Advance even on a bad row; a full pass revisits it rather than starving later IDs.
        *cursor = row.id.clone();
        match bounded(store.recover(&row)).await {
            Ok(true) => tracing::info!(response_id = %row.id, "SAP local ledger recovered"),
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(response_id = %row.id, code = ?error.code, "SAP local ledger recovery deferred")
            }
        }
    }
    if !full {
        cursor.clear();
    }
    Ok(())
}

pub(super) async fn bounded<T>(
    work: impl std::future::Future<Output = Result<T, GatewayError>>,
) -> Result<T, GatewayError> {
    // Cancellation here is only of a database transaction. Commit uncertainty is safe to
    // revisit through the durable hold and receipt checks; there is never an upstream call.
    tokio::time::timeout(STEP_BUDGET, work).await.map_err(|_| {
        GatewayError::service_unavailable("SAP ledger recovery database step timed out.")
            .with_code("sap_aicore_response_recovery_timeout")
    })?
}
