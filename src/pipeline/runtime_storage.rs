//! Ephemeral routing and server reporting adapters. Local usage is persisted by request audits.
use crate::state::AppState;

pub(super) async fn get_affinity(state: &AppState, scope: &str, model: &str) -> Option<String> {
    if let Some(local) = &state.local_runtime {
        return local.credential_affinity(scope, model);
    }
    crate::redis::credential_cache::get_credential_affinity(&state.redis_pool, scope, model)
        .await
        .ok()
        .flatten()
}

pub(super) async fn set_affinity(state: &AppState, scope: &str, model: &str, id: &str) {
    if let Some(local) = &state.local_runtime {
        local.set_credential_affinity(scope, model, id);
    } else {
        let _ = crate::redis::credential_cache::set_credential_affinity(
            &state.redis_pool,
            scope,
            model,
            id,
        )
        .await;
    }
}

pub(super) async fn publish_usage(
    state: &AppState,
    report: &crate::redis::usage_tracking::UsageReport,
) -> anyhow::Result<()> {
    if state.local_runtime.is_none() {
        crate::redis::usage_tracking::enqueue_usage_report(&state.redis_pool, report).await?;
    }
    Ok(())
}
