//! Archive independently, then publish the successful subset in one existing route-CAS.
use super::*;
use crate::routing::config::ProviderConfigYaml;
use std::time::Duration;

pub(super) async fn commit_selection(
    state: &Arc<AppState>,
    snapshot: &RouteConfigSnapshot,
    provider_id: &str,
    selection: PruneSelection,
    lease: Arc<MaintenanceAdmission>,
) -> Result<PruneReceipt, GatewayError> {
    let runtime = state.route_config_runtime.as_ref().ok_or_else(|| {
        GatewayError::service_unavailable("Route configuration runtime is unavailable")
    })?;
    let mut document = snapshot.document().clone();
    let provider = document
        .providers
        .iter_mut()
        .find(|p| p.id == provider_id)
        .ok_or_else(stale)?;
    // This human-reviewed path always preserves recovery data, including legacy permanent-delete pools.
    let mut archive_provider = provider.clone();
    archive_provider.credential_permanent_delete_enabled = false;
    let archive_provider = Arc::new(archive_provider);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let mut ready = HashSet::new();
    let mut items = Vec::with_capacity(selection.credential_ids.len());
    for id in selection.credential_ids {
        let status = if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
            "retained_revision_changed"
        } else if tokio::time::Instant::now() >= deadline {
            "archive_budget_exhausted"
        } else {
            match tokio::time::timeout_at(
                deadline,
                preserve_one(
                    state,
                    &archive_provider,
                    &id,
                    snapshot.revision().id(),
                    &lease,
                ),
            )
            .await
            {
                Ok(Ok(1)) => {
                    ready.insert(id.clone());
                    "archived_pending_commit"
                }
                _ => "archive_failed",
            }
        };
        items.push(PruneResult {
            credential_id: id,
            status,
        });
    }
    let mut receipt = PruneReceipt {
        provider_id: provider_id.into(),
        source_revision: snapshot.revision().id().into(),
        revision: state.route_config.snapshot().revision().id().into(),
        outcome: "completed",
        removed_count: Some(0),
        items,
    };
    if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
        receipt.outcome = "revision_changed";
        replace_pending(&mut receipt, "retained_revision_changed");
        return Ok(receipt);
    }
    if ready.is_empty() {
        return Ok(receipt);
    }
    let removed = inventory::prune_credentials(provider, &ready);
    if removed != ready.len() {
        return Err(stale());
    }
    match runtime
        .commit_automation_document(
            snapshot.revision().id(),
            document,
            Some(format!(
                "reviewed credential prune: {removed} archived credentials"
            )),
        )
        .await
    {
        Ok(committed) => {
            receipt.revision = committed.revision().id().into();
            receipt.removed_count = Some(removed);
            replace_pending(&mut receipt, "archived_and_removed");
        }
        Err(_) => {
            // A persistence error may be post-commit. Do not encourage automatic destructive replay.
            receipt.outcome = "commit_unconfirmed";
            receipt.removed_count = None;
            replace_pending(&mut receipt, "commit_unconfirmed");
        }
    }
    Ok(receipt)
}

fn replace_pending(receipt: &mut PruneReceipt, status: &'static str) {
    for item in &mut receipt.items {
        if item.status == "archived_pending_commit" {
            item.status = status;
        }
    }
}

async fn preserve_one(
    state: &AppState,
    provider: &Arc<ProviderConfigYaml>,
    id: &str,
    revision: &str,
    lease: &Arc<MaintenanceAdmission>,
) -> anyhow::Result<usize> {
    let ids = HashSet::from([id.to_string()]);
    if provider
        .credential_archive_connection
        .as_ref()
        .is_some_and(|c| c.is_remote())
    {
        return crate::credential_pool_storage::archive::preserve(
            &state.config,
            provider,
            &ids,
            revision,
        )
        .await;
    }
    let config = state.config.clone();
    let provider = Arc::clone(provider);
    // No filesystem work on the async executor. A timed-out write can leave an archive,
    // never a removal: only a completed, verified preservation enters the route-CAS set.
    blocking_preservation(Arc::clone(lease), move || {
        archive::archive_pruned_credentials(&config, &provider, &ids)
    })
    .await
}

async fn blocking_preservation<T: Send + 'static>(
    lease: Arc<MaintenanceAdmission>,
    write: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    tokio::task::spawn_blocking(move || {
        let _lease = lease;
        write()
    })
    .await?
}

#[cfg(test)]
#[path = "blocking_tests.rs"]
mod blocking_tests;
