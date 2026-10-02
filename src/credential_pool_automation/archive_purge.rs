//! The persisted route revision fences stale source removals across all instances.
use crate::{
    credential_pool_storage::purge::ArchivePurgePlan, error::GatewayError, state::AppState,
};
use std::sync::Arc;

pub(crate) async fn purge_for_provider(
    state: &Arc<AppState>,
    provider_id: &str,
) -> Result<usize, GatewayError> {
    let provider_lock = state
        .credential_pool_automation
        .lock_for_provider(provider_id);
    let _guard = provider_lock.lock().await;
    let _admission = super::admission::acquire(state, provider_id).await?;
    // Capture R before listing. A stale replica must lose the authoritative CAS.
    let snapshot = state.route_config.snapshot();
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| GatewayError::not_found("Credential archive provider does not exist"))?;
    let plan = ArchivePurgePlan::capture(&state.config, provider)
        .await
        .map_err(failed)?;
    execute_with_barrier(state, &snapshot, provider_id, plan).await
}

pub(super) async fn execute_with_barrier(
    state: &Arc<AppState>,
    snapshot: &crate::routing::config::RouteConfigSnapshot,
    provider_id: &str,
    plan: ArchivePurgePlan,
) -> Result<usize, GatewayError> {
    if plan.is_empty() {
        return Ok(0);
    }
    let runtime = state.route_config_runtime.as_ref().ok_or_else(|| {
        GatewayError::service_unavailable("Route configuration runtime is unavailable")
    })?;
    // A no-content-change commit still advances the durable sequence. No DELETE
    // is issued on CAS failure or an uncertain response, and this is never rolled back.
    let barrier = runtime
        .commit_automation_document(
            snapshot.revision().id(),
            snapshot.document().clone(),
            Some(format!(
                "credential archive purge barrier for {provider_id}"
            )),
        )
        .await
        .map_err(|_| {
            GatewayError::conflict(
                "Archive purge barrier was not confirmed; no archives were deleted",
            )
        })?;
    if barrier.revision().sequence() <= snapshot.revision().sequence()
        || barrier.revision().id() == snapshot.revision().id()
    {
        return Err(GatewayError::conflict(
            "Archive purge barrier did not advance; no archives were deleted",
        ));
    }
    plan.execute(&|| state.route_config.snapshot().revision().id() == barrier.revision().id())
        .await
        .map_err(failed)
}
fn failed(error: anyhow::Error) -> GatewayError {
    GatewayError::server_error(error.to_string()).with_code("credential_pool_archive_purge_failed")
}
