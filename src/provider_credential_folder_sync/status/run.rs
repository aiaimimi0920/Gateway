use deadpool_redis::Pool;
use time::OffsetDateTime;

use crate::error::GatewayError;
use crate::state::ProviderCredentialFolderSyncRuntime;

use super::super::deletion::apply_explicit_delete_summary;
use super::super::{format_timestamp, FolderSyncCounters};
use super::store::update_folder_sync_status_with_shared_enabled;
use super::{
    update_latest_timestamp, FolderSyncStatusConfig, ProviderCredentialFolderSyncStatusView,
};

#[derive(Default)]
pub(in crate::provider_credential_folder_sync) struct CompletedSyncPhases {
    pub import: bool,
    pub export: bool,
}

pub(in crate::provider_credential_folder_sync) async fn record_sync_run(
    pool: &Pool,
    config: &FolderSyncStatusConfig,
    runtime: &ProviderCredentialFolderSyncRuntime,
    started_at: OffsetDateTime,
    completed: &CompletedSyncPhases,
    counters: &FolderSyncCounters,
    error: Option<&GatewayError>,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    update_folder_sync_status_with_shared_enabled(pool, |status, _present, shared_enabled| {
        if let Some(enabled) = shared_enabled {
            config.apply_shared_enabled(status, enabled);
        } else {
            config.apply_current(status, runtime);
        }
        apply_run_results(status, started_at, completed, counters, error);
    })
    .await
}

fn apply_run_results(
    status: &mut ProviderCredentialFolderSyncStatusView,
    started_at: OffsetDateTime,
    completed: &CompletedSyncPhases,
    counters: &FolderSyncCounters,
    error: Option<&GatewayError>,
) {
    let run_at = format_timestamp(started_at);
    let run_is_latest = update_latest_timestamp(&mut status.last_run_at, run_at.clone());
    if completed.import {
        update_latest_timestamp(&mut status.last_import_at, run_at.clone());
    }
    if completed.export {
        update_latest_timestamp(&mut status.last_export_at, run_at.clone());
    }
    if !run_is_latest {
        // Keep audit events from an admitted run even when a newer run already
        // owns the aggregate counters and error fields.
        apply_explicit_delete_summary(status, counters);
        return;
    }
    status.imported_count = counters.imported_count;
    status.updated_count = counters.updated_count;
    status.exported_count = counters.exported_count;
    status.deleted_count = counters.deleted_count;
    status.skipped_count = counters.skipped_count;
    apply_explicit_delete_summary(status, counters);
    status.last_error = error.map(|error| error.message.clone());
}

#[cfg(test)]
mod tests;
