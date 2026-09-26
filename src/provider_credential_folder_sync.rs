//! Coordinate folder-sync phases and retain the public management/runtime API.
use crate::error::GatewayError;
use crate::state::AppState;
use std::collections::HashSet;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

mod account_cache;
mod accounts;
mod canonicalization;
mod classification;
mod credential_paths;
mod deletion;
mod export;
mod filesystem;
mod import;
mod layout;
mod limits;
mod normalization;
mod path_budget;
mod paths;
mod payload_metadata;
mod run;
mod status;
#[cfg(test)]
mod tests;
mod watcher;

pub use deletion::delete_synced_credential_file;
use filesystem::resolve_root_dir;
use layout::normalize_source_path_key;
use run::run_folder_sync_once_with_explicit_deletes;
pub use status::{
    default_runtime_enabled, folder_sync_root_available, get_folder_sync_status,
    load_runtime_enabled, set_runtime_enabled, FolderSyncExplicitDeleteEventView,
    ProviderCredentialFolderSyncStatusView,
};
use watcher::folder_sync_path_should_trigger;
pub use watcher::start_folder_sync_task;

#[derive(Debug, Clone, Copy)]
pub enum FolderSyncDirection {
    Import,
    Export,
    Both,
}

#[derive(Debug, Default)]
struct FolderSyncCounters {
    imported_count: usize,
    updated_count: usize,
    exported_count: usize,
    deleted_count: usize,
    skipped_count: usize,
    explicit_delete_events: Vec<FolderSyncExplicitDeleteEventView>,
}

pub async fn run_folder_sync_once(
    state: &AppState,
    direction: FolderSyncDirection,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    run_folder_sync_once_with_explicit_deletes(state, direction, &HashSet::new()).await
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
}
