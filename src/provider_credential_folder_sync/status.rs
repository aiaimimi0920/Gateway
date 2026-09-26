use deadpool_redis::Pool;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::config::Config;
use crate::error::GatewayError;
use crate::state::{AppState, ProviderCredentialFolderSyncRuntime};

use super::format_timestamp;

mod config;
mod enable;
mod run;
mod store;
pub(super) use config::FolderSyncStatusConfig;
pub use enable::set_runtime_enabled;
pub(super) use run::{record_sync_run, CompletedSyncPhases};
use store::{
    read_folder_sync_enabled_override, read_folder_sync_status_and_enabled_override,
    update_folder_sync_status_with_shared_enabled,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderCredentialFolderSyncStatusView {
    pub enabled: bool,
    pub root_dir: Option<String>,
    pub interval_seconds: Option<u64>,
    pub watch_enabled: bool,
    pub watch_running: bool,
    pub watch_debounce_millis: Option<u64>,
    pub import_enabled: bool,
    pub export_enabled: bool,
    pub delete_missing: bool,
    pub last_run_at: Option<String>,
    pub last_import_at: Option<String>,
    pub last_export_at: Option<String>,
    pub last_watch_event_at: Option<String>,
    pub last_explicit_delete_at: Option<String>,
    pub last_explicit_delete_count: usize,
    pub last_explicit_delete_paths: Vec<String>,
    pub recent_explicit_delete_events: Vec<FolderSyncExplicitDeleteEventView>,
    pub imported_count: usize,
    pub updated_count: usize,
    pub exported_count: usize,
    pub deleted_count: usize,
    pub skipped_count: usize,
    pub last_error: Option<String>,
    pub last_watch_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct FolderSyncExplicitDeleteEventView {
    pub event_id: String,
    pub occurred_at: String,
    pub deleted_count: usize,
    pub deleted_paths: Vec<String>,
    pub provider_credential_ids: Vec<String>,
}

pub fn folder_sync_root_available(config: &Config) -> bool {
    config
        .provider_credential_folder_sync_root_dir
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

pub fn default_runtime_enabled(config: &Config) -> bool {
    folder_sync_root_available(config) && config.provider_credential_folder_sync_enabled
}

fn resolve_runtime_enabled(config: &Config, enabled: bool) -> bool {
    folder_sync_root_available(config) && enabled
}

pub async fn load_runtime_enabled(
    redis_pool: &Pool,
    config: &Config,
) -> Result<bool, GatewayError> {
    let stored = read_folder_sync_enabled_override(redis_pool).await?;
    Ok(resolve_runtime_enabled(
        config,
        stored.unwrap_or(config.provider_credential_folder_sync_enabled),
    ))
}

pub async fn get_folder_sync_status(
    state: &AppState,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    get_owned_folder_sync_status(
        &state.redis_pool,
        &FolderSyncStatusConfig::new(
            &state.config,
            state.provider_credential_folder_sync.enabled(),
        ),
        &state.provider_credential_folder_sync,
    )
    .await
}

pub(super) async fn get_owned_folder_sync_status(
    pool: &Pool,
    config: &FolderSyncStatusConfig,
    runtime: &ProviderCredentialFolderSyncRuntime,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    let (stored_status, stored_enabled) =
        read_folder_sync_status_and_enabled_override(pool).await?;
    let mut status = stored_status.unwrap_or_default();
    match stored_enabled {
        Some(enabled) => config.apply_shared_enabled(&mut status, enabled),
        None => config.apply_current(&mut status, runtime),
    }
    Ok(status)
}

pub(super) async fn update_watch_runtime_state(
    redis_pool: &Pool,
    config: &Config,
    runtime_enabled: bool,
    watch_running: bool,
    last_watch_event_at: Option<OffsetDateTime>,
    last_watch_error: Option<&str>,
) -> Result<(), GatewayError> {
    update_folder_sync_status_with_shared_enabled(redis_pool, |status, _present, shared_enabled| {
        let folder_config = FolderSyncStatusConfig::new(config, runtime_enabled);
        if let Some(enabled) = shared_enabled {
            // The Redis override is authoritative across Gateway processes.
            folder_config.apply_shared_enabled(status, enabled);
        } else {
            // Without a management override, this process owns the startup value.
            folder_config.apply(status);
        }
        status.watch_running = watch_running;
        if let Some(timestamp) = last_watch_event_at {
            update_latest_timestamp(&mut status.last_watch_event_at, format_timestamp(timestamp));
        }
        if let Some(error) = last_watch_error {
            status.last_watch_error = Some(error.to_string());
        } else if watch_running {
            status.last_watch_error = None;
        }
    })
    .await
    .map(|_| ())
}

pub(super) fn update_latest_timestamp(slot: &mut Option<String>, candidate: String) -> bool {
    let should_update = match slot.as_deref() {
        None => true,
        Some(current) => match (
            OffsetDateTime::parse(current, &Rfc3339).ok(),
            OffsetDateTime::parse(&candidate, &Rfc3339).ok(),
        ) {
            (Some(current), Some(candidate)) => candidate >= current,
            (None, Some(_)) => true,
            _ => false,
        },
    };
    if should_update {
        *slot = Some(candidate);
    }
    should_update
}
