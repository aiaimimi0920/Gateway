use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use deadpool_redis::Pool;
use notify::{
    Config as NotifyConfig, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode,
    Watcher,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use uuid::Uuid;

use crate::config::Config;
use crate::db;
use crate::error::GatewayError;
use crate::implementation_lines;
use crate::protocol::gemini::shared::{
    GEMINI_API_MODULAR_PROFILE, GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE, GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE,
    GEMINI_WEB_REVERSE_MODULAR_PROFILE,
};
use crate::provider_runtime;
use crate::redis::keys;
use crate::state::AppState;

#[derive(Debug, Clone, Copy)]
pub enum FolderSyncDirection {
    Import,
    Export,
    Both,
}

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

#[derive(Debug, Default)]
struct FolderSyncCounters {
    imported_count: usize,
    updated_count: usize,
    exported_count: usize,
    deleted_count: usize,
    skipped_count: usize,
    explicit_delete_events: Vec<FolderSyncExplicitDeleteEventView>,
}

#[derive(Debug, Clone)]
struct FolderSyncExplicitDeleteHit {
    provider_credential_id: String,
    source_path: String,
}

#[derive(Debug, Clone)]
struct FolderWatchEvent {
    deleted_paths: HashSet<String>,
}

impl FolderWatchEvent {
    fn from_event(root_dir: &Path, event: &Event) -> Self {
        Self {
            deleted_paths: deleted_relative_paths_from_event(root_dir, event),
        }
    }
}

#[derive(Debug, Clone)]
enum FolderWatchSignal {
    FilesystemEvent(FolderWatchEvent),
    DebouncedSync,
    WatcherError(String),
}

struct FolderSyncWatchHandles {
    native: Option<RecommendedWatcher>,
    poll: Option<PollWatcher>,
}

impl FolderSyncWatchHandles {
    fn is_running(&self) -> bool {
        self.native.is_some() || self.poll.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderImportPathDescriptor {
    service_provider_slug: Option<String>,
    provider_surface_slug: String,
    credential_material_kind: Option<String>,
}

#[derive(Debug, Default)]
struct FolderSyncProviderMaps {
    legacy_family_map: HashMap<String, Vec<String>>,
    surface_map: HashMap<String, Vec<String>>,
    service_surface_map: HashMap<String, Vec<String>>,
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

pub async fn set_runtime_enabled(
    state: &AppState,
    enabled: bool,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    if enabled && !folder_sync_root_available(&state.config) {
        return Err(GatewayError::service_unavailable(
            "未配置服务商凭证文件夹同步根目录",
        ));
    }
    let runtime_enabled = resolve_runtime_enabled(&state.config, enabled);
    write_folder_sync_enabled_override(&state.redis_pool, runtime_enabled).await?;
    state
        .provider_credential_folder_sync
        .set_enabled(runtime_enabled);

    let mut status = read_folder_sync_status(&state.redis_pool)
        .await?
        .unwrap_or_default();
    apply_folder_sync_config(&mut status, &state.config, runtime_enabled);
    write_folder_sync_status(&state.redis_pool, &status).await?;
    Ok(status)
}

pub async fn get_folder_sync_status(
    state: &AppState,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    let mut status = read_folder_sync_status(&state.redis_pool)
        .await?
        .unwrap_or_default();
    apply_folder_sync_config(
        &mut status,
        &state.config,
        state.provider_credential_folder_sync.enabled(),
    );
    Ok(status)
}

pub async fn run_folder_sync_once(
    state: &AppState,
    direction: FolderSyncDirection,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    run_folder_sync_once_with_explicit_deletes(state, direction, &HashSet::new()).await
}

async fn run_folder_sync_once_with_explicit_deletes(
    state: &AppState,
    direction: FolderSyncDirection,
    explicit_deleted_paths: &HashSet<String>,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    let mut status = get_folder_sync_status(state).await?;

    let root_dir = resolve_root_dir(&state.config)?;
    std::fs::create_dir_all(&root_dir).map_err(|error| {
        GatewayError::server_error(format!(
            "create provider credential sync root {}: {error}",
            root_dir.display()
        ))
    })?;

    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))?;

    let mut counters = FolderSyncCounters::default();
    let now = OffsetDateTime::now_utc();

    let result: Result<(), GatewayError> = async {
        if matches!(
            direction,
            FolderSyncDirection::Import | FolderSyncDirection::Both
        ) {
            if !state.config.provider_credential_folder_sync_import_enabled {
                return Err(GatewayError::bad_request(
                    "当前未启用服务商凭证文件夹导入模式",
                ));
            }
            import_folder_credentials(
                pg_pool,
                &state.redis_pool,
                &root_dir,
                state.config.provider_credential_folder_sync_delete_missing,
                explicit_deleted_paths,
                &mut counters,
            )
            .await?;
            status.last_import_at = Some(format_timestamp(now));
        }
        if matches!(
            direction,
            FolderSyncDirection::Export | FolderSyncDirection::Both
        ) {
            if !state.config.provider_credential_folder_sync_export_enabled {
                return Err(GatewayError::bad_request(
                    "当前未启用服务商凭证文件夹导出模式",
                ));
            }
            export_database_credentials(
                pg_pool,
                &root_dir,
                state.config.provider_credential_folder_sync_delete_missing,
                &mut counters,
            )
            .await?;
            status.last_export_at = Some(format_timestamp(now));
        }
        Ok(())
    }
    .await;

    status.last_run_at = Some(format_timestamp(now));
    status.imported_count = counters.imported_count;
    status.updated_count = counters.updated_count;
    status.exported_count = counters.exported_count;
    status.deleted_count = counters.deleted_count;
    status.skipped_count = counters.skipped_count;
    apply_explicit_delete_summary(&mut status, &counters);
    status.last_error = result.as_ref().err().map(|error| error.message.clone());

    write_folder_sync_status(&state.redis_pool, &status).await?;
    result?;
    Ok(status)
}

pub async fn start_folder_sync_task(state: Arc<AppState>) {
    if !folder_sync_root_available(&state.config) {
        tracing::info!("provider credential folder sync root dir is not configured");
        return;
    }

    let runtime_enabled = state.provider_credential_folder_sync.enabled();
    if !runtime_enabled {
        tracing::info!(
            root_dir = ?state.config.provider_credential_folder_sync_root_dir,
            import_enabled = state.config.provider_credential_folder_sync_import_enabled,
            export_enabled = state.config.provider_credential_folder_sync_export_enabled,
            "provider credential folder sync runtime is disabled; skipping watcher startup"
        );
        if let Err(error) =
            update_watch_runtime_state(&state.redis_pool, &state.config, false, false, None, None)
                .await
        {
            tracing::warn!(
                error = %error.message,
                "failed to record disabled provider credential folder sync state"
            );
        }
        return;
    }

    let root_dir = match resolve_root_dir(&state.config) {
        Ok(root_dir) => root_dir,
        Err(error) => {
            tracing::warn!(
                error = %error.message,
                "provider credential folder sync root dir is not configured"
            );
            return;
        }
    };
    if let Err(error) = std::fs::create_dir_all(&root_dir) {
        tracing::warn!(
            path = %root_dir.display(),
            error = %error,
            "failed to create provider credential folder sync root directory"
        );
        return;
    }

    let interval_secs = state
        .config
        .provider_credential_folder_sync_interval_secs
        .max(5);
    let watch_enabled = state.config.provider_credential_folder_sync_watch_enabled;
    let watch_debounce_millis = state
        .config
        .provider_credential_folder_sync_watch_debounce_millis
        .max(250);
    tracing::info!(
        interval_secs,
        watch_enabled,
        watch_debounce_millis,
        runtime_enabled,
        root_dir = ?state.config.provider_credential_folder_sync_root_dir,
        import_enabled = state.config.provider_credential_folder_sync_import_enabled,
        export_enabled = state.config.provider_credential_folder_sync_export_enabled,
        "starting provider credential folder sync task"
    );

    let (watch_tx, mut watch_rx) = unbounded_channel::<FolderWatchSignal>();
    let watch_handles = if watch_enabled {
        let handles =
            start_folder_sync_watchers(&root_dir, watch_tx.clone(), watch_debounce_millis);
        let watch_running = handles.is_running();
        let watch_error = (!watch_running).then_some(
            "failed to start provider credential folder watchers; falling back to periodic rescan",
        );
        if let Err(status_error) = update_watch_runtime_state(
            &state.redis_pool,
            &state.config,
            runtime_enabled,
            watch_running,
            None,
            watch_error,
        )
        .await
        {
            tracing::warn!(
                error = %status_error.message,
                "failed to record folder sync watch startup state"
            );
        }
        handles
    } else {
        if let Err(error) = update_watch_runtime_state(
            &state.redis_pool,
            &state.config,
            runtime_enabled,
            false,
            None,
            None,
        )
        .await
        {
            tracing::warn!(
                error = %error.message,
                "failed to record folder sync watch disabled state"
            );
        }
        FolderSyncWatchHandles {
            native: None,
            poll: None,
        }
    };

    if let Err(error) = run_folder_sync_once(&state, FolderSyncDirection::Both).await {
        tracing::warn!(error = %error.message, "initial provider credential folder sync failed");
    }

    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    interval.tick().await;
    let mut enabled_rx = state.provider_credential_folder_sync.subscribe();
    let mut debounce_scheduled = false;
    let mut debounce_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut pending_deleted_paths = HashSet::new();
    loop {
        tokio::select! {
            changed = enabled_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                let runtime_enabled = *enabled_rx.borrow_and_update();
                if !runtime_enabled {
                    debounce_scheduled = false;
                    pending_deleted_paths.clear();
                    if let Some(handle) = debounce_task.take() {
                        handle.abort();
                    }
                } else if let Err(error) = run_folder_sync_once_with_explicit_deletes(
                    &state,
                    FolderSyncDirection::Both,
                    &pending_deleted_paths,
                )
                .await
                {
                    tracing::warn!(error = %error.message, "runtime-enabled provider credential folder sync failed");
                } else {
                    pending_deleted_paths.clear();
                }
                if let Err(error) = update_watch_runtime_state(
                    &state.redis_pool,
                    &state.config,
                    runtime_enabled,
                    watch_handles.is_running(),
                    None,
                    None,
                )
                .await
                {
                    tracing::warn!(
                        error = %error.message,
                        "failed to record provider credential folder sync runtime toggle"
                    );
                }
            }
            _ = interval.tick() => {
                let runtime_enabled = *enabled_rx.borrow();
                if !runtime_enabled {
                    continue;
                }
                if let Err(error) = run_folder_sync_once_with_explicit_deletes(
                    &state,
                    FolderSyncDirection::Both,
                    &pending_deleted_paths,
                )
                .await
                {
                    tracing::warn!(error = %error.message, "provider credential folder sync periodic rescan failed");
                } else {
                    pending_deleted_paths.clear();
                }
            }
            Some(signal) = watch_rx.recv() => {
                let runtime_enabled = *enabled_rx.borrow();
                if !runtime_enabled {
                    continue;
                }
                match signal {
                    FolderWatchSignal::FilesystemEvent(event) => {
                        pending_deleted_paths.extend(event.deleted_paths);
                        if let Err(error) = update_watch_runtime_state(
                            &state.redis_pool,
                            &state.config,
                            true,
                            watch_handles.is_running(),
                            Some(OffsetDateTime::now_utc()),
                            None,
                        )
                        .await
                        {
                            tracing::warn!(
                                error = %error.message,
                                "failed to record provider credential folder watch event"
                            );
                        }
                        if !debounce_scheduled {
                            debounce_scheduled = true;
                            let debounce_tx = watch_tx.clone();
                            debounce_task = Some(tokio::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(watch_debounce_millis)).await;
                                let _ = debounce_tx.send(FolderWatchSignal::DebouncedSync);
                            }));
                        }
                    }
                    FolderWatchSignal::DebouncedSync => {
                        debounce_scheduled = false;
                        if let Some(handle) = debounce_task.take() {
                            handle.abort();
                        }
                        if let Err(error) = run_folder_sync_once_with_explicit_deletes(
                            &state,
                            FolderSyncDirection::Both,
                            &pending_deleted_paths,
                        )
                        .await
                        {
                            tracing::warn!(error = %error.message, "provider credential folder sync watch-triggered run failed");
                        } else {
                            pending_deleted_paths.clear();
                        }
                    }
                    FolderWatchSignal::WatcherError(message) => {
                        tracing::warn!(error = %message, "provider credential folder sync watcher error");
                        let runtime_enabled = *enabled_rx.borrow();
                        if let Err(error) = update_watch_runtime_state(
                            &state.redis_pool,
                            &state.config,
                            runtime_enabled,
                            watch_handles.is_running(),
                            None,
                            Some(message.as_str()),
                        )
                        .await
                        {
                            tracing::warn!(
                                error = %error.message,
                                "failed to record provider credential watcher error"
                            );
                        }
                    }
                }
            }
        }
    }
}

pub fn delete_synced_credential_file(
    state: &AppState,
    relative_path: &str,
) -> Result<bool, GatewayError> {
    if !state.provider_credential_folder_sync.enabled() {
        return Ok(false);
    }
    let Some(relative_path) = normalize_source_path_key(relative_path) else {
        return Ok(false);
    };
    let root_dir = resolve_root_dir(&state.config)?;
    let absolute = absolute_path_for_relative(&root_dir, &relative_path);
    if !absolute.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&absolute).map_err(|error| {
        GatewayError::server_error(format!(
            "delete provider credential file {}: {error}",
            absolute.display()
        ))
    })?;
    Ok(true)
}

async fn import_folder_credentials(
    pg_pool: &sqlx::PgPool,
    redis_pool: &Pool,
    root_dir: &Path,
    delete_missing: bool,
    explicit_deleted_paths: &HashSet<String>,
    counters: &mut FolderSyncCounters,
) -> Result<(), GatewayError> {
    let provider_accounts = db::list_provider_accounts(pg_pool).await?;
    let account_map = provider_accounts
        .iter()
        .map(|account| (account.id.clone(), account.clone()))
        .collect::<HashMap<_, _>>();
    let provider_maps = build_provider_maps(&provider_accounts);
    let existing_credentials = db::list_provider_credentials(pg_pool, None).await?;
    let existing_by_path = existing_credentials
        .iter()
        .filter_map(|credential| {
            credential
                .source_path
                .as_deref()
                .and_then(normalize_source_path_key)
                .map(|source_path| (source_path, credential.clone()))
        })
        .collect::<HashMap<_, _>>();
    let files = collect_json_files(root_dir)?;
    let mut observed_paths = HashSet::new();

    for file_path in files {
        let relative = normalize_relative_path(root_dir, &file_path)?;
        observed_paths.insert(relative.clone());

        let (hash, raw_payload) = read_provider_credential_json_with_retry(&file_path).await?;
        let Some(path_descriptor) = resolve_import_path_descriptor(&relative, &raw_payload) else {
            counters.skipped_count += 1;
            continue;
        };
        let Some(provider_account_id) =
            select_provider_account_for_path(&provider_maps, &path_descriptor)
        else {
            counters.skipped_count += 1;
            continue;
        };
        let provider_account = account_map
            .get(&provider_account_id)
            .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
        let payload = normalize_import_payload(
            &path_descriptor.provider_surface_slug,
            provider_account,
            raw_payload,
            path_descriptor.credential_material_kind.as_deref(),
        )?;

        match existing_by_path.get(&relative) {
            Some(existing) if existing.source_hash.as_deref() == Some(hash.as_str()) => {
                counters.skipped_count += 1;
            }
            Some(existing) => {
                db::update_provider_credential(
                    pg_pool,
                    &existing.id,
                    db::UpsertProviderCredentialInput {
                        provider_account_id: provider_account_id.clone(),
                        label: existing.label.clone(),
                        status: Some(existing.status.clone()),
                        payload,
                        source_kind: Some("folder_sync_import".to_string()),
                        source_path: Some(relative.clone()),
                        source_hash: Some(hash),
                        sync_mode: Some("folder_sync".to_string()),
                        sync_state: Some("imported".to_string()),
                        sync_error: None,
                    },
                )
                .await?;
                counters.updated_count += 1;
            }
            None => {
                db::create_provider_credential(
                    pg_pool,
                    db::UpsertProviderCredentialInput {
                        provider_account_id,
                        label: credential_label_from_path(&file_path),
                        status: Some("active".to_string()),
                        payload,
                        source_kind: Some("folder_sync_import".to_string()),
                        source_path: Some(relative),
                        source_hash: Some(hash),
                        sync_mode: Some("folder_sync".to_string()),
                        sync_state: Some("imported".to_string()),
                        sync_error: None,
                    },
                )
                .await?;
                counters.imported_count += 1;
            }
        }
    }

    delete_explicitly_removed_folder_credentials(
        pg_pool,
        redis_pool,
        &existing_credentials,
        &observed_paths,
        explicit_deleted_paths,
        counters,
    )
    .await?;

    if delete_missing {
        delete_missing_folder_credentials(
            pg_pool,
            redis_pool,
            &existing_credentials,
            &observed_paths,
            counters,
        )
        .await?;
    }

    Ok(())
}

async fn delete_explicitly_removed_folder_credentials(
    pg_pool: &sqlx::PgPool,
    redis_pool: &Pool,
    existing_credentials: &[db::GatewayProviderCredentialView],
    observed_paths: &HashSet<String>,
    explicit_deleted_paths: &HashSet<String>,
    counters: &mut FolderSyncCounters,
) -> Result<(), GatewayError> {
    let mut deleted_hits = Vec::new();
    for credential in existing_credentials.iter().filter(|credential| {
        should_delete_explicitly_removed_folder_credential(
            credential,
            observed_paths,
            explicit_deleted_paths,
        )
    }) {
        if let Some(source_path) = credential
            .source_path
            .as_deref()
            .and_then(normalize_source_path_key)
        {
            deleted_hits.push(FolderSyncExplicitDeleteHit {
                provider_credential_id: credential.id.clone(),
                source_path,
            });
        }
        db::delete_provider_credential(pg_pool, credential.id.as_str()).await?;
        provider_runtime::clear_provider_credential_runtime_keys(
            redis_pool,
            credential.id.as_str(),
        )
        .await?;
        counters.deleted_count += 1;
    }
    if !deleted_hits.is_empty() {
        let audit_event = build_explicit_delete_event(&deleted_hits);
        tracing::info!(
            audit_event_id = %audit_event.event_id,
            explicit_delete_count = audit_event.deleted_count,
            deleted_paths = ?audit_event.deleted_paths,
            provider_credential_ids = ?audit_event.provider_credential_ids,
            "provider credential folder sync explicit delete audit"
        );
        counters.explicit_delete_events.push(audit_event);
    }
    Ok(())
}

async fn delete_missing_folder_credentials(
    pg_pool: &sqlx::PgPool,
    redis_pool: &Pool,
    existing_credentials: &[db::GatewayProviderCredentialView],
    observed_paths: &HashSet<String>,
    counters: &mut FolderSyncCounters,
) -> Result<(), GatewayError> {
    for credential in existing_credentials
        .iter()
        .filter(|credential| should_delete_missing_folder_credential(credential, observed_paths))
    {
        db::delete_provider_credential(pg_pool, credential.id.as_str()).await?;
        provider_runtime::clear_provider_credential_runtime_keys(
            redis_pool,
            credential.id.as_str(),
        )
        .await?;
        counters.deleted_count += 1;
    }
    Ok(())
}

async fn export_database_credentials(
    pg_pool: &sqlx::PgPool,
    root_dir: &Path,
    delete_missing: bool,
    counters: &mut FolderSyncCounters,
) -> Result<(), GatewayError> {
    let provider_accounts = db::list_provider_accounts(pg_pool).await?;
    let account_map = provider_accounts
        .into_iter()
        .map(|account| (account.id.clone(), account))
        .collect::<HashMap<_, _>>();
    let credentials = db::list_provider_credentials(pg_pool, None).await?;
    let mut expected_paths = HashSet::new();

    for credential in credentials
        .into_iter()
        .filter(|item| item.archived_at.is_none() && item.status != "archived")
    {
        let Some(provider_account) = account_map.get(&credential.provider_account_id) else {
            counters.skipped_count += 1;
            continue;
        };
        let relative = credential
            .source_path
            .as_deref()
            .and_then(normalize_source_path_key)
            .unwrap_or_else(|| default_folder_sync_relative_path(provider_account, &credential));
        expected_paths.insert(relative.clone());
        let absolute = absolute_path_for_relative(root_dir, &relative);
        if let Some(parent) = absolute.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                GatewayError::server_error(format!(
                    "create provider credential directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        let export_payload = materialize_export_payload(provider_account, &credential.payload);
        let serialized = serde_json::to_vec_pretty(&export_payload).map_err(|error| {
            GatewayError::server_error(format!(
                "serialize provider credential {} for folder sync: {error}",
                credential.id
            ))
        })?;
        let hash = sha256_hex(&serialized);
        let needs_write = match std::fs::read(&absolute) {
            Ok(existing) => sha256_hex(&existing) != hash,
            Err(_) => true,
        };
        if needs_write {
            std::fs::write(&absolute, &serialized).map_err(|error| {
                GatewayError::server_error(format!(
                    "write provider credential file {}: {error}",
                    absolute.display()
                ))
            })?;
            counters.exported_count += 1;
        } else {
            counters.skipped_count += 1;
        }
        db::update_provider_credential_sync_metadata(
            pg_pool,
            &credential.id,
            Some(relative.as_str()),
            Some(hash.as_str()),
            Some("folder_sync"),
            "exported",
            None,
        )
        .await?;
    }

    if delete_missing {
        for file_path in collect_json_files(root_dir)? {
            let relative = normalize_relative_path(root_dir, &file_path)?;
            if expected_paths.contains(&relative) {
                continue;
            }
            std::fs::remove_file(&file_path).map_err(|error| {
                GatewayError::server_error(format!(
                    "delete stale provider credential file {}: {error}",
                    file_path.display()
                ))
            })?;
            counters.deleted_count += 1;
        }
    }

    Ok(())
}

async fn read_provider_credential_json_with_retry(
    file_path: &Path,
) -> Result<(String, Value), GatewayError> {
    let mut last_error: Option<GatewayError> = None;
    for attempt in 0..5 {
        match read_provider_credential_json_once(file_path) {
            Ok(result) => return Ok(result),
            Err(error) => {
                last_error = Some(error);
                if attempt < 4 {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| {
        GatewayError::server_error(format!(
            "read provider credential file {} failed without concrete error",
            file_path.display()
        ))
    }))
}

fn read_provider_credential_json_once(file_path: &Path) -> Result<(String, Value), GatewayError> {
    let bytes = std::fs::read(file_path).map_err(|error| {
        GatewayError::server_error(format!(
            "read provider credential file {}: {error}",
            file_path.display()
        ))
    })?;
    let raw_payload: Value = serde_json::from_slice(&bytes).map_err(|error| {
        GatewayError::bad_request(format!(
            "provider credential file {} is not valid JSON: {error}",
            file_path.display()
        ))
    })?;
    if !raw_payload.is_object() {
        return Err(GatewayError::bad_request(format!(
            "provider credential file {} 必须是 JSON object",
            file_path.display()
        )));
    }
    Ok((sha256_hex(&bytes), raw_payload))
}

async fn read_folder_sync_status(
    redis_pool: &Pool,
) -> Result<Option<ProviderCredentialFolderSyncStatusView>, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> = conn
        .get(keys::provider_credential_folder_sync_status_key())
        .await
        .map_err(|error| GatewayError::server_error(format!("read folder sync status: {error}")))?;
    match raw {
        Some(raw) => serde_json::from_str(&raw).map(Some).map_err(|error| {
            GatewayError::server_error(format!("decode folder sync status: {error}"))
        }),
        None => Ok(None),
    }
}

async fn read_folder_sync_enabled_override(
    redis_pool: &Pool,
) -> Result<Option<bool>, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> = conn
        .get(keys::provider_credential_folder_sync_enabled_key())
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("read folder sync enabled: {error}"))
        })?;
    match raw {
        Some(raw) => serde_json::from_str(&raw).map(Some).map_err(|error| {
            GatewayError::server_error(format!("decode folder sync enabled: {error}"))
        }),
        None => Ok(None),
    }
}

async fn write_folder_sync_enabled_override(
    redis_pool: &Pool,
    enabled: bool,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw = serde_json::to_string(&enabled).map_err(|error| {
        GatewayError::server_error(format!("encode folder sync enabled: {error}"))
    })?;
    redis::cmd("SET")
        .arg(keys::provider_credential_folder_sync_enabled_key())
        .arg(raw)
        .query_async::<()>(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write folder sync enabled: {error}"))
        })?;
    Ok(())
}

async fn write_folder_sync_status(
    redis_pool: &Pool,
    status: &ProviderCredentialFolderSyncStatusView,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw = serde_json::to_string(status).map_err(|error| {
        GatewayError::server_error(format!("encode folder sync status: {error}"))
    })?;
    redis::cmd("SET")
        .arg(keys::provider_credential_folder_sync_status_key())
        .arg(raw)
        .query_async::<()>(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write folder sync status: {error}"))
        })?;
    Ok(())
}

fn apply_folder_sync_config(
    status: &mut ProviderCredentialFolderSyncStatusView,
    config: &Config,
    runtime_enabled: bool,
) {
    status.enabled = resolve_runtime_enabled(config, runtime_enabled);
    status.root_dir = config.provider_credential_folder_sync_root_dir.clone();
    status.interval_seconds = folder_sync_root_available(config)
        .then_some(config.provider_credential_folder_sync_interval_secs);
    status.watch_enabled =
        folder_sync_root_available(config) && config.provider_credential_folder_sync_watch_enabled;
    status.watch_debounce_millis = status
        .watch_enabled
        .then_some(config.provider_credential_folder_sync_watch_debounce_millis);
    status.import_enabled = config.provider_credential_folder_sync_import_enabled;
    status.export_enabled = config.provider_credential_folder_sync_export_enabled;
    status.delete_missing = config.provider_credential_folder_sync_delete_missing;
    if !status.watch_enabled {
        status.watch_running = false;
    }
}

async fn update_watch_runtime_state(
    redis_pool: &Pool,
    config: &Config,
    runtime_enabled: bool,
    watch_running: bool,
    last_watch_event_at: Option<OffsetDateTime>,
    last_watch_error: Option<&str>,
) -> Result<(), GatewayError> {
    let mut status = read_folder_sync_status(redis_pool)
        .await?
        .unwrap_or_default();
    apply_folder_sync_config(&mut status, config, runtime_enabled);
    status.watch_running = watch_running;
    if let Some(timestamp) = last_watch_event_at {
        status.last_watch_event_at = Some(format_timestamp(timestamp));
    }
    if let Some(error) = last_watch_error {
        status.last_watch_error = Some(error.to_string());
    } else if watch_running {
        status.last_watch_error = None;
    }
    write_folder_sync_status(redis_pool, &status).await
}

fn resolve_root_dir(config: &Config) -> Result<PathBuf, GatewayError> {
    config
        .provider_credential_folder_sync_root_dir
        .as_ref()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| GatewayError::service_unavailable("未配置服务商凭证文件夹同步根目录"))
}

fn start_folder_sync_watchers(
    root_dir: &Path,
    signal_tx: UnboundedSender<FolderWatchSignal>,
    watch_debounce_millis: u64,
) -> FolderSyncWatchHandles {
    let native = match start_native_folder_sync_watcher(root_dir, signal_tx.clone()) {
        Ok(watcher) => Some(watcher),
        Err(error) => {
            tracing::warn!(
                error = %error.message,
                path = %root_dir.display(),
                "failed to start native provider credential folder watcher"
            );
            None
        }
    };
    let poll_interval = Duration::from_millis(watch_debounce_millis.max(1000));
    let poll = match start_poll_folder_sync_watcher(root_dir, signal_tx, poll_interval) {
        Ok(watcher) => Some(watcher),
        Err(error) => {
            tracing::warn!(
                error = %error.message,
                path = %root_dir.display(),
                "failed to start poll fallback provider credential folder watcher"
            );
            None
        }
    };
    FolderSyncWatchHandles { native, poll }
}

fn start_native_folder_sync_watcher(
    root_dir: &Path,
    signal_tx: UnboundedSender<FolderWatchSignal>,
) -> Result<RecommendedWatcher, GatewayError> {
    let callback_tx = signal_tx.clone();
    let watch_root = root_dir.to_path_buf();
    let mut watcher =
        notify::recommended_watcher(move |result: Result<Event, notify::Error>| match result {
            Ok(event) => {
                if folder_sync_event_should_trigger(&event) {
                    let _ = callback_tx.send(FolderWatchSignal::FilesystemEvent(
                        FolderWatchEvent::from_event(watch_root.as_path(), &event),
                    ));
                }
            }
            Err(error) => {
                let _ = callback_tx.send(FolderWatchSignal::WatcherError(error.to_string()));
            }
        })
        .map_err(|error| {
            GatewayError::server_error(format!(
                "create provider credential folder watcher for {}: {error}",
                root_dir.display()
            ))
        })?;
    watcher
        .watch(root_dir, RecursiveMode::Recursive)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "watch provider credential folder {}: {error}",
                root_dir.display()
            ))
        })?;
    Ok(watcher)
}

fn start_poll_folder_sync_watcher(
    root_dir: &Path,
    signal_tx: UnboundedSender<FolderWatchSignal>,
    poll_interval: Duration,
) -> Result<PollWatcher, GatewayError> {
    let callback_tx = signal_tx.clone();
    let watch_root = root_dir.to_path_buf();
    let mut watcher = PollWatcher::new(
        move |result: Result<Event, notify::Error>| match result {
            Ok(event) => {
                if folder_sync_event_should_trigger(&event) {
                    let _ = callback_tx.send(FolderWatchSignal::FilesystemEvent(
                        FolderWatchEvent::from_event(watch_root.as_path(), &event),
                    ));
                }
            }
            Err(error) => {
                let _ = callback_tx.send(FolderWatchSignal::WatcherError(error.to_string()));
            }
        },
        NotifyConfig::default().with_poll_interval(poll_interval),
    )
    .map_err(|error| {
        GatewayError::server_error(format!(
            "create poll provider credential folder watcher for {}: {error}",
            root_dir.display()
        ))
    })?;
    watcher
        .watch(root_dir, RecursiveMode::Recursive)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "watch provider credential folder with poll watcher {}: {error}",
                root_dir.display()
            ))
        })?;
    Ok(watcher)
}

fn folder_sync_event_should_trigger(event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    if event.paths.is_empty() {
        return true;
    }
    event
        .paths
        .iter()
        .any(|path| folder_sync_path_should_trigger(path.as_path()))
}

fn folder_sync_path_should_trigger(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => ext.eq_ignore_ascii_case("json"),
        None => true,
    }
}

fn build_provider_maps(
    provider_accounts: &[db::GatewayProviderAccountView],
) -> FolderSyncProviderMaps {
    FolderSyncProviderMaps {
        legacy_family_map: build_provider_legacy_family_map(provider_accounts),
        surface_map: build_provider_surface_map(provider_accounts),
        service_surface_map: build_provider_service_surface_map(provider_accounts),
    }
}

fn build_provider_legacy_family_map(
    provider_accounts: &[db::GatewayProviderAccountView],
) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut grouped: HashMap<String, Vec<&db::GatewayProviderAccountView>> = HashMap::new();
    for account in provider_accounts {
        grouped
            .entry(derive_provider_family_slug(account))
            .or_default()
            .push(account);
    }
    for (family, mut accounts) in grouped {
        accounts.sort_by(|left, right| {
            provider_family_selection_score(right, family.as_str())
                .cmp(&provider_family_selection_score(left, family.as_str()))
                .then_with(|| left.label.cmp(&right.label))
        });
        map.insert(
            family,
            accounts
                .into_iter()
                .map(|account| account.id.clone())
                .collect::<Vec<_>>(),
        );
    }
    map
}

fn build_provider_surface_map(
    provider_accounts: &[db::GatewayProviderAccountView],
) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut grouped: HashMap<String, Vec<&db::GatewayProviderAccountView>> = HashMap::new();
    for account in provider_accounts {
        grouped
            .entry(derive_provider_surface_slug(account))
            .or_default()
            .push(account);
    }
    for (surface, mut accounts) in grouped {
        accounts.sort_by(|left, right| {
            provider_surface_selection_score(right, surface.as_str())
                .cmp(&provider_surface_selection_score(left, surface.as_str()))
                .then_with(|| left.label.cmp(&right.label))
        });
        map.insert(
            surface,
            accounts
                .into_iter()
                .map(|account| account.id.clone())
                .collect::<Vec<_>>(),
        );
    }
    map
}

fn build_provider_service_surface_map(
    provider_accounts: &[db::GatewayProviderAccountView],
) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut grouped: HashMap<String, Vec<&db::GatewayProviderAccountView>> = HashMap::new();
    for account in provider_accounts {
        grouped
            .entry(service_surface_lookup_key(
                derive_service_provider_slug(account).as_str(),
                derive_provider_surface_slug(account).as_str(),
            ))
            .or_default()
            .push(account);
    }
    for (key, mut accounts) in grouped {
        let (_, surface) = key.split_once("::").unwrap_or((key.as_str(), key.as_str()));
        accounts.sort_by(|left, right| {
            provider_surface_selection_score(right, surface)
                .cmp(&provider_surface_selection_score(left, surface))
                .then_with(|| left.label.cmp(&right.label))
        });
        map.insert(
            key,
            accounts
                .into_iter()
                .map(|account| account.id.clone())
                .collect::<Vec<_>>(),
        );
    }
    map
}

fn select_provider_account_for_family(
    family_map: &HashMap<String, Vec<String>>,
    family_slug: &str,
) -> Option<String> {
    let family_slug = canonicalize_folder_family_slug(family_slug);
    family_map
        .get(family_slug.as_str())
        .and_then(|items| items.first())
        .cloned()
}

fn select_provider_account_for_path(
    provider_maps: &FolderSyncProviderMaps,
    descriptor: &FolderImportPathDescriptor,
) -> Option<String> {
    if let Some(service_provider_slug) = descriptor.service_provider_slug.as_deref() {
        let service_surface_key = service_surface_lookup_key(
            service_provider_slug,
            descriptor.provider_surface_slug.as_str(),
        );
        if let Some(provider_account_id) = provider_maps
            .service_surface_map
            .get(service_surface_key.as_str())
            .and_then(|items| items.first())
            .cloned()
        {
            return Some(provider_account_id);
        }
    }

    provider_maps
        .surface_map
        .get(descriptor.provider_surface_slug.as_str())
        .and_then(|items| items.first())
        .cloned()
        .or_else(|| {
            select_provider_account_for_family(
                &provider_maps.legacy_family_map,
                descriptor.provider_surface_slug.as_str(),
            )
        })
}

fn resolve_import_path_descriptor(
    relative: &str,
    raw_payload: &Value,
) -> Option<FolderImportPathDescriptor> {
    let path_descriptor = describe_folder_import_path(relative)?;
    let raw_map = raw_payload.as_object()?;

    let service_provider_slug = raw_map
        .get("serviceProviderKey")
        .or_else(|| raw_map.get("service_provider_key"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(canonicalize_folder_service_provider_slug)
        .or(path_descriptor.service_provider_slug);

    let provider_surface_slug = raw_map
        .get("providerSurfaceKey")
        .or_else(|| raw_map.get("provider_surface_key"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(canonicalize_folder_surface_slug)
        .unwrap_or(path_descriptor.provider_surface_slug);

    let credential_material_kind = raw_map
        .get("credentialMaterialKind")
        .or_else(|| raw_map.get("credential_material_kind"))
        .or_else(|| raw_map.get("materialKind"))
        .or_else(|| raw_map.get("material_kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(canonicalize_credential_material_kind)
        .or(path_descriptor.credential_material_kind);

    Some(FolderImportPathDescriptor {
        service_provider_slug,
        provider_surface_slug,
        credential_material_kind,
    })
}

fn describe_folder_import_path(relative: &str) -> Option<FolderImportPathDescriptor> {
    let normalized = normalize_source_path_key(relative)?;
    let segments = normalized.split('/').collect::<Vec<_>>();
    if segments.len() < 2 {
        return None;
    }
    if segments.len() >= 4 {
        return Some(FolderImportPathDescriptor {
            service_provider_slug: Some(canonicalize_folder_service_provider_slug(segments[0])),
            provider_surface_slug: canonicalize_folder_surface_slug(segments[1]),
            credential_material_kind: Some(canonicalize_credential_material_kind(segments[2])),
        });
    }
    if segments.len() == 3 {
        return Some(FolderImportPathDescriptor {
            service_provider_slug: Some(canonicalize_folder_service_provider_slug(segments[0])),
            provider_surface_slug: canonicalize_folder_surface_slug(segments[1]),
            credential_material_kind: None,
        });
    }
    Some(FolderImportPathDescriptor {
        service_provider_slug: None,
        provider_surface_slug: canonicalize_folder_surface_slug(segments[0]),
        credential_material_kind: None,
    })
}

fn derive_provider_family_slug(provider_account: &db::GatewayProviderAccountView) -> String {
    let label = provider_account.label.to_ascii_lowercase();
    let adapter = provider_account.adapter.to_ascii_lowercase();
    let protocol_profile = provider_account.protocol_profile.to_ascii_lowercase();
    let protocol_family = provider_account.protocol_family.to_ascii_lowercase();
    let service_provider_key = provider_account.service_provider_key.to_ascii_lowercase();
    let source_kind = provider_account
        .source_kind
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let web_reverse_access_mode = provider_account
        .web_reverse_access_mode
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let payload_base_url = provider_account
        .payload
        .get("baseUrl")
        .and_then(Value::as_str)
        .or_else(|| {
            provider_account
                .payload
                .get("base_url")
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .to_ascii_lowercase();

    if let Some(family_slug) = derive_gemini_provider_family_slug(
        &adapter,
        &protocol_profile,
        &service_provider_key,
        &source_kind,
        &web_reverse_access_mode,
        &payload_base_url,
    ) {
        return family_slug;
    }

    if adapter == "accio_compatible"
        || protocol_profile == "accio"
        || service_provider_key == "accio_platform"
        || payload_base_url.contains("phoenix-gw.alibaba.com")
        || label.contains("accio")
    {
        return "accio".to_string();
    }
    if adapter == "qwen_web_compatible"
        || protocol_profile == "qwen_web_chat"
        || protocol_family == "qwen_web_chat"
        || (service_provider_key == "qwen_platform"
            && source_kind == "web_reverse_api"
            && web_reverse_access_mode == "direct_http_replay")
        || payload_base_url.contains("chat.qwen.ai")
    {
        return "qwen-web-chat".to_string();
    }
    if adapter == "gemini_web_compatible"
        || protocol_profile == "gemini_web"
        || protocol_family == "gemini_web_chat"
        || (service_provider_key == "gemini_platform"
            && source_kind == "web_reverse_api"
            && web_reverse_access_mode == "direct_http_replay"
            && payload_base_url.contains("gemini.google.com"))
    {
        return "gemini-web-chat".to_string();
    }
    if adapter == "chatgpt_web_reverse_compatible"
        || protocol_profile == "chatgpt_web_reverse"
        || protocol_family == "chatgpt_web_chat"
        || (service_provider_key == "chatgpt_platform"
            && source_kind == "web_reverse_api"
            && web_reverse_access_mode == "direct_http_replay")
        || (payload_base_url.contains("chatgpt.com")
            && payload_base_url.contains("/backend-api/conversation"))
    {
        return "chatgpt-web-reverse".to_string();
    }
    if protocol_profile == "nvidia"
        || service_provider_key == "nvidia_platform"
        || payload_base_url.contains("integrate.api.nvidia.com")
        || payload_base_url.contains("api.nvidia.com")
        || label.contains("nvidia")
    {
        return "nvidia".to_string();
    }
    if protocol_profile == "grok_web"
        || service_provider_key == "grok_platform"
        || payload_base_url.contains("grok.com")
        || label.contains("grok")
    {
        return "grok".to_string();
    }
    if adapter == "suno_compatible"
        || protocol_profile == "suno"
        || service_provider_key == "suno_platform"
        || payload_base_url.contains("suno.com")
        || label.contains("suno")
    {
        return "suno".to_string();
    }
    if adapter == "udio_compatible"
        || protocol_profile == "udio"
        || service_provider_key == "udio_platform"
        || payload_base_url.contains("udio.com")
        || label.contains("udio")
    {
        return "udio".to_string();
    }
    if adapter == "lumalabs_compatible"
        || protocol_profile == "lumalabs"
        || service_provider_key == "lumalabs_platform"
        || payload_base_url.contains("lumalabs.ai")
        || label.contains("lumalabs")
        || label.contains("luma labs")
    {
        return "lumalabs".to_string();
    }
    if protocol_profile == "xai"
        || protocol_profile == "xai_openai"
        || service_provider_key == "xai_platform"
        || payload_base_url.contains("api.x.ai")
        || label.contains("xai")
        || label.contains("x.ai")
    {
        return "xai".to_string();
    }
    if protocol_profile == "perplexity_chat"
        || service_provider_key == "perplexity_platform"
        || payload_base_url.contains("api.perplexity.ai")
        || label.contains("perplexity")
    {
        return "perplexity".to_string();
    }
    if adapter == "freebuff_compatible"
        || protocol_profile == "freebuff"
        || service_provider_key == "freebuff_platform"
        || payload_base_url.contains("codebuff.com")
        || label.contains("freebuff")
        || label.contains("codebuff")
    {
        return "freebuff".to_string();
    }
    if protocol_profile == "xfyun_openai"
        || protocol_profile == "xfyun_native_websocket"
        || service_provider_key == "xfyun_platform"
        || payload_base_url.contains("xf-yun.com")
        || payload_base_url.contains("xfyun.cn")
        || label.contains("xfyun")
        || label.contains("讯飞")
    {
        return "xfyun".to_string();
    }
    if adapter == "producer_compatible"
        || protocol_profile == "producer"
        || service_provider_key == "producer_platform"
        || payload_base_url.contains("flowmusic.app")
        || payload_base_url.contains("producer.ai")
        || label.contains("producer")
    {
        return "producer".to_string();
    }
    if adapter == "kiro_compatible"
        || protocol_profile == "kiro"
        || service_provider_key == "kiro_platform"
        || payload_base_url.contains("codewhisperer")
        || label.contains("kiro")
    {
        return "kiro".to_string();
    }
    if label.contains("codex") || payload_base_url.contains("/backend-api/codex") {
        return "codex".to_string();
    }
    if payload_base_url.contains("anthropic") {
        return "anthropic".to_string();
    }
    if payload_base_url.contains("openai") {
        return "openai".to_string();
    }
    if payload_base_url.contains("cohere") {
        return "cohere".to_string();
    }
    if payload_base_url.contains("gemini") || payload_base_url.contains("googleapis.com") {
        return "gemini".to_string();
    }

    sanitize_file_component(&provider_account.label)
}

fn derive_service_provider_slug(provider_account: &db::GatewayProviderAccountView) -> String {
    if !provider_account.service_provider_key.trim().is_empty() {
        return canonicalize_folder_service_provider_slug(&provider_account.service_provider_key);
    }
    canonicalize_folder_service_provider_slug(&provider_account.service_provider_label)
}

fn derive_gemini_provider_family_slug(
    adapter: &str,
    protocol_profile: &str,
    service_provider_key: &str,
    source_kind: &str,
    web_reverse_access_mode: &str,
    payload_base_url: &str,
) -> Option<String> {
    if adapter == "gemini_web_compatible"
        || protocol_profile == "gemini_web"
        || protocol_profile == GEMINI_WEB_REVERSE_MODULAR_PROFILE
        || (service_provider_key == "gemini_platform"
            && source_kind == "web_reverse_api"
            && web_reverse_access_mode == "direct_http_replay"
            && payload_base_url.contains("gemini.google.com"))
    {
        return Some("gemini-web-chat".to_string());
    }

    if matches!(
        protocol_profile,
        "google_gemini_api"
            | GEMINI_API_MODULAR_PROFILE
            | "google_vertex_gemini"
            | "gemini_business"
            | "gemini_canvas"
            | GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE
            | GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE
    ) || service_provider_key == "gemini_platform"
        || payload_base_url.contains("gemini")
        || payload_base_url.contains("googleapis.com")
    {
        return Some("gemini".to_string());
    }

    None
}

fn derive_legacy_gemini_canvas_surface_slug(label: &str) -> String {
    if label.contains("chat") || label.contains("tts") {
        "gemini-canvas-chat-tts".to_string()
    } else if label.contains("image") {
        "gemini-canvas-images".to_string()
    } else if label.contains("music") {
        "gemini-canvas-music".to_string()
    } else if label.contains("video") {
        "gemini-canvas-videos".to_string()
    } else {
        "gemini-canvas".to_string()
    }
}

fn derive_gemini_provider_surface_slug(protocol_profile: &str, label: &str) -> Option<String> {
    match protocol_profile {
        "google_gemini_api" => Some("google-gemini-api".to_string()),
        GEMINI_API_MODULAR_PROFILE => Some("google-gemini-api-modular".to_string()),
        "google_vertex_gemini" => Some("google-vertex-gemini".to_string()),
        "gemini_web" => Some("gemini-web-chat".to_string()),
        GEMINI_WEB_REVERSE_MODULAR_PROFILE => Some("gemini-web-chat-modular".to_string()),
        "gemini_business" => Some("gemini-business-images".to_string()),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE => {
            Some("gemini-canvas-browser-relay".to_string())
        }
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE => {
            Some("gemini-canvas-program-relay".to_string())
        }
        "gemini_canvas" => Some(derive_legacy_gemini_canvas_surface_slug(label)),
        _ => None,
    }
}

fn derive_provider_surface_slug(provider_account: &db::GatewayProviderAccountView) -> String {
    let protocol_profile = provider_account.protocol_profile.to_ascii_lowercase();
    let protocol_family = provider_account.protocol_family.to_ascii_lowercase();
    let service_provider_key = provider_account.service_provider_key.to_ascii_lowercase();
    let label = provider_account.label.to_ascii_lowercase();

    if service_provider_key == "gemini_platform" {
        if let Some(surface_slug) =
            derive_gemini_provider_surface_slug(protocol_profile.as_str(), label.as_str())
        {
            return surface_slug;
        }
    }

    match (service_provider_key.as_str(), protocol_profile.as_str()) {
        ("azure_openai_platform", "azure_openai") => "azure-openai".to_string(),
        ("anthropic_platform", "anthropic") => "anthropic-compatible".to_string(),
        ("aws_bedrock_platform", "aws_bedrock") => "bedrock-converse".to_string(),
        ("cohere_platform", "cohere") => "cohere-chat".to_string(),
        ("groq_platform", "groq") => "groq-openai".to_string(),
        ("nvidia_platform", "nvidia") => "nvidia-openai".to_string(),
        ("together_platform", "together") => "together-openai".to_string(),
        ("openrouter_platform", "openrouter") => "openrouter-openai".to_string(),
        ("muyuan_platform", "muyuan") => "muyuan-openai".to_string(),
        ("poe_platform", "poe") => "poe-openai".to_string(),
        ("longcat_platform", "longcat") => "longcat-openai".to_string(),
        ("deepseek_platform", "deepseek") => "deepseek-openai".to_string(),
        ("mistral_platform", "mistral") => "mistral-openai".to_string(),
        ("grok_platform", "grok_web") => "grok-web-reverse-api".to_string(),
        ("xai_platform", "xai") | ("xai_platform", "xai_openai") => "xai-openai".to_string(),
        ("perplexity_platform", "perplexity_search") => "perplexity-search".to_string(),
        ("perplexity_platform", "perplexity_chat") => "perplexity-chat".to_string(),
        ("tavily_platform", "tavily") => "tavily-search".to_string(),
        ("exa_platform", "exa") => "exa-search".to_string(),
        ("jina_platform", "jina_search") => "jina-search".to_string(),
        ("jina_platform", "jina_reader") => "jina-reader".to_string(),
        ("linkup_platform", "linkup") => "linkup-search".to_string(),
        ("you_platform", "you_search") => "you-search".to_string(),
        ("websearchapi_platform", "websearchapi") => "websearchapi-search".to_string(),
        ("qwen_platform", "qwen_dashscope_openai") => "qwen-dashscope-openai".to_string(),
        ("qwen_platform", "qwen_coding_plan_openai") => "qwen-coding-plan-openai".to_string(),
        ("qwen_platform", "qwen_coding_plan_anthropic") => "qwen-coding-plan-anthropic".to_string(),
        ("qwen_platform", "qwen_web_chat") => "qwen-web-chat".to_string(),
        ("chatgpt_platform", "chatgpt_official_api") => "chatgpt-official-api".to_string(),
        ("chatgpt_platform", "chatgpt_codex_backend") => "chatgpt-codex-backend".to_string(),
        ("chatgpt_platform", "chatgpt_web_reverse") => "chatgpt-web-reverse".to_string(),
        ("chataibot_platform", "chataibot") => "chataibot-images".to_string(),
        ("aistudio_platform", "aistudio_web_reverse") => "aistudio-web-reverse".to_string(),
        ("suno_platform", "suno") => "suno".to_string(),
        ("udio_platform", "udio") => "udio".to_string(),
        ("lumalabs_platform", "lumalabs") => "lumalabs".to_string(),
        ("freebuff_platform", "freebuff") => "freebuff-compatible".to_string(),
        ("xfyun_platform", "xfyun_openai") => "xfyun-openai".to_string(),
        ("xfyun_platform", "xfyun_native_websocket") => "xfyun-native-websocket".to_string(),
        ("producer_platform", "producer") => {
            if protocol_family.contains("image") || label.contains("image") {
                "producer-images".to_string()
            } else if protocol_family.contains("video") || label.contains("video") {
                "producer-videos".to_string()
            } else if protocol_family.contains("music") || label.contains("music") {
                "producer-music".to_string()
            } else {
                "producer".to_string()
            }
        }
        ("kiro_platform", "kiro") => "kiro-compatible".to_string(),
        _ if !provider_account.protocol_profile.trim().is_empty() => {
            sanitize_file_component(&provider_account.protocol_profile)
        }
        _ => derive_provider_family_slug(provider_account),
    }
}

fn canonicalize_folder_family_slug(value: &str) -> String {
    let slug = sanitize_file_component(value);
    match slug.as_str() {
        "accio-platform" | "accio-manager" | "phoenix-gw" => "accio".to_string(),
        "qwen"
        | "qwen-web"
        | "qwen-webui"
        | "qwen-web-chat"
        | "qwen-webui-replay"
        | "qwen-webui-replay-live" => "qwen-web-chat".to_string(),
        "aistudio" | "ai-studio" | "aistudio-web-reverse" => "aistudio-web-reverse".to_string(),
        "gemini-web" | "gemini-web-chat" => "gemini-web-chat".to_string(),
        "chatgpt" | "chatgpt-web" | "chatgpt-web-reverse" => "chatgpt-web-reverse".to_string(),
        "nvidia-platform" | "nvidia-nim" => "nvidia".to_string(),
        "grok-platform" | "grok-web" | "grok-web-reverse" => "grok".to_string(),
        "suno-platform" | "suno-music" | "suno-images" | "suno-videos" => "suno".to_string(),
        "udio-platform" | "udio-music" | "udio-images" | "udio-videos" => "udio".to_string(),
        "xai-platform" | "xai-openai" => "xai".to_string(),
        "perplexity-platform" | "perplexity-chat" | "perplexity-search" => "perplexity".to_string(),
        "freebuff-platform" | "freebuff-compatible" | "codebuff" => "freebuff".to_string(),
        "xfyun-platform" | "xfyun-openai" | "xfyun-native-websocket" | "xfyun-websocket" => {
            "xfyun".to_string()
        }
        "producer-platform" | "producer-images" | "producer-music" | "producer-videos" => {
            "producer".to_string()
        }
        "kiro-platform" | "kiro-compatible" => "kiro".to_string(),
        "lumalabs-platform" | "luma" | "luma-labs" | "luma-labs-images" | "luma-labs-videos"
        | "luma-labs-audio" | "lumalabs-images" | "lumalabs-videos" | "lumalabs-audio" => {
            "lumalabs".to_string()
        }
        _ => slug,
    }
}

fn canonicalize_folder_service_provider_slug(value: &str) -> String {
    let slug = sanitize_file_component(value);
    match slug.as_str() {
        "azure-openai" | "azure-openai-platform" | "azure-openai-service" => {
            "azure-openai-platform".to_string()
        }
        "anthropic" | "anthropic-platform" => "anthropic-platform".to_string(),
        "aws-bedrock" | "aws-bedrock-platform" | "bedrock-platform" => {
            "aws-bedrock-platform".to_string()
        }
        "cohere" | "cohere-platform" => "cohere-platform".to_string(),
        "groq" | "groq-platform" => "groq-platform".to_string(),
        "grok" | "grok-platform" | "grok-web" => "grok-platform".to_string(),
        "together" | "together-platform" => "together-platform".to_string(),
        "openrouter" | "openrouter-platform" => "openrouter-platform".to_string(),
        "deepseek" | "deepseek-platform" => "deepseek-platform".to_string(),
        "mistral" | "mistral-platform" => "mistral-platform".to_string(),
        "qwen" | "qwen-platform" => "qwen-platform".to_string(),
        "chatgpt" | "chatgpt-platform" => "chatgpt-platform".to_string(),
        "aistudio" | "ai-studio" | "aistudio-platform" => "aistudio-platform".to_string(),
        "gemini" | "gemini-platform" | "google-gemini" => "gemini-platform".to_string(),
        "suno" | "suno-platform" => "suno-platform".to_string(),
        "udio" | "udio-platform" => "udio-platform".to_string(),
        "xai" | "xai-platform" | "xai-openai" => "xai-platform".to_string(),
        "perplexity" | "perplexity-platform" | "perplexity-chat" | "perplexity-search" => {
            "perplexity-platform".to_string()
        }
        "freebuff" | "freebuff-platform" | "freebuff-compatible" | "codebuff" => {
            "freebuff-platform".to_string()
        }
        "xfyun" | "xfyun-platform" | "xfyun-openai" | "xfyun-native-websocket" => {
            "xfyun-platform".to_string()
        }
        "producer" | "producer-platform" | "producer-images" | "producer-music"
        | "producer-videos" => "producer-platform".to_string(),
        "kiro" | "kiro-platform" | "kiro-compatible" => "kiro-platform".to_string(),
        "luma" | "luma-labs" | "lumalabs" | "lumalabs-platform" => "lumalabs-platform".to_string(),
        _ => slug,
    }
}

fn canonicalize_folder_surface_slug(value: &str) -> String {
    let slug = sanitize_file_component(value);
    match slug.as_str() {
        "azure-openai" | "azure-openai-v1" => "azure-openai".to_string(),
        "anthropic-compatible" | "anthropic-messages" => "anthropic-compatible".to_string(),
        "bedrock-converse" | "aws-bedrock-converse" => "bedrock-converse".to_string(),
        "cohere-chat" | "cohere-chat-v2" | "cohere" => "cohere-chat".to_string(),
        "groq" | "groq-openai" => "groq-openai".to_string(),
        "nvidia" | "nvidia-openai" | "nvidia-nim" => "nvidia-openai".to_string(),
        "together" | "together-openai" => "together-openai".to_string(),
        "openrouter" | "openrouter-openai" => "openrouter-openai".to_string(),
        "deepseek" | "deepseek-openai" => "deepseek-openai".to_string(),
        "mistral" | "mistral-openai" => "mistral-openai".to_string(),
        "grok" | "grok-web" | "grok-web-reverse-api" => "grok-web-reverse-api".to_string(),
        "xai" | "xai-openai" => "xai-openai".to_string(),
        "perplexity-chat" | "perplexity-openai" => "perplexity-chat".to_string(),
        "perplexity-search" => "perplexity-search".to_string(),
        "freebuff" | "freebuff-compatible" | "codebuff" => "freebuff-compatible".to_string(),
        "xfyun" | "xfyun-openai" => "xfyun-openai".to_string(),
        "xfyun-native" | "xfyun-websocket" | "xfyun-native-websocket" => {
            "xfyun-native-websocket".to_string()
        }
        "producer-images" | "producer-image" => "producer-images".to_string(),
        "producer-music" => "producer-music".to_string(),
        "producer-videos" | "producer-video" => "producer-videos".to_string(),
        "producer-web-reverse-api" => "producer".to_string(),
        "kiro" | "kiro-compatible" => "kiro-compatible".to_string(),
        "qwen-dashscope" | "qwen-dashscope-openai" => "qwen-dashscope-openai".to_string(),
        "qwen-coding-plan-openai" | "qwen-coding-openai" => "qwen-coding-plan-openai".to_string(),
        "qwen-coding-plan-anthropic" | "qwen-coding-anthropic" => {
            "qwen-coding-plan-anthropic".to_string()
        }
        "chatgpt-official-api" | "openai-platform" => "chatgpt-official-api".to_string(),
        "chatgpt-codex-backend" => "chatgpt-codex-backend".to_string(),
        "chataibot" | "chataibot-images" => "chataibot-images".to_string(),
        "aistudio" | "ai-studio" | "aistudio-web-reverse" => "aistudio-web-reverse".to_string(),
        "google-gemini-api" | "gemini-api" => "google-gemini-api".to_string(),
        "google-gemini-api-modular" | "gemini-api-modular" => {
            "google-gemini-api-modular".to_string()
        }
        "google-vertex-gemini" | "vertex-gemini" => "google-vertex-gemini".to_string(),
        "gemini-business" | "gemini-business-images" => "gemini-business-images".to_string(),
        "gemini-web" | "gemini-web-chat" => "gemini-web-chat".to_string(),
        "gemini-web-chat-modular" | "gemini-web-modular" => "gemini-web-chat-modular".to_string(),
        "gemini-canvas-chat" | "gemini-canvas-chat-tts" => "gemini-canvas-chat-tts".to_string(),
        "gemini-canvas-browser-relay" | "gemini-canvas-web-reverse" => {
            "gemini-canvas-browser-relay".to_string()
        }
        "gemini-canvas-program-relay" | "gemini-canvas-program-web-reverse" => {
            "gemini-canvas-program-relay".to_string()
        }
        "gemini-canvas-images" | "gemini-canvas-image" => "gemini-canvas-images".to_string(),
        "gemini-canvas-music" => "gemini-canvas-music".to_string(),
        "gemini-canvas-video" | "gemini-canvas-videos" => "gemini-canvas-videos".to_string(),
        "suno" | "suno-music" | "suno-images" | "suno-videos" | "suno-web-reverse-api" => {
            "suno".to_string()
        }
        "udio" | "udio-music" | "udio-images" | "udio-videos" | "udio-web-reverse-api" => {
            "udio".to_string()
        }
        "luma"
        | "luma-labs"
        | "luma-labs-images"
        | "luma-labs-videos"
        | "luma-labs-audio"
        | "lumalabs"
        | "lumalabs-images"
        | "lumalabs-videos"
        | "lumalabs-audio"
        | "lumalabs-web-reverse-api" => "lumalabs".to_string(),
        _ => canonicalize_folder_family_slug(slug.as_str()),
    }
}

fn canonicalize_credential_material_kind(value: &str) -> String {
    let slug = sanitize_file_component(value);
    match slug.as_str() {
        "api-key" | "apikey" => "api_key".to_string(),
        "bearer" | "bearer-token" => "bearer_token".to_string(),
        "session" | "session-auth" | "web-session" | "cookie-session" => "session_auth".to_string(),
        "browser-state" | "browser-profile" | "storage-state" => "browser_state".to_string(),
        "jwt-widget-session" | "widget-session" | "jwt-session" => "jwt_widget_session".to_string(),
        "access-token" => "access_token".to_string(),
        _ => slug.replace('-', "_"),
    }
}

fn service_surface_lookup_key(service_provider_slug: &str, provider_surface_slug: &str) -> String {
    format!(
        "{}::{}",
        canonicalize_folder_service_provider_slug(service_provider_slug),
        canonicalize_folder_surface_slug(provider_surface_slug)
    )
}

fn provider_family_selection_score(
    provider_account: &db::GatewayProviderAccountView,
    family_slug: &str,
) -> i32 {
    let family_slug = canonicalize_folder_family_slug(family_slug);
    let payload_base_url = provider_account
        .payload
        .get("baseUrl")
        .and_then(Value::as_str)
        .or_else(|| {
            provider_account
                .payload
                .get("base_url")
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .to_ascii_lowercase();
    let label = provider_account.label.to_ascii_lowercase();

    match family_slug.as_str() {
        "accio" => {
            let mut score = 0;
            if provider_account
                .adapter
                .eq_ignore_ascii_case("accio_compatible")
            {
                score += 100;
            }
            if provider_account
                .protocol_profile
                .eq_ignore_ascii_case("accio")
            {
                score += 50;
            }
            if provider_account
                .service_provider_key
                .eq_ignore_ascii_case("accio_platform")
            {
                score += 20;
            }
            if payload_base_url.contains("phoenix-gw.alibaba.com") {
                score += 25;
            }
            if label.contains("accio") {
                score += 10;
            }
            score
        }
        "codex" => {
            let mut score = 0;
            if payload_base_url.contains("/backend-api/codex") {
                score += 100;
            }
            if label.contains("codex") {
                score += 20;
            }
            if provider_account.protocol_family == "openai" {
                score += 5;
            }
            score
        }
        "qwen-web-chat" => {
            let mut score = 0;
            if provider_account
                .adapter
                .eq_ignore_ascii_case("qwen_web_compatible")
            {
                score += 100;
            }
            if provider_account
                .protocol_profile
                .eq_ignore_ascii_case("qwen_web_chat")
            {
                score += 50;
            }
            if provider_account
                .protocol_family
                .eq_ignore_ascii_case("qwen_web_chat")
            {
                score += 25;
            }
            if provider_account
                .service_provider_key
                .eq_ignore_ascii_case("qwen_platform")
            {
                score += 15;
            }
            if label.contains("qwen") && label.contains("web") {
                score += 10;
            }
            if payload_base_url.contains("chat.qwen.ai") {
                score += 20;
            }
            score
        }
        _ => {
            let mut score = 0;
            if label.contains(family_slug.as_str()) {
                score += 10;
            }
            if payload_base_url.contains(family_slug.as_str()) {
                score += 10;
            }
            score
        }
    }
}

fn provider_surface_selection_score(
    provider_account: &db::GatewayProviderAccountView,
    surface_slug: &str,
) -> i32 {
    let surface_slug = canonicalize_folder_surface_slug(surface_slug);
    let derived_surface_slug = derive_provider_surface_slug(provider_account);
    let protocol_profile_slug = sanitize_file_component(&provider_account.protocol_profile);
    let label = provider_account.label.to_ascii_lowercase();

    let mut score = 0;
    if derived_surface_slug == surface_slug {
        score += 100;
    }
    if protocol_profile_slug == surface_slug {
        score += 30;
    }
    if label.contains(surface_slug.as_str()) {
        score += 10;
    }
    score
}

fn default_folder_sync_relative_path(
    provider_account: &db::GatewayProviderAccountView,
    credential: &db::GatewayProviderCredentialView,
) -> String {
    if provider_account
        .service_provider_key
        .eq_ignore_ascii_case("azure_openai_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("anthropic_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("aws_bedrock_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("cohere_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("qwen_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("gemini_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("chataibot_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("suno_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("udio_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("lumalabs_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("xai_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("perplexity_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("freebuff_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("xfyun_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("producer_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("kiro_platform")
    {
        return format!(
            "{}/{}/{}/{}.json",
            derive_service_provider_slug(provider_account),
            derive_provider_surface_slug(provider_account),
            material_kind_folder_slug(derive_credential_material_kind(
                provider_account,
                Some(&credential.payload),
            )),
            sanitize_file_component(&credential.id)
        );
    }

    format!(
        "{}/{}.json",
        derive_provider_family_slug(provider_account),
        sanitize_file_component(&credential.id)
    )
}

fn materialize_export_payload(
    provider_account: &db::GatewayProviderAccountView,
    payload: &Value,
) -> Value {
    let mut payload_map = payload.as_object().cloned().unwrap_or_default();
    apply_folder_sync_metadata(&mut payload_map, provider_account, None);
    Value::Object(payload_map)
}

fn derive_credential_material_kind(
    provider_account: &db::GatewayProviderAccountView,
    payload: Option<&Value>,
) -> String {
    if let Some(raw_map) = payload.and_then(Value::as_object) {
        if let Some(kind) = raw_map
            .get("credentialMaterialKind")
            .or_else(|| raw_map.get("credential_material_kind"))
            .or_else(|| raw_map.get("materialKind"))
            .or_else(|| raw_map.get("material_kind"))
            .and_then(Value::as_str)
        {
            let trimmed = kind.trim();
            if !trimmed.is_empty() {
                return canonicalize_credential_material_kind(trimmed);
            }
        }
        if raw_map.contains_key("runtimeStateObjectKey")
            || raw_map.contains_key("runtime_state_object_key")
        {
            return "browser_state".to_string();
        }
        if raw_map
            .get("extraBody")
            .or_else(|| raw_map.get("extra_body"))
            .and_then(Value::as_object)
            .is_some_and(|extra| {
                extra.contains_key("configId")
                    || extra.contains_key("config_id")
                    || extra.contains_key("session")
            })
        {
            return "jwt_widget_session".to_string();
        }
    }

    if let Some(kind) = default_credential_material_kind_for_protocol_profile(
        provider_account.protocol_profile.as_str(),
    ) {
        return kind.to_string();
    }

    match provider_account.protocol_profile.as_str() {
        "qwen_dashscope_openai" | "qwen_coding_plan_openai" | "qwen_coding_plan_anthropic" => {
            "api_key".to_string()
        }
        "chatgpt_web_reverse" => "session_auth".to_string(),
        "qwen_web_chat" | "accio" | "chataibot" | "suno" | "udio" | "lumalabs" | "freebuff"
        | "producer" => "session_auth".to_string(),
        _ => "api_key".to_string(),
    }
}

fn default_credential_material_kind_for_protocol_profile(
    protocol_profile: &str,
) -> Option<&'static str> {
    match protocol_profile {
        "azure_openai" => Some("api_key"),
        "anthropic" => Some("api_key"),
        "aws_bedrock" => Some("bearer_token"),
        "cohere" => Some("api_key"),
        "google_gemini_api" | GEMINI_API_MODULAR_PROFILE => Some("api_key"),
        "google_vertex_gemini" => Some("bearer_token"),
        "gemini_web" | GEMINI_WEB_REVERSE_MODULAR_PROFILE => Some("session_auth"),
        "gemini_business" => Some("jwt_widget_session"),
        "gemini_canvas"
        | GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE
        | GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE => Some("browser_state"),
        "xai" | "xai_openai" | "perplexity_chat" | "xfyun_openai" | "xfyun_native_websocket" => {
            Some("api_key")
        }
        "freebuff" | "producer" => Some("session_auth"),
        "kiro" => Some("bearer_token"),
        _ => None,
    }
}

fn material_kind_folder_slug(kind: String) -> String {
    kind.replace('_', "-")
}

fn normalize_import_payload(
    surface_slug: &str,
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let surface_slug = canonicalize_folder_surface_slug(surface_slug);
    if surface_slug == "codex" {
        return normalize_codex_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "chatgpt-codex-backend" {
        return normalize_codex_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "accio" {
        return normalize_accio_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "qwen-web-chat" {
        return normalize_qwen_web_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "chataibot-images" {
        return normalize_chataibot_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "gemini-web-chat" {
        return normalize_gemini_web_import_payload(
            provider_account,
            raw_payload,
            credential_material_kind_hint,
        );
    }
    if surface_slug == "chatgpt-web-reverse" {
        return normalize_chatgpt_web_import_payload(
            provider_account,
            raw_payload,
            credential_material_kind_hint,
        );
    }
    if surface_slug == "gemini-business-images" {
        return normalize_gemini_business_import_payload(
            provider_account,
            raw_payload,
            credential_material_kind_hint,
        );
    }
    if matches!(
        surface_slug.as_str(),
        "gemini-canvas"
            | "gemini-canvas-chat-tts"
            | "gemini-canvas-browser-relay"
            | "gemini-canvas-program-relay"
            | "gemini-canvas-images"
            | "gemini-canvas-music"
            | "gemini-canvas-videos"
    ) {
        return normalize_gemini_canvas_import_payload(
            provider_account,
            raw_payload,
            credential_material_kind_hint,
        );
    }

    let Some(mut payload) = raw_payload.as_object().cloned() else {
        return Err(GatewayError::bad_request(
            "provider credential payload 必须是 JSON object",
        ));
    };
    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint,
    );
    Ok(Value::Object(payload))
}

fn normalize_codex_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "codex provider credential payload 必须是 JSON object",
        ));
    };

    let access_token = raw_map
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            chatgpt_web_import_string(
                raw_map,
                &[
                    &["accessToken"],
                    &["apiKey"],
                    &["api_key"],
                    &["token"],
                    &["chatgptLoginDetails", "clientBootstrap", "accessToken"],
                ],
            )
        });
    let access_token_claims = access_token.and_then(decode_jwt_claims);
    let account_id = raw_map
        .get("account_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            chatgpt_web_import_string(
                raw_map,
                &[
                    &["accountId"],
                    &["chatgptLoginDetails", "clientBootstrap", "accountId"],
                    &["chatgptLogin", "workspaceId"],
                    &["chatgptLogin", "personalWorkspaceId"],
                    &["workspaceId"],
                    &["personalWorkspaceId"],
                ],
            )
        })
        .or_else(|| {
            access_token_claims.as_ref().and_then(|claims| {
                json_string_at_path(
                    claims,
                    &["https://api.openai.com/auth", "chatgpt_account_id"],
                )
            })
        });

    if access_token.is_none() || account_id.is_none() {
        return Ok(raw_payload);
    }

    let base_url = provider_account
        .payload
        .get("base_url")
        .and_then(Value::as_str)
        .or_else(|| {
            provider_account
                .payload
                .get("baseUrl")
                .and_then(Value::as_str)
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("https://chatgpt.com/backend-api/codex");
    let default_model = provider_account
        .payload
        .get("default_model")
        .cloned()
        .or_else(|| provider_account.payload.get("defaultModel").cloned())
        .unwrap_or_else(|| Value::String("gpt-5.4".to_string()));
    let responses_path = provider_account
        .payload
        .get("responses_path")
        .cloned()
        .or_else(|| provider_account.payload.get("responsesPath").cloned())
        .unwrap_or_else(|| Value::String("/responses".to_string()));

    Ok(serde_json::json!({
        "adapter": "openai_compatible",
        "apiKey": access_token,
        "baseUrl": base_url,
        "defaultModel": default_model,
        "responsesPath": responses_path,
        "headers": {
            "Chatgpt-Account-Id": account_id,
            "Originator": "codex_cli_rs",
            "User-Agent": "codex_cli_rs/0.1.2504151532"
        },
        "extraBody": {
            "store": false
        },
        "rawSource": canonicalize_folder_sync_raw_source(&Value::Object(raw_map.clone()))
    }))
}

fn normalize_chatgpt_web_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "chatgpt web reverse provider credential payload 必须是 JSON object",
        ));
    };

    let access_token = chatgpt_web_import_string(
        raw_map,
        &[
            &["accessToken"],
            &["access_token"],
            &["apiKey"],
            &["api_key"],
            &["token"],
            &["chatgptLoginDetails", "clientBootstrap", "accessToken"],
        ],
    )
    .ok_or_else(|| {
        GatewayError::bad_request(
            "chatgpt web reverse provider credential payload 缺少 access token",
        )
    })?;
    let refresh_token = chatgpt_web_import_string(
        raw_map,
        &[
            &["refreshToken"],
            &["refresh_token"],
            &["oauthRefreshToken"],
            &["openaiRefreshToken"],
            &["oauthTokens", "refreshToken"],
            &["oauthTokens", "refresh_token"],
            &["tokens", "refreshToken"],
            &["tokens", "refresh_token"],
            &["chatgptLoginDetails", "refreshToken"],
            &["chatgptLoginDetails", "refresh_token"],
            &["chatgptLoginDetails", "oauthTokens", "refreshToken"],
            &["chatgptLoginDetails", "oauthTokens", "refresh_token"],
            &["platformAuth", "refreshToken"],
            &["platformAuth", "refresh_token"],
        ],
    );
    let id_token = chatgpt_web_import_string(
        raw_map,
        &[
            &["idToken"],
            &["id_token"],
            &["oauthIdToken"],
            &["openaiIdToken"],
            &["oauthTokens", "idToken"],
            &["oauthTokens", "id_token"],
            &["tokens", "idToken"],
            &["tokens", "id_token"],
            &["chatgptLoginDetails", "idToken"],
            &["chatgptLoginDetails", "id_token"],
            &["chatgptLoginDetails", "oauthTokens", "idToken"],
            &["chatgptLoginDetails", "oauthTokens", "id_token"],
            &["platformAuth", "idToken"],
            &["platformAuth", "id_token"],
        ],
    );
    let access_token_claims = decode_jwt_claims(access_token);

    let base_url = provider_account
        .payload
        .get("base_url")
        .and_then(Value::as_str)
        .or_else(|| {
            provider_account
                .payload
                .get("baseUrl")
                .and_then(Value::as_str)
        })
        .filter(|value| !value.trim().is_empty())
        .map(crate::protocol::chatgpt::web_reverse::normalize_site_base_url)
        .unwrap_or_else(|| "https://chatgpt.com".to_string());
    let default_model = provider_account
        .payload
        .get("default_model")
        .cloned()
        .or_else(|| provider_account.payload.get("defaultModel").cloned())
        .unwrap_or_else(|| Value::String("gpt-5.4".to_string()));
    let expires_at = raw_map
        .get("expires")
        .or_else(|| raw_map.get("expiresAt"))
        .or_else(|| raw_map.get("expires_at"))
        .cloned()
        .or_else(|| {
            access_token_claims
                .as_ref()
                .and_then(|claims| claims.get("exp"))
                .and_then(Value::as_i64)
                .and_then(|exp| OffsetDateTime::from_unix_timestamp(exp).ok())
                .and_then(|timestamp| timestamp.format(&Rfc3339).ok())
                .map(Value::String)
        })
        .unwrap_or(Value::Null);

    let mut payload = serde_json::Map::new();
    payload.insert(
        "adapter".to_string(),
        Value::String("chatgpt_web_reverse_compatible".to_string()),
    );
    payload.insert(
        "apiKey".to_string(),
        Value::String(access_token.to_string()),
    );
    payload.insert("baseUrl".to_string(), Value::String(base_url.to_string()));
    payload.insert("defaultModel".to_string(), default_model);
    payload.insert(
        "sessionAuth".to_string(),
        serde_json::json!({
            "transport": "bearer",
            "headerName": "authorization"
        }),
    );
    if !expires_at.is_null() {
        payload.insert("expiresAt".to_string(), expires_at);
    }
    payload.insert(
        "headers".to_string(),
        serde_json::json!({
            "Origin": "https://chatgpt.com",
            "Referer": "https://chatgpt.com/",
            "Accept-Language": "zh-CN,zh;q=0.9,en;q=0.8,en-US;q=0.7",
            "User-Agent": crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_USER_AGENT,
        }),
    );
    let mut extra_body = serde_json::Map::new();
    extra_body.insert(
        "clientVersion".to_string(),
        Value::String(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_CLIENT_VERSION.to_string(),
        ),
    );
    extra_body.insert(
        "clientBuildNumber".to_string(),
        Value::String(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_CLIENT_BUILD_NUMBER
                .to_string(),
        ),
    );
    extra_body.insert(
        "timezone".to_string(),
        Value::String(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_TIMEZONE.to_string(),
        ),
    );
    if let Some(pow_sources) = raw_map
        .get("chatgptPowSources")
        .or_else(|| raw_map.get("powSources"))
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        extra_body.insert(
            "chatgptPowSources".to_string(),
            Value::Array(pow_sources.clone()),
        );
    }
    if let Some(pow_data_build) = raw_map
        .get("chatgptPowDataBuild")
        .or_else(|| raw_map.get("powDataBuild"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        extra_body.insert(
            "chatgptPowDataBuild".to_string(),
            Value::String(pow_data_build.to_string()),
        );
    }
    if let Some(refresh_token) = refresh_token {
        extra_body.insert(
            "refreshToken".to_string(),
            Value::String(refresh_token.to_string()),
        );
        extra_body.insert(
            "refreshStrategy".to_string(),
            Value::String("oauth_token".to_string()),
        );
        extra_body.insert(
            "oauthTokenEndpoint".to_string(),
            Value::String("https://auth.openai.com/oauth/token".to_string()),
        );
        extra_body.insert(
            "oauthClientId".to_string(),
            Value::String("app_2SKx67EdpoN0G6j64rFvigXD".to_string()),
        );
        payload.insert(
            "refreshToken".to_string(),
            Value::String(refresh_token.to_string()),
        );
    }
    if let Some(id_token) = id_token {
        extra_body.insert("idToken".to_string(), Value::String(id_token.to_string()));
        payload.insert("idToken".to_string(), Value::String(id_token.to_string()));
    }
    if let Some(device_id) = chatgpt_web_import_string(
        raw_map,
        &[
            &["deviceId"],
            &["oaiDeviceId"],
            &["chatgptLogin", "deviceId"],
            &["platformAuth", "deviceId"],
        ],
    ) {
        extra_body.insert("deviceId".to_string(), Value::String(device_id.to_string()));
    } else {
        extra_body.insert(
            "deviceId".to_string(),
            Value::String(uuid::Uuid::new_v4().to_string()),
        );
    }
    if let Some(session_id) =
        chatgpt_web_import_string(raw_map, &[&["sessionId"], &["oaiSessionId"]]).or_else(|| {
            access_token_claims
                .as_ref()
                .and_then(|claims| claims.get("session_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
    {
        extra_body.insert(
            "sessionId".to_string(),
            Value::String(session_id.to_string()),
        );
    } else {
        extra_body.insert(
            "sessionId".to_string(),
            Value::String(uuid::Uuid::new_v4().to_string()),
        );
    }
    if let Some(auth_url) = chatgpt_web_import_string(
        raw_map,
        &[
            &["chatgptLogin", "authUrl"],
            &["authUrl"],
            &["chatgptAuthUrl"],
        ],
    ) {
        extra_body.insert(
            "chatgptAuthUrl".to_string(),
            Value::String(auth_url.to_string()),
        );
    }
    if let Some(mailbox_ref) =
        chatgpt_web_import_string(raw_map, &[&["chatgptLogin", "mailboxRef"], &["mailboxRef"]])
    {
        extra_body.insert(
            "mailboxRef".to_string(),
            Value::String(mailbox_ref.to_string()),
        );
    }
    if let Some(mailbox_session_id) = chatgpt_web_import_string(
        raw_map,
        &[&["chatgptLogin", "mailboxSessionId"], &["mailboxSessionId"]],
    ) {
        extra_body.insert(
            "mailboxSessionId".to_string(),
            Value::String(mailbox_session_id.to_string()),
        );
    }
    if let Some(registration_ip_country) = chatgpt_web_import_string(
        raw_map,
        &[
            &[
                "platformOrganizationDetails",
                "onboardingLogin",
                "ip_country",
            ],
            &["ip_country"],
            &["ipCountry"],
        ],
    ) {
        extra_body.insert(
            "registrationIpCountry".to_string(),
            Value::String(registration_ip_country.to_string()),
        );
    }
    if let Some(proxy_url) = chatgpt_web_import_string(
        raw_map,
        &[
            &["proxyUrl"],
            &["proxy_url"],
            &["credentialProxyUrl"],
            &["credential_proxy_url"],
            &["outboundProxy"],
            &["outbound_proxy"],
        ],
    ) {
        extra_body.insert("proxyUrl".to_string(), Value::String(proxy_url.to_string()));
    }
    if let Some(proxy_bypass) = chatgpt_web_import_string(
        raw_map,
        &[
            &["proxyBypass"],
            &["proxy_bypass"],
            &["noProxy"],
            &["no_proxy"],
        ],
    ) {
        extra_body.insert(
            "proxyBypass".to_string(),
            Value::String(proxy_bypass.to_string()),
        );
    }
    let auth_seed_email = chatgpt_web_import_string(
        raw_map,
        &[
            &["email"],
            &["loginEmail"],
            &["login_email"],
            &["accountEmail"],
        ],
    );
    let auth_seed_password = chatgpt_web_import_string(
        raw_map,
        &[&["password"], &["loginPassword"], &["login_password"]],
    );
    let auth_seed_password_sha256 = chatgpt_web_import_string(
        raw_map,
        &[
            &["passwordSha256"],
            &["password_sha256"],
            &["passwordHash"],
            &["password_hash"],
        ],
    );
    if let Some(email) = auth_seed_email {
        let mut auth_seed = serde_json::Map::new();
        auth_seed.insert(
            "type".to_string(),
            Value::String("chatgpt_web_signin".to_string()),
        );
        auth_seed.insert("email".to_string(), Value::String(email.to_string()));
        if let Some(password) = auth_seed_password {
            auth_seed.insert("password".to_string(), Value::String(password.to_string()));
        }
        if let Some(password_sha256) = auth_seed_password_sha256 {
            auth_seed.insert(
                "passwordSha256".to_string(),
                Value::String(password_sha256.to_string()),
            );
        }
        extra_body.insert("authSeed".to_string(), Value::Object(auth_seed));
    }
    payload.insert("extraBody".to_string(), Value::Object(extra_body));
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&Value::Object(raw_map.clone())),
    );
    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint.or(Some("session_auth")),
    );
    Ok(Value::Object(payload))
}

fn chatgpt_web_import_string<'a>(
    raw_map: &'a serde_json::Map<String, Value>,
    candidate_paths: &[&[&str]],
) -> Option<&'a str> {
    candidate_paths.iter().find_map(|path| {
        let mut current = raw_map.get(*path.first()?)?;
        for segment in &path[1..] {
            current = current.get(*segment)?;
        }
        current
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn json_string_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
}

fn decode_jwt_claims(token: &str) -> Option<Value> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let claims = parts.next()?;
    let decoded = URL_SAFE_NO_PAD.decode(claims.as_bytes()).ok()?;
    serde_json::from_slice::<Value>(&decoded).ok()
}

fn normalize_qwen_web_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "qwen web provider credential payload 必须是 JSON object",
        ));
    };

    let auth_token = raw_map
        .get("apiKey")
        .or_else(|| raw_map.get("api_key"))
        .or_else(|| raw_map.get("authToken"))
        .or_else(|| raw_map.get("auth_token"))
        .or_else(|| raw_map.get("active_token"))
        .or_else(|| raw_map.get("token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::bad_request("qwen web provider credential payload 缺少 auth token")
        })?;

    let cookie_header = raw_map
        .get("cookieHeader")
        .or_else(|| raw_map.get("cookie_header"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| {
                    headers
                        .get("Cookie")
                        .or_else(|| headers.get("cookie"))
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    let expires_at = raw_map
        .get("expiresAt")
        .or_else(|| raw_map.get("expires_at"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let selected_model = raw_map
        .get("selectedModel")
        .or_else(|| raw_map.get("selected_model"))
        .or_else(|| raw_map.get("model"))
        .or_else(|| raw_map.get("defaultModel"))
        .or_else(|| raw_map.get("default_model"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("supportedModels")
                .and_then(Value::as_array)
                .and_then(|models| models.first())
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            provider_account
                .payload
                .get("defaultModel")
                .or_else(|| provider_account.payload.get("default_model"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    let selected_display_model = raw_map
        .get("selectedDisplayModel")
        .or_else(|| raw_map.get("displayModel"))
        .or_else(|| raw_map.get("display_model"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let account_name = raw_map
        .get("accountName")
        .or_else(|| raw_map.get("account_name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("authProbe")
                .and_then(Value::as_object)
                .and_then(|probe| {
                    probe
                        .get("email")
                        .or_else(|| probe.get("userId"))
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    let credential_material_key = raw_map
        .get("credentialMaterialKey")
        .or_else(|| raw_map.get("materialKey"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("authProbe")
                .and_then(Value::as_object)
                .and_then(|probe| probe.get("userId").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|user_id| format!("qwen-web-user:{user_id}"))
        });

    let auth_seed_value = raw_map
        .get("authSeed")
        .or_else(|| raw_map.get("auth_seed"))
        .cloned()
        .or_else(|| {
            let email = raw_map
                .get("email")
                .or_else(|| raw_map.get("loginEmail"))
                .or_else(|| raw_map.get("login_email"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let password = raw_map
                .get("password")
                .or_else(|| raw_map.get("loginPassword"))
                .or_else(|| raw_map.get("login_password"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let password_sha256 = raw_map
                .get("passwordSha256")
                .or_else(|| raw_map.get("password_sha256"))
                .or_else(|| raw_map.get("passwordHash"))
                .or_else(|| raw_map.get("password_hash"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);

            if email.is_none() && password.is_none() && password_sha256.is_none() {
                return None;
            }

            let mut auth_seed = serde_json::Map::new();
            auth_seed.insert(
                "type".to_string(),
                Value::String("qwen_web_signin".to_string()),
            );
            if let Some(email) = email {
                auth_seed.insert("email".to_string(), Value::String(email));
            }
            if let Some(password) = password {
                auth_seed.insert("password".to_string(), Value::String(password));
            }
            if let Some(password_sha256) = password_sha256 {
                auth_seed.insert("passwordSha256".to_string(), Value::String(password_sha256));
            }
            Some(Value::Object(auth_seed))
        });

    let mut payload = serde_json::Map::new();
    payload.insert("apiKey".to_string(), Value::String(auth_token.to_string()));
    if let Some(expires_at) = expires_at {
        payload.insert("expiresAt".to_string(), Value::String(expires_at));
    }
    if let Some(cookie_header) = cookie_header {
        payload.insert(
            "headers".to_string(),
            serde_json::json!({
                "Cookie": cookie_header
            }),
        );
    }
    if let Some(selected_model) = selected_model {
        payload.insert(
            "supportedModels".to_string(),
            Value::Array(vec![Value::String(selected_model)]),
        );
    }
    if let Some(selected_display_model) = selected_display_model {
        payload.insert(
            "selectedDisplayModel".to_string(),
            Value::String(selected_display_model),
        );
    }
    if let Some(account_name) = account_name {
        payload.insert("accountName".to_string(), Value::String(account_name));
    }
    if let Some(credential_material_key) = credential_material_key {
        payload.insert(
            "credentialMaterialKey".to_string(),
            Value::String(credential_material_key),
        );
    }
    if let Some(auth_seed) = auth_seed_value {
        let mut extra_body = serde_json::Map::new();
        extra_body.insert("authSeed".to_string(), auth_seed);
        payload.insert("extraBody".to_string(), Value::Object(extra_body));
    }
    apply_folder_sync_metadata(&mut payload, provider_account, Some("session_auth"));
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}

fn normalize_chataibot_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "chataibot image provider credential payload 必须是 JSON object",
        ));
    };

    let cookie_header = read_optional_object_string(raw_map, &["cookieHeader", "cookie_header"])
        .or_else(|| {
            raw_map
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| {
                    headers
                        .get("Cookie")
                        .or_else(|| headers.get("cookie"))
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    let auth_token = read_optional_object_string(
        raw_map,
        &["authToken", "auth_token", "apiKey", "api_key", "token"],
    )
    .or_else(|| {
        cookie_header
            .as_deref()
            .and_then(|cookie| extract_cookie_value(cookie, "token"))
            .map(str::to_string)
    })
    .ok_or_else(|| {
        GatewayError::bad_request("chataibot image provider credential payload 缺少 auth token")
    })?;

    let normalized_cookie_header = match cookie_header {
        Some(cookie) => {
            let trimmed = cookie.trim();
            if trimmed.is_empty() {
                Some(format!("token={auth_token}"))
            } else if trimmed.to_ascii_lowercase().contains("token=") {
                Some(trimmed.to_string())
            } else {
                Some(format!("token={auth_token}; {trimmed}"))
            }
        }
        None => Some(format!("token={auth_token}")),
    };

    let expires_at = read_optional_object_string(raw_map, &["expiresAt", "expires_at"]);
    let supported_models =
        read_optional_object_string_array(raw_map, &["supportedModels", "supported_models"])
            .or_else(|| {
                read_optional_object_string(
                    raw_map,
                    &[
                        "selectedModel",
                        "selected_model",
                        "model",
                        "defaultModel",
                        "default_model",
                    ],
                )
                .map(|model| vec![model])
            })
            .or_else(|| {
                provider_account
                    .payload
                    .get("defaultModel")
                    .or_else(|| provider_account.payload.get("default_model"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|model| vec![model.to_string()])
            });
    let selected_display_model = read_optional_object_string(
        raw_map,
        &["selectedDisplayModel", "displayModel", "display_model"],
    );
    let account_name = read_optional_object_string(raw_map, &["accountName", "account_name"])
        .or_else(|| read_optional_object_string(raw_map, &["email", "userId", "user_id"]));
    let credential_material_key =
        read_optional_object_string(raw_map, &["credentialMaterialKey", "materialKey"]).or_else(
            || {
                read_optional_object_string(raw_map, &["userId", "user_id"])
                    .map(|user_id| format!("chataibot-user:{user_id}"))
            },
        );

    let mut payload = serde_json::Map::new();
    payload.insert("apiKey".to_string(), Value::String(auth_token));
    if let Some(cookie_header) = normalized_cookie_header {
        payload.insert(
            "headers".to_string(),
            serde_json::json!({
                "Cookie": cookie_header
            }),
        );
    }
    if let Some(expires_at) = expires_at {
        payload.insert("expiresAt".to_string(), Value::String(expires_at));
    }
    if let Some(supported_models) = supported_models {
        payload.insert(
            "supportedModels".to_string(),
            Value::Array(
                supported_models
                    .into_iter()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
    if let Some(selected_display_model) = selected_display_model {
        payload.insert(
            "selectedDisplayModel".to_string(),
            Value::String(selected_display_model),
        );
    }
    if let Some(account_name) = account_name {
        payload.insert("accountName".to_string(), Value::String(account_name));
    }
    if let Some(credential_material_key) = credential_material_key {
        payload.insert(
            "credentialMaterialKey".to_string(),
            Value::String(credential_material_key),
        );
    }
    apply_folder_sync_metadata(&mut payload, provider_account, Some("session_auth"));
    payload.insert(
        "providerSurfaceKey".to_string(),
        Value::String("chataibot-images".to_string()),
    );
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}

fn normalize_gemini_web_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "gemini web provider credential payload 必须是 JSON object",
        ));
    };

    let api_key = raw_map
        .get("apiKey")
        .or_else(|| raw_map.get("api_key"))
        .or_else(|| raw_map.get("secure_1psid"))
        .or_else(|| raw_map.get("__Secure-1PSID"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::bad_request("gemini web provider credential payload 缺少 __Secure-1PSID")
        })?;

    let auth_token = raw_map
        .get("authToken")
        .or_else(|| raw_map.get("auth_token"))
        .or_else(|| raw_map.get("secure_1psidts"))
        .or_else(|| raw_map.get("__Secure-1PSIDTS"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let cookie_header = raw_map
        .get("cookieHeader")
        .or_else(|| raw_map.get("cookie_header"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| {
                    headers
                        .get("Cookie")
                        .or_else(|| headers.get("cookie"))
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    let default_model = read_optional_object_string(
        raw_map,
        &[
            "defaultModel",
            "default_model",
            "model",
            "selectedModel",
            "selected_model",
        ],
    );
    let credential_material_key = read_optional_object_string(
        raw_map,
        &[
            "credentialMaterialKey",
            "credential_material_key",
            "materialKey",
            "material_key",
        ],
    )
    .or_else(|| {
        read_optional_object_string(raw_map, &["accountName", "account_name"])
            .map(|value| format!("gemini-web-session:{value}"))
    });

    let mut payload = serde_json::Map::new();
    payload.insert("apiKey".to_string(), Value::String(api_key.to_string()));
    if let Some(auth_token) = auth_token {
        payload.insert("authToken".to_string(), Value::String(auth_token));
    }
    if let Some(default_model) = default_model {
        payload.insert(
            "supportedModels".to_string(),
            Value::Array(vec![Value::String(default_model.clone())]),
        );
        payload.insert("defaultModel".to_string(), Value::String(default_model));
    }
    if let Some(cookie_header) = cookie_header {
        payload.insert(
            "headers".to_string(),
            serde_json::json!({
                "Cookie": cookie_header
            }),
        );
    }
    if let Some(credential_material_key) = credential_material_key {
        payload.insert(
            "credentialMaterialKey".to_string(),
            Value::String(credential_material_key),
        );
    }

    let bootstrap_keys = [
        ("accessToken", "accessToken"),
        ("access_token", "accessToken"),
        ("buildLabel", "buildLabel"),
        ("build_label", "buildLabel"),
        ("sessionId", "sessionId"),
        ("session_id", "sessionId"),
        ("language", "language"),
        ("pushId", "pushId"),
        ("push_id", "pushId"),
        ("modelHeader", "modelHeader"),
        ("model_header", "modelHeader"),
        ("requestContextHeader", "requestContextHeader"),
        ("request_context_header", "requestContextHeader"),
    ];
    let mut extra_body = serde_json::Map::new();
    for (raw_key, normalized_key) in bootstrap_keys {
        if let Some(value) = raw_map.get(raw_key).cloned() {
            extra_body.insert(normalized_key.to_string(), value);
        }
    }
    if let Some(model_headers) = raw_map.get("modelHeaders").cloned() {
        extra_body.insert("modelHeaders".to_string(), model_headers);
    }
    if !extra_body.is_empty() {
        payload.insert("extraBody".to_string(), Value::Object(extra_body));
    }

    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint.or(Some("session_auth")),
    );
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}

fn normalize_accio_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
) -> Result<Value, GatewayError> {
    implementation_lines::assert_accio_web_reverse_api_compiled(
        "compiled-out Accio import normalization requested",
    )?;
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "accio provider credential payload 必须是 JSON object",
        ));
    };

    let nested_headers = raw_map.get("headers").and_then(Value::as_object);
    let nested_extra_body = raw_map
        .get("extraBody")
        .or_else(|| raw_map.get("extra_body"))
        .and_then(Value::as_object);

    let access_token = read_optional_object_string(
        raw_map,
        &["accessToken", "access_token", "apiKey", "api_key"],
    )
    .or_else(|| {
        nested_extra_body.and_then(|extra| {
            read_optional_object_string(extra, &["accessToken", "access_token", "token"])
        })
    });
    if access_token.is_none() {
        return Ok(raw_payload);
    }
    let access_token = access_token.unwrap_or_default();
    let utdid = read_optional_object_string(raw_map, &["utdid", "x-utdid"])
        .or_else(|| {
            nested_headers
                .and_then(|headers| read_optional_object_string(headers, &["utdid", "x-utdid"]))
        })
        .ok_or_else(|| GatewayError::bad_request("accio provider credential payload 缺少 utdid"))?;

    let base_url = provider_account
        .payload
        .get("baseUrl")
        .and_then(Value::as_str)
        .or_else(|| {
            provider_account
                .payload
                .get("base_url")
                .and_then(Value::as_str)
        })
        .or_else(|| raw_map.get("baseUrl").and_then(Value::as_str))
        .or_else(|| raw_map.get("base_url").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("https://phoenix-gw.alibaba.com");
    let version = provider_account
        .payload
        .get("headers")
        .and_then(Value::as_object)
        .and_then(|headers| {
            headers
                .get("x-app-version")
                .or_else(|| headers.get("version"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            nested_headers.and_then(|headers| {
                read_optional_object_string(headers, &["version", "x-app-version"])
            })
        })
        .unwrap_or_else(|| "0.5.9".to_string());
    let default_model = provider_account
        .payload
        .get("defaultModel")
        .cloned()
        .or_else(|| provider_account.payload.get("default_model").cloned())
        .unwrap_or_else(|| Value::String("claude-sonnet-4-6".to_string()));
    let responses_path = provider_account
        .payload
        .get("responsesPath")
        .cloned()
        .or_else(|| provider_account.payload.get("responses_path").cloned())
        .unwrap_or_else(|| Value::String("/api/adk/llm/generateContent".to_string()));

    let refresh_token = raw_map
        .get("refreshToken")
        .or_else(|| raw_map.get("refresh_token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            nested_extra_body.and_then(|extra| {
                read_optional_object_string(extra, &["refreshToken", "refresh_token"])
            })
        });
    let cookie_header =
        read_optional_object_string(raw_map, &["cookie", "cookieHeader"]).or_else(|| {
            nested_headers
                .and_then(|headers| read_optional_object_string(headers, &["Cookie", "cookie"]))
        });
    let expires_at = raw_map
        .get("expiresAt")
        .or_else(|| raw_map.get("expires_at"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let account_name =
        read_optional_object_string(raw_map, &["accountName", "email", "name", "id"]);
    let credential_material_key = raw_map
        .get("credentialMaterialKey")
        .or_else(|| raw_map.get("materialKey"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|account_id| format!("accio-account:{account_id}"))
        });
    let excluded_models = raw_map
        .get("disabledModels")
        .or_else(|| raw_map.get("disabled_models"))
        .and_then(Value::as_object)
        .map(|models| {
            models
                .keys()
                .cloned()
                .map(Value::String)
                .collect::<Vec<_>>()
        })
        .filter(|models| !models.is_empty());
    let language = read_optional_object_string(raw_map, &["x-language", "language"])
        .or_else(|| {
            nested_headers.and_then(|headers| {
                read_optional_object_string(headers, &["x-language", "language"])
            })
        })
        .unwrap_or_else(|| "zh-CN".to_string());
    let os = read_optional_object_string(raw_map, &["x-os", "os"])
        .or_else(|| {
            nested_headers.and_then(|headers| read_optional_object_string(headers, &["x-os", "os"]))
        })
        .unwrap_or_else(|| "win32".to_string());
    let empid =
        read_optional_object_string(raw_map, &["empid", "empId", "accountId", "account_id"])
            .or_else(|| {
                nested_extra_body.and_then(|extra| {
                    read_optional_object_string(
                        extra,
                        &["empid", "empId", "accountId", "account_id"],
                    )
                })
            })
            .or_else(|| {
                cookie_header
                    .as_deref()
                    .and_then(extract_accio_empid_from_cookie_header)
            })
            .or_else(|| {
                read_optional_object_string(raw_map, &["id"]).and_then(|value| {
                    let trimmed = value.trim().to_string();
                    (!trimmed.is_empty()).then_some(trimmed)
                })
            })
            .unwrap_or_default();
    let tenant = read_optional_object_string(raw_map, &["tenant", "tenantId", "tenant_id"])
        .or_else(|| {
            nested_extra_body.and_then(|extra| {
                read_optional_object_string(extra, &["tenant", "tenantId", "tenant_id"])
            })
        })
        .unwrap_or_default();
    let iai_tag = read_optional_object_string(raw_map, &["iaiTag", "iai_tag"])
        .or_else(|| {
            nested_extra_body
                .and_then(|extra| read_optional_object_string(extra, &["iaiTag", "iai_tag"]))
        })
        .unwrap_or_else(|| "phoenix-desktop".to_string());
    let app_key = provider_account
        .payload
        .get("headers")
        .and_then(Value::as_object)
        .and_then(|headers| {
            headers
                .get("appKey")
                .or_else(|| headers.get("app_key"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            nested_headers
                .and_then(|headers| read_optional_object_string(headers, &["appKey", "app_key"]))
        })
        .or_else(|| Some("35306229".to_string()));

    let mut headers = serde_json::Map::new();
    headers.insert("utdid".to_string(), Value::String(utdid.to_string()));
    headers.insert("x-utdid".to_string(), Value::String(utdid.to_string()));
    headers.insert("version".to_string(), Value::String(version.to_string()));
    headers.insert(
        "x-app-version".to_string(),
        Value::String(version.to_string()),
    );
    headers.insert("x-language".to_string(), Value::String(language));
    headers.insert("x-os".to_string(), Value::String(os));
    if let Some(cookie_header) = cookie_header.clone() {
        headers.insert("Cookie".to_string(), Value::String(cookie_header.clone()));
        if let Some(cna) = extract_cookie_value(&cookie_header, "cna") {
            headers.insert("x-cna".to_string(), Value::String(cna.to_string()));
        }
    }
    if let Some(app_key) = app_key {
        headers.insert("appKey".to_string(), Value::String(app_key));
    }

    let mut extra_body = serde_json::Map::new();
    extra_body.insert("token".to_string(), Value::String(access_token.to_string()));
    extra_body.insert(
        "accessToken".to_string(),
        Value::String(access_token.to_string()),
    );
    extra_body.insert("empid".to_string(), Value::String(empid));
    extra_body.insert("tenant".to_string(), Value::String(tenant));
    extra_body.insert("iaiTag".to_string(), Value::String(iai_tag));
    extra_body.insert("utdid".to_string(), Value::String(utdid.to_string()));
    extra_body.insert("version".to_string(), Value::String(version.to_string()));

    let mut payload = serde_json::Map::new();
    payload.insert(
        "apiKey".to_string(),
        Value::String(access_token.to_string()),
    );
    payload.insert("baseUrl".to_string(), Value::String(base_url.to_string()));
    payload.insert("defaultModel".to_string(), default_model);
    payload.insert("responsesPath".to_string(), responses_path);
    payload.insert("headers".to_string(), Value::Object(headers));
    payload.insert("extraBody".to_string(), Value::Object(extra_body));
    if let Some(refresh_token) = refresh_token {
        payload.insert("refreshToken".to_string(), Value::String(refresh_token));
    }
    if let Some(expires_at) = expires_at {
        payload.insert("expiresAt".to_string(), Value::String(expires_at));
    }
    if let Some(account_name) = account_name {
        payload.insert("accountName".to_string(), Value::String(account_name));
    }
    if let Some(credential_material_key) = credential_material_key {
        payload.insert(
            "credentialMaterialKey".to_string(),
            Value::String(credential_material_key),
        );
    }
    if let Some(excluded_models) = excluded_models {
        payload.insert("excludedModels".to_string(), Value::Array(excluded_models));
    }
    apply_folder_sync_metadata(&mut payload, provider_account, Some("session_auth"));
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}

fn normalize_gemini_business_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "gemini business provider credential payload 必须是 JSON object",
        ));
    };

    let api_key = raw_map
        .get("apiKey")
        .or_else(|| raw_map.get("api_key"))
        .or_else(|| raw_map.get("token"))
        .or_else(|| raw_map.get("jwt"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let config_id =
        read_optional_object_string(raw_map, &["configId", "config_id"]).or_else(|| {
            raw_map
                .get("extraBody")
                .or_else(|| raw_map.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| read_optional_object_string(extra, &["configId", "config_id"]))
        });
    let session =
        read_optional_object_string(raw_map, &["session", "widgetSession", "widget_session"])
            .or_else(|| {
                raw_map
                    .get("extraBody")
                    .or_else(|| raw_map.get("extra_body"))
                    .and_then(Value::as_object)
                    .and_then(|extra| {
                        read_optional_object_string(
                            extra,
                            &[
                                "session",
                                "widgetSession",
                                "widget_session",
                                "upstreamSessionId",
                                "upstream_session_id",
                            ],
                        )
                    })
            });

    let mut payload = raw_map.clone();
    if let Some(api_key) = api_key {
        payload.insert("apiKey".to_string(), Value::String(api_key));
    }
    if config_id.is_some() || session.is_some() {
        let mut extra_body = payload
            .remove("extraBody")
            .or_else(|| payload.remove("extra_body"))
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        if let Some(config_id) = config_id {
            extra_body.insert("configId".to_string(), Value::String(config_id));
        }
        if let Some(session) = session {
            extra_body.insert("session".to_string(), Value::String(session));
        }
        payload.insert("extraBody".to_string(), Value::Object(extra_body));
    }
    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint.or(Some("jwt_widget_session")),
    );
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}

fn normalize_gemini_canvas_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "gemini canvas provider credential payload 必须是 JSON object",
        ));
    };

    let runtime_state_object_key = read_optional_object_string(
        raw_map,
        &["runtimeStateObjectKey", "runtime_state_object_key"],
    )
    .ok_or_else(|| {
        GatewayError::bad_request(
            "gemini canvas provider credential payload 缺少 runtimeStateObjectKey。",
        )
    })?;
    let share_url =
        read_optional_object_string(raw_map, &["shareUrl", "share_url"]).or_else(|| {
            provider_account
                .payload
                .get("extraBody")
                .or_else(|| provider_account.payload.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| read_optional_object_string(extra, &["shareUrl", "share_url"]))
        });
    let share_id = read_optional_object_string(raw_map, &["shareId", "share_id"])
        .or_else(|| {
            read_optional_object_string(raw_map, &["suggestedShareId", "suggested_share_id"])
        })
        .or_else(|| {
            share_url
                .as_deref()
                .and_then(crate::protocol::gemini_canvas::share_id_from_share_url)
        })
        .or_else(|| {
            provider_account
                .payload
                .get("extraBody")
                .or_else(|| provider_account.payload.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| read_optional_object_string(extra, &["shareId", "share_id"]))
        })
        .or_else(|| {
            provider_account
                .payload
                .get("extraBody")
                .or_else(|| provider_account.payload.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| read_optional_object_string(extra, &["shareUrl", "share_url"]))
                .and_then(|value| crate::protocol::gemini_canvas::share_id_from_share_url(&value))
        });
    let api_base_url = read_optional_object_string(raw_map, &["apiBaseUrl", "api_base_url"])
        .or_else(|| {
            provider_account
                .payload
                .get("extraBody")
                .or_else(|| provider_account.payload.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| {
                    read_optional_object_string(extra, &["apiBaseUrl", "api_base_url"])
                })
        });
    let canvas_program_url = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramUrl",
            "canvas_program_url",
            "programUrl",
            "program_url",
        ],
    )
    .or_else(|| {
        provider_account
            .payload
            .get("extraBody")
            .or_else(|| provider_account.payload.get("extra_body"))
            .and_then(Value::as_object)
            .and_then(|extra| {
                read_optional_object_string(
                    extra,
                    &[
                        "canvasProgramUrl",
                        "canvas_program_url",
                        "programUrl",
                        "program_url",
                    ],
                )
            })
    });
    let canvas_program_hint = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramHint",
            "canvas_program_hint",
            "programHint",
            "program_hint",
        ],
    )
    .or_else(|| {
        provider_account
            .payload
            .get("extraBody")
            .or_else(|| provider_account.payload.get("extra_body"))
            .and_then(Value::as_object)
            .and_then(|extra| {
                read_optional_object_string(
                    extra,
                    &[
                        "canvasProgramHint",
                        "canvas_program_hint",
                        "programHint",
                        "program_hint",
                    ],
                )
            })
    });
    let page_url = read_optional_object_string(raw_map, &["pageUrl", "page_url"]);
    let app_path = read_optional_object_string(raw_map, &["appPath", "app_path"]);
    let conversation_id =
        read_optional_object_string(raw_map, &["conversationId", "conversation_id"]);
    let response_id = read_optional_object_string(raw_map, &["responseId", "response_id"]);
    let invoke_base_url = read_optional_object_string(
        raw_map,
        &[
            "invokeBaseUrl",
            "invoke_base_url",
            "appEndpointBaseUrl",
            "app_endpoint_base_url",
        ],
    );
    let music_ws_url = read_optional_object_string(
        raw_map,
        &[
            "musicWsUrl",
            "music_ws_url",
            "appMusicWsUrl",
            "app_music_ws_url",
        ],
    );
    let video_invoke_path = read_optional_object_string(
        raw_map,
        &[
            "videoInvokePath",
            "video_invoke_path",
            "videoPath",
            "video_path",
        ],
    );
    let canvas_program_action = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramAction",
            "canvas_program_action",
            "programAction",
            "program_action",
        ],
    );
    let canvas_program_action_input = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramActionInput",
            "canvas_program_action_input",
            "programActionInput",
            "program_action_input",
        ],
    );
    let canvas_program_invoke_contract = raw_map
        .get("canvasProgramInvokeContract")
        .or_else(|| raw_map.get("canvas_program_invoke_contract"))
        .or_else(|| raw_map.get("programInvokeContract"))
        .or_else(|| raw_map.get("program_invoke_contract"))
        .filter(|value| value.is_object())
        .cloned();
    let before_url = read_optional_object_string(raw_map, &["beforeUrl", "before_url"]);
    let final_url = read_optional_object_string(raw_map, &["finalUrl", "final_url"]);
    let share_follow_kind =
        read_optional_object_string(raw_map, &["shareFollowKind", "share_follow_kind"]);
    let program_id = read_optional_object_string(raw_map, &["programId", "program_id"]);
    let canvas_program_operation = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramOperation",
            "canvas_program_operation",
            "bootstrapOperation",
            "bootstrap_operation",
        ],
    );
    let last_seen_conversation_id = read_optional_object_string(
        raw_map,
        &["lastSeenConversationId", "last_seen_conversation_id"],
    );
    let last_seen_response_id =
        read_optional_object_string(raw_map, &["lastSeenResponseId", "last_seen_response_id"]);
    let captured_at = read_optional_object_string(raw_map, &["capturedAt", "captured_at"]);
    let last_validated_at =
        read_optional_object_string(raw_map, &["lastValidatedAt", "last_validated_at"]);
    let candidate_pairs = raw_map
        .get("candidatePairs")
        .filter(|value| value.is_array())
        .cloned();
    let aggregate_hints = raw_map
        .get("aggregateHints")
        .filter(|value| value.is_object())
        .cloned();
    let new_chat_clicked = raw_map.get("newChatClicked").and_then(Value::as_bool);
    let mode_selected = raw_map.get("modeSelected").and_then(Value::as_bool);
    let account_name = read_optional_object_string(raw_map, &["accountName", "account_name"])
        .or_else(|| read_optional_object_string(raw_map, &["currentUrl", "current_url"]));
    let credential_material_key = read_optional_object_string(
        raw_map,
        &[
            "credentialMaterialKey",
            "credential_material_key",
            "materialKey",
        ],
    );
    let google_api_key = read_optional_object_string(
        raw_map,
        &["googleApiKey", "google_api_key", "apiKey", "api_key"],
    );
    let api_keys = read_optional_object_string_array(raw_map, &["apiKeys", "api_keys"]);
    let cookie_header = read_optional_object_string(raw_map, &["cookieHeader", "cookie_header"]);
    let program_owned_surface =
        provider_account.adapter == GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER;

    let mut payload = serde_json::Map::new();
    payload.insert("apiKey".to_string(), Value::String(String::new()));
    payload.insert(
        "runtimeStateObjectKey".to_string(),
        Value::String(runtime_state_object_key),
    );
    if let Some(account_name) = account_name {
        payload.insert("accountName".to_string(), Value::String(account_name));
    }
    let mut extra_body = serde_json::Map::new();
    if let Some(share_id) = share_id {
        extra_body.insert("shareId".to_string(), Value::String(share_id));
    }
    if let Some(api_base_url) = api_base_url {
        extra_body.insert("apiBaseUrl".to_string(), Value::String(api_base_url));
    }
    if let Some(canvas_program_url) = canvas_program_url {
        extra_body.insert(
            "canvasProgramUrl".to_string(),
            Value::String(canvas_program_url),
        );
    }
    if let Some(canvas_program_hint) = canvas_program_hint {
        extra_body.insert(
            "canvasProgramHint".to_string(),
            Value::String(canvas_program_hint),
        );
    }
    if let Some(page_url) = page_url {
        extra_body.insert("pageUrl".to_string(), Value::String(page_url));
    }
    if let Some(app_path) = app_path {
        extra_body.insert("appPath".to_string(), Value::String(app_path));
    }
    if let Some(conversation_id) = conversation_id {
        extra_body.insert("conversationId".to_string(), Value::String(conversation_id));
    }
    if let Some(response_id) = response_id {
        extra_body.insert("responseId".to_string(), Value::String(response_id));
    }
    if let Some(invoke_base_url) = invoke_base_url {
        extra_body.insert("invokeBaseUrl".to_string(), Value::String(invoke_base_url));
    }
    if let Some(music_ws_url) = music_ws_url {
        extra_body.insert("musicWsUrl".to_string(), Value::String(music_ws_url));
    }
    if let Some(video_invoke_path) = video_invoke_path {
        extra_body.insert(
            "videoInvokePath".to_string(),
            Value::String(video_invoke_path),
        );
    }
    if let Some(canvas_program_action) = canvas_program_action {
        extra_body.insert(
            "canvasProgramAction".to_string(),
            Value::String(canvas_program_action),
        );
    }
    if let Some(canvas_program_action_input) = canvas_program_action_input {
        extra_body.insert(
            "canvasProgramActionInput".to_string(),
            Value::String(canvas_program_action_input),
        );
    }
    if let Some(canvas_program_invoke_contract) = canvas_program_invoke_contract {
        extra_body.insert(
            "canvasProgramInvokeContract".to_string(),
            canvas_program_invoke_contract,
        );
    }
    if let Some(share_url) = share_url {
        extra_body.insert("shareUrl".to_string(), Value::String(share_url));
    }
    if let Some(before_url) = before_url {
        extra_body.insert("beforeUrl".to_string(), Value::String(before_url));
    }
    if let Some(final_url) = final_url {
        extra_body.insert("finalUrl".to_string(), Value::String(final_url));
    }
    if let Some(share_follow_kind) = share_follow_kind {
        extra_body.insert(
            "shareFollowKind".to_string(),
            Value::String(share_follow_kind),
        );
    }
    if let Some(program_id) = program_id {
        extra_body.insert("programId".to_string(), Value::String(program_id));
    }
    if let Some(canvas_program_operation) = canvas_program_operation {
        extra_body.insert(
            "canvasProgramOperation".to_string(),
            Value::String(canvas_program_operation.clone()),
        );
        extra_body.insert(
            "bootstrapOperation".to_string(),
            Value::String(canvas_program_operation),
        );
    }
    if let Some(last_seen_conversation_id) = last_seen_conversation_id {
        extra_body.insert(
            "lastSeenConversationId".to_string(),
            Value::String(last_seen_conversation_id),
        );
    }
    if let Some(last_seen_response_id) = last_seen_response_id {
        extra_body.insert(
            "lastSeenResponseId".to_string(),
            Value::String(last_seen_response_id),
        );
    }
    if let Some(captured_at) = captured_at {
        extra_body.insert("capturedAt".to_string(), Value::String(captured_at));
    }
    if let Some(last_validated_at) = last_validated_at {
        extra_body.insert(
            "lastValidatedAt".to_string(),
            Value::String(last_validated_at),
        );
    }
    if let Some(candidate_pairs) = candidate_pairs {
        extra_body.insert("candidatePairs".to_string(), candidate_pairs);
    }
    if let Some(aggregate_hints) = aggregate_hints {
        extra_body.insert("aggregateHints".to_string(), aggregate_hints);
    }
    if let Some(new_chat_clicked) = new_chat_clicked {
        extra_body.insert("newChatClicked".to_string(), Value::Bool(new_chat_clicked));
    }
    if let Some(mode_selected) = mode_selected {
        extra_body.insert("modeSelected".to_string(), Value::Bool(mode_selected));
    }
    if let Some(cookie_header) = cookie_header {
        extra_body.insert("cookieHeader".to_string(), Value::String(cookie_header));
    }
    if !program_owned_surface {
        if let Some(google_api_key) = google_api_key {
            extra_body.insert("googleApiKey".to_string(), Value::String(google_api_key));
        }
    }
    if !program_owned_surface {
        if let Some(api_keys) = api_keys {
            extra_body.insert(
                "apiKeys".to_string(),
                Value::Array(api_keys.into_iter().map(Value::String).collect()),
            );
        }
    }
    if !extra_body.is_empty() {
        payload.insert("extraBody".to_string(), Value::Object(extra_body));
    }
    if let Some(credential_material_key) = credential_material_key {
        payload.insert(
            "credentialMaterialKey".to_string(),
            Value::String(credential_material_key),
        );
    }
    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint.or(Some("browser_state")),
    );
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}

fn apply_folder_sync_metadata(
    payload: &mut serde_json::Map<String, Value>,
    provider_account: &db::GatewayProviderAccountView,
    credential_material_kind_hint: Option<&str>,
) {
    payload
        .entry("serviceProviderKey".to_string())
        .or_insert_with(|| Value::String(provider_account.service_provider_key.clone()));
    payload
        .entry("providerSurfaceKey".to_string())
        .or_insert_with(|| Value::String(derive_provider_surface_slug(provider_account)));

    let material_kind = credential_material_kind_hint
        .map(canonicalize_credential_material_kind)
        .unwrap_or_else(|| {
            derive_credential_material_kind(provider_account, Some(&Value::Object(payload.clone())))
        });
    payload
        .entry("credentialMaterialKind".to_string())
        .or_insert_with(|| Value::String(material_kind));
}

fn canonicalize_folder_sync_raw_source(raw_payload: &Value) -> Value {
    let mut cursor = raw_payload;
    let mut depth = 0usize;
    while depth < 32 {
        let Some(raw_map) = cursor.as_object() else {
            break;
        };
        let Some(next) = raw_map.get("rawSource") else {
            break;
        };
        cursor = next;
        depth += 1;
    }

    match cursor {
        Value::Object(raw_map) => {
            let mut sanitized = raw_map.clone();
            sanitized.remove("rawSource");
            Value::Object(sanitized)
        }
        other => other.clone(),
    }
}

fn read_optional_object_string(
    raw_map: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        raw_map
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn read_optional_object_string_array(
    raw_map: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<Vec<String>> {
    keys.iter().find_map(|key| {
        let values = raw_map
            .get(*key)
            .and_then(Value::as_array)?
            .iter()
            .filter_map(|entry| {
                entry
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        (!values.is_empty()).then_some(values)
    })
}

fn extract_cookie_value<'a>(cookie_header: &'a str, cookie_name: &str) -> Option<&'a str> {
    cookie_header
        .split(';')
        .filter_map(|segment| segment.split_once('='))
        .find_map(|(name, value)| {
            if name.trim().eq_ignore_ascii_case(cookie_name) {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            } else {
                None
            }
        })
}

fn extract_accio_empid_from_cookie_header(cookie_header: &str) -> Option<String> {
    let xman_us_f = extract_cookie_value(cookie_header, "xman_us_f")?;
    for segment in xman_us_f.split('&') {
        let (name, value) = segment.split_once('=')?;
        if !name.trim().eq_ignore_ascii_case("x_user") {
            continue;
        }
        let empid = value
            .split('|')
            .next_back()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())?;
        return Some(empid.to_string());
    }
    None
}

fn credential_label_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "Imported Credential".to_string())
}

fn normalize_source_path_key(source_path: &str) -> Option<String> {
    let segments = source_path
        .split(['/', '\\'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return None;
    }
    Some(segments.join("/"))
}

fn deleted_relative_paths_from_event(root_dir: &Path, event: &Event) -> HashSet<String> {
    event
        .paths
        .iter()
        .filter(|path| folder_sync_path_should_trigger(path.as_path()))
        .filter(|path| !path.exists())
        .filter_map(|path| normalize_deleted_relative_path(root_dir, path.as_path()))
        .collect()
}

fn normalize_deleted_relative_path(root_dir: &Path, file_path: &Path) -> Option<String> {
    file_path
        .strip_prefix(root_dir)
        .ok()
        .and_then(|relative| relative.to_str())
        .and_then(normalize_source_path_key)
}

const RECENT_EXPLICIT_DELETE_EVENT_LIMIT: usize = 8;

fn build_explicit_delete_event(
    deleted_hits: &[FolderSyncExplicitDeleteHit],
) -> FolderSyncExplicitDeleteEventView {
    let mut deleted_paths = Vec::new();
    let mut provider_credential_ids = Vec::new();
    for hit in deleted_hits {
        if !deleted_paths.contains(&hit.source_path) {
            deleted_paths.push(hit.source_path.clone());
        }
        if !provider_credential_ids.contains(&hit.provider_credential_id) {
            provider_credential_ids.push(hit.provider_credential_id.clone());
        }
    }
    FolderSyncExplicitDeleteEventView {
        event_id: Uuid::new_v4().to_string(),
        occurred_at: format_timestamp(OffsetDateTime::now_utc()),
        deleted_count: deleted_paths.len(),
        deleted_paths,
        provider_credential_ids,
    }
}

fn append_explicit_delete_events(
    status: &mut ProviderCredentialFolderSyncStatusView,
    events: &[FolderSyncExplicitDeleteEventView],
) {
    let Some(latest_event) = events.last() else {
        return;
    };
    let mut new_events = events.to_vec();
    new_events.reverse();
    new_events.extend(status.recent_explicit_delete_events.iter().cloned());
    new_events.truncate(RECENT_EXPLICIT_DELETE_EVENT_LIMIT);
    status.recent_explicit_delete_events = new_events;
    status.last_explicit_delete_at = Some(latest_event.occurred_at.clone());
    status.last_explicit_delete_count = latest_event.deleted_count;
    status.last_explicit_delete_paths = latest_event.deleted_paths.clone();
}

fn apply_explicit_delete_summary(
    status: &mut ProviderCredentialFolderSyncStatusView,
    counters: &FolderSyncCounters,
) {
    append_explicit_delete_events(status, &counters.explicit_delete_events);
}

fn should_delete_missing_folder_credential(
    credential: &db::GatewayProviderCredentialView,
    observed_paths: &HashSet<String>,
) -> bool {
    credential.archived_at.is_none()
        && credential.status != "archived"
        && credential.sync_mode == "folder_sync"
        && has_materialized_folder_copy(credential)
        && credential
            .source_path
            .as_deref()
            .and_then(normalize_source_path_key)
            .is_some_and(|source_path| !observed_paths.contains(&source_path))
}

fn should_delete_explicitly_removed_folder_credential(
    credential: &db::GatewayProviderCredentialView,
    observed_paths: &HashSet<String>,
    explicit_deleted_paths: &HashSet<String>,
) -> bool {
    credential.archived_at.is_none()
        && credential.status != "archived"
        && credential.sync_mode == "folder_sync"
        && has_materialized_folder_copy(credential)
        && credential
            .source_path
            .as_deref()
            .and_then(normalize_source_path_key)
            .is_some_and(|source_path| {
                source_path_matches_explicit_delete(&source_path, explicit_deleted_paths)
                    && !observed_paths.contains(&source_path)
            })
}

fn source_path_matches_explicit_delete(
    source_path: &str,
    explicit_deleted_paths: &HashSet<String>,
) -> bool {
    explicit_deleted_paths.iter().any(|deleted_path| {
        source_path
            .strip_prefix(deleted_path)
            .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('/'))
    })
}

fn has_materialized_folder_copy(credential: &db::GatewayProviderCredentialView) -> bool {
    credential.source_kind == "folder_sync_import"
        || matches!(credential.sync_state.as_str(), "imported" | "exported")
}

fn collect_json_files(root_dir: &Path) -> Result<Vec<PathBuf>, GatewayError> {
    let mut files = Vec::new();
    collect_json_files_recursive(root_dir, &mut files)?;
    Ok(files)
}

fn collect_json_files_recursive(
    current_dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), GatewayError> {
    if !current_dir.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(current_dir).map_err(|error| {
        GatewayError::server_error(format!(
            "read provider credential directory {}: {error}",
            current_dir.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            GatewayError::server_error(format!(
                "read provider credential directory entry {}: {error}",
                current_dir.display()
            ))
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files_recursive(&path, files)?;
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn normalize_relative_path(root_dir: &Path, file_path: &Path) -> Result<String, GatewayError> {
    let relative = file_path.strip_prefix(root_dir).map_err(|error| {
        GatewayError::server_error(format!(
            "derive relative provider credential path {}: {error}",
            file_path.display()
        ))
    })?;
    let segments = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(GatewayError::bad_request(
            "provider credential path is empty",
        ));
    }
    Ok(segments.join("/"))
}

fn absolute_path_for_relative(root_dir: &Path, relative: &str) -> PathBuf {
    let mut path = root_dir.to_path_buf();
    for segment in relative.split('/') {
        if !segment.trim().is_empty() {
            path.push(segment);
        }
    }
    path
}

fn sanitize_file_component(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderExecutionMode;

    fn build_test_provider_account(
        label: &str,
        adapter: &str,
        protocol_family: &str,
        protocol_profile: &str,
        base_url: &str,
        source_kind: Option<&str>,
        web_reverse_access_mode: Option<&str>,
    ) -> db::GatewayProviderAccountView {
        db::GatewayProviderAccountView {
            id: "provider-1".to_string(),
            label: label.to_string(),
            service_provider_key: "qwen_platform".to_string(),
            service_provider_label: "Qwen Platform".to_string(),
            adapter: adapter.to_string(),
            protocol_family: protocol_family.to_string(),
            protocol_profile: protocol_profile.to_string(),
            status: "active".to_string(),
            source_kind: source_kind.map(|value| value.to_string()),
            aggregator_api_mode: None,
            web_reverse_access_mode: web_reverse_access_mode.map(|value| value.to_string()),
            source_notes: None,
            execution_mode: ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: serde_json::json!({
                "baseUrl": base_url,
                "defaultModel": "qwen3-coder-plus"
            }),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-20T00:00:00Z".to_string(),
            updated_at: "2026-04-20T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn folder_sync_event_ignores_access_only_changes() {
        let event = Event {
            kind: EventKind::Access(notify::event::AccessKind::Any),
            paths: vec![PathBuf::from("/tmp/example.json")],
            attrs: Default::default(),
        };
        assert!(!folder_sync_event_should_trigger(&event));
    }

    #[test]
    fn folder_sync_event_triggers_for_json_changes() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![PathBuf::from("/tmp/example.json")],
            attrs: Default::default(),
        };
        assert!(folder_sync_event_should_trigger(&event));
    }

    #[test]
    fn folder_sync_path_triggers_for_provider_directory_changes() {
        assert!(folder_sync_path_should_trigger(Path::new("/tmp/codex")));
        assert!(!folder_sync_path_should_trigger(Path::new(
            "/tmp/readme.txt"
        )));
    }

    fn build_test_credential(
        id: &str,
        sync_mode: &str,
        source_path: Option<&str>,
        status: &str,
        archived_at: Option<&str>,
    ) -> db::GatewayProviderCredentialView {
        db::GatewayProviderCredentialView {
            id: id.to_string(),
            provider_account_id: "provider-1".to_string(),
            label: format!("Credential {id}"),
            status: status.to_string(),
            payload: serde_json::json!({}),
            storage_mode: "inline".to_string(),
            source_kind: "folder_sync_import".to_string(),
            source_path: source_path.map(|value| value.to_string()),
            source_hash: None,
            sync_mode: sync_mode.to_string(),
            sync_state: "idle".to_string(),
            sync_error: None,
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-18T00:00:00Z".to_string(),
            updated_at: "2026-04-18T00:00:00Z".to_string(),
            archived_at: archived_at.map(|value| value.to_string()),
        }
    }

    #[test]
    fn normalize_source_path_key_supports_windows_separators() {
        assert_eq!(
            normalize_source_path_key(r"codex\team-alpha.json").as_deref(),
            Some("codex/team-alpha.json")
        );
    }

    #[test]
    fn derive_provider_family_slug_prefers_qwen_web_chat_for_qwen_web_surfaces() {
        let provider = build_test_provider_account(
            "Qwen WebUI Replay Live",
            "qwen_web_compatible",
            "qwen_web_chat",
            "qwen_web_chat",
            "https://chat.qwen.ai",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        assert_eq!(derive_provider_family_slug(&provider), "qwen-web-chat");
        assert_eq!(
            canonicalize_folder_family_slug("qwen_web_chat"),
            "qwen-web-chat"
        );
        assert_eq!(canonicalize_folder_family_slug("qwen-web"), "qwen-web-chat");
        assert_eq!(
            canonicalize_folder_family_slug("qwen-webui"),
            "qwen-web-chat"
        );
        assert_eq!(
            canonicalize_folder_family_slug("qwen-webui-replay-live"),
            "qwen-web-chat"
        );
    }

    #[test]
    fn derive_provider_family_slug_prefers_accio_for_phoenix_surfaces() {
        let provider = build_test_provider_account(
            "Accio Live",
            "accio_compatible",
            "accio",
            "openai_responses",
            "https://phoenix-gw.alibaba.com",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        assert_eq!(derive_provider_family_slug(&provider), "accio");
        assert_eq!(canonicalize_folder_family_slug("accio-manager"), "accio");
    }

    #[test]
    fn derive_provider_family_slug_prefers_nvidia_for_official_nim_surfaces() {
        let provider = build_test_provider_account(
            "NVIDIA Platform Live",
            "openai_compatible",
            "nvidia",
            "openai",
            "https://integrate.api.nvidia.com",
            Some("official_model_api"),
            None,
        );
        assert_eq!(derive_provider_family_slug(&provider), "nvidia");
        assert_eq!(canonicalize_folder_family_slug("nvidia-platform"), "nvidia");
        assert_eq!(canonicalize_folder_family_slug("nvidia_nim"), "nvidia");
    }

    #[test]
    fn derive_provider_surface_slug_maps_chatgpt_platform_dual_lines() {
        let mut official = build_test_provider_account(
            "OpenAI Platform",
            "openai_compatible",
            "openai",
            "chatgpt_official_api",
            "https://api.openai.com/v1",
            Some("official_vendor_api"),
            None,
        );
        official.service_provider_key = "chatgpt_platform".to_string();
        official.service_provider_label = "ChatGPT Platform".to_string();
        assert_eq!(
            derive_provider_surface_slug(&official),
            "chatgpt-official-api"
        );
        assert_eq!(
            canonicalize_folder_surface_slug("openai-platform"),
            "chatgpt-official-api"
        );

        let mut codex = build_test_provider_account(
            "ChatGPT Codex Backend",
            "openai_compatible",
            "openai",
            "chatgpt_codex_backend",
            "https://chatgpt.com/backend-api/codex",
            Some("official_vendor_api"),
            None,
        );
        codex.service_provider_key = "chatgpt_platform".to_string();
        codex.service_provider_label = "ChatGPT Platform".to_string();
        assert_eq!(
            derive_provider_surface_slug(&codex),
            "chatgpt-codex-backend"
        );
        assert_eq!(
            canonicalize_folder_surface_slug("chatgpt-codex-backend"),
            "chatgpt-codex-backend"
        );
    }

    #[test]
    fn derive_provider_family_slug_keeps_gemini_modular_lines_explicit() {
        let mut official = build_test_provider_account(
            "Gemini API Modular",
            "gemini_api_modular_compatible",
            "gemini_generate_content",
            GEMINI_API_MODULAR_PROFILE,
            "https://example.invalid",
            Some("official_model_api"),
            None,
        );
        official.service_provider_key = "gemini_platform".to_string();
        official.service_provider_label = "Gemini Platform".to_string();
        assert_eq!(derive_provider_family_slug(&official), "gemini");

        let mut web = build_test_provider_account(
            "Gemini Web Reverse Modular",
            "gemini_web_reverse_modular_compatible",
            "gemini_web_chat",
            GEMINI_WEB_REVERSE_MODULAR_PROFILE,
            "https://example.invalid",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        web.service_provider_key = "gemini_platform".to_string();
        web.service_provider_label = "Gemini Platform".to_string();
        assert_eq!(derive_provider_family_slug(&web), "gemini-web-chat");

        let mut program = build_test_provider_account(
            "Gemini Canvas Program Relay",
            "gemini_canvas_program_web_reverse_compatible",
            "gemini_canvas_images",
            GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
            "https://example.invalid",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        program.service_provider_key = "gemini_platform".to_string();
        program.service_provider_label = "Gemini Platform".to_string();
        assert_eq!(derive_provider_family_slug(&program), "gemini");
    }

    #[test]
    fn derive_provider_surface_slug_maps_gemini_modular_lines() {
        let mut official = build_test_provider_account(
            "Gemini API Modular",
            "gemini_api_modular_compatible",
            "gemini_generate_content",
            GEMINI_API_MODULAR_PROFILE,
            "https://example.invalid",
            Some("official_model_api"),
            None,
        );
        official.service_provider_key = "gemini_platform".to_string();
        official.service_provider_label = "Gemini Platform".to_string();
        assert_eq!(
            derive_provider_surface_slug(&official),
            "google-gemini-api-modular"
        );

        let mut browser = build_test_provider_account(
            "Gemini Canvas Browser Relay",
            "gemini_canvas_web_reverse_compatible",
            "gemini_canvas_images",
            GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE,
            "https://example.invalid",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        browser.service_provider_key = "gemini_platform".to_string();
        browser.service_provider_label = "Gemini Platform".to_string();
        assert_eq!(
            derive_provider_surface_slug(&browser),
            "gemini-canvas-browser-relay"
        );

        let mut program = build_test_provider_account(
            "Gemini Canvas Program Relay",
            "gemini_canvas_program_web_reverse_compatible",
            "gemini_canvas_images",
            GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
            "https://example.invalid",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        program.service_provider_key = "gemini_platform".to_string();
        program.service_provider_label = "Gemini Platform".to_string();
        assert_eq!(
            derive_provider_surface_slug(&program),
            "gemini-canvas-program-relay"
        );
    }

    #[test]
    fn derive_provider_surface_slug_maps_chataibot_image_line() {
        let mut provider = build_test_provider_account(
            "ChatAIBot Images",
            "chataibot_compatible",
            "chataibot_images",
            "chataibot",
            "https://chataibot.pro",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        provider.service_provider_key = "chataibot_platform".to_string();
        provider.service_provider_label = "ChatAIBot".to_string();

        assert_eq!(derive_provider_surface_slug(&provider), "chataibot-images");
        assert_eq!(
            canonicalize_folder_surface_slug("chataibot-images"),
            "chataibot-images"
        );
    }

    #[test]
    fn derive_provider_surface_slug_maps_wave3_official_api_lines() {
        let mut azure = build_test_provider_account(
            "Azure OpenAI",
            "openai_compatible",
            "openai",
            "azure_openai",
            "https://example.openai.azure.com/openai/v1",
            Some("official_vendor_api"),
            None,
        );
        azure.service_provider_key = "azure_openai_platform".to_string();
        azure.service_provider_label = "Azure OpenAI".to_string();
        assert_eq!(derive_provider_surface_slug(&azure), "azure-openai");
        assert_eq!(
            canonicalize_folder_surface_slug("azure-openai-v1"),
            "azure-openai"
        );

        let mut anthropic = build_test_provider_account(
            "Anthropic Messages",
            "anthropic_compatible",
            "anthropic",
            "anthropic",
            "https://api.anthropic.com/v1",
            Some("official_model_api"),
            None,
        );
        anthropic.service_provider_key = "anthropic_platform".to_string();
        anthropic.service_provider_label = "Anthropic Messages".to_string();
        assert_eq!(
            derive_provider_surface_slug(&anthropic),
            "anthropic-compatible"
        );
        assert_eq!(
            canonicalize_folder_surface_slug("anthropic-messages"),
            "anthropic-compatible"
        );

        let mut bedrock = build_test_provider_account(
            "AWS Bedrock Converse",
            "bedrock_converse_compatible",
            "bedrock_converse",
            "aws_bedrock",
            "https://bedrock-runtime.us-east-1.amazonaws.com",
            Some("official_model_api"),
            None,
        );
        bedrock.service_provider_key = "aws_bedrock_platform".to_string();
        bedrock.service_provider_label = "AWS Bedrock Converse".to_string();
        assert_eq!(derive_provider_surface_slug(&bedrock), "bedrock-converse");

        let mut cohere = build_test_provider_account(
            "Cohere Chat",
            "cohere_compatible",
            "cohere_chat",
            "cohere",
            "https://api.cohere.com",
            Some("official_model_api"),
            None,
        );
        cohere.service_provider_key = "cohere_platform".to_string();
        cohere.service_provider_label = "Cohere Chat".to_string();
        assert_eq!(derive_provider_surface_slug(&cohere), "cohere-chat");
        assert_eq!(
            canonicalize_folder_surface_slug("cohere-chat-v2"),
            "cohere-chat"
        );
    }

    #[test]
    fn derive_provider_surface_slug_maps_wave4_openai_compatible_lines() {
        let mut groq = build_test_provider_account(
            "Groq OpenAI-compatible",
            "openai_compatible",
            "openai",
            "groq",
            "https://api.groq.com/openai/v1",
            Some("official_vendor_api"),
            None,
        );
        groq.service_provider_key = "groq_platform".to_string();
        groq.service_provider_label = "Groq OpenAI-compatible".to_string();
        assert_eq!(derive_provider_surface_slug(&groq), "groq-openai");
        assert_eq!(canonicalize_folder_surface_slug("groq"), "groq-openai");

        let mut together = build_test_provider_account(
            "Together OpenAI-compatible",
            "openai_compatible",
            "openai",
            "together",
            "https://api.together.xyz/v1",
            Some("aggregator_api"),
            None,
        );
        together.service_provider_key = "together_platform".to_string();
        together.service_provider_label = "Together OpenAI-compatible".to_string();
        assert_eq!(derive_provider_surface_slug(&together), "together-openai");
        assert_eq!(
            canonicalize_folder_surface_slug("together"),
            "together-openai"
        );

        let mut openrouter = build_test_provider_account(
            "OpenRouter OpenAI-compatible",
            "openai_compatible",
            "openai",
            "openrouter",
            "https://openrouter.ai/api/v1",
            Some("aggregator_api"),
            None,
        );
        openrouter.service_provider_key = "openrouter_platform".to_string();
        openrouter.service_provider_label = "OpenRouter OpenAI-compatible".to_string();
        assert_eq!(
            derive_provider_surface_slug(&openrouter),
            "openrouter-openai"
        );
        assert_eq!(
            canonicalize_folder_surface_slug("openrouter"),
            "openrouter-openai"
        );

        let mut deepseek = build_test_provider_account(
            "DeepSeek OpenAI-compatible",
            "openai_compatible",
            "openai",
            "deepseek",
            "https://api.deepseek.com/v1",
            Some("official_model_api"),
            None,
        );
        deepseek.service_provider_key = "deepseek_platform".to_string();
        deepseek.service_provider_label = "DeepSeek OpenAI-compatible".to_string();
        assert_eq!(derive_provider_surface_slug(&deepseek), "deepseek-openai");
        assert_eq!(
            canonicalize_folder_surface_slug("deepseek"),
            "deepseek-openai"
        );

        let mut mistral = build_test_provider_account(
            "Mistral OpenAI-compatible",
            "openai_compatible",
            "openai",
            "mistral",
            "https://api.mistral.ai/v1",
            Some("official_model_api"),
            None,
        );
        mistral.service_provider_key = "mistral_platform".to_string();
        mistral.service_provider_label = "Mistral OpenAI-compatible".to_string();
        assert_eq!(derive_provider_surface_slug(&mistral), "mistral-openai");
        assert_eq!(
            canonicalize_folder_surface_slug("mistral"),
            "mistral-openai"
        );
    }

    #[test]
    fn derive_provider_surface_slug_maps_nvidia_and_grok_lines() {
        let mut nvidia = build_test_provider_account(
            "NVIDIA OpenAI-compatible",
            "openai_compatible",
            "openai",
            "nvidia",
            "https://integrate.api.nvidia.com/v1",
            Some("official_vendor_api"),
            None,
        );
        nvidia.service_provider_key = "nvidia_platform".to_string();
        nvidia.service_provider_label = "NVIDIA OpenAI-compatible".to_string();
        assert_eq!(derive_provider_family_slug(&nvidia), "nvidia");
        assert_eq!(derive_provider_surface_slug(&nvidia), "nvidia-openai");
        assert_eq!(
            canonicalize_folder_surface_slug("nvidia-nim"),
            "nvidia-openai"
        );

        let mut grok = build_test_provider_account(
            "Grok Web",
            "grok_compatible",
            "openai",
            "grok_web",
            "https://grok.com",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        grok.service_provider_key = "grok_platform".to_string();
        grok.service_provider_label = "Grok Web".to_string();
        assert_eq!(derive_provider_family_slug(&grok), "grok");
        assert_eq!(derive_provider_surface_slug(&grok), "grok-web-reverse-api");
        assert_eq!(
            canonicalize_folder_surface_slug("grok-web"),
            "grok-web-reverse-api"
        );
    }

    #[test]
    fn derive_provider_surface_slug_maps_media_platform_lines() {
        let mut suno = build_test_provider_account(
            "Suno Music",
            "suno_compatible",
            "suno_music",
            "suno",
            "https://suno.com",
            Some("web_reverse_api"),
            Some("browser_challenge"),
        );
        suno.service_provider_key = "suno_platform".to_string();
        suno.service_provider_label = "Suno Platform".to_string();
        assert_eq!(derive_provider_family_slug(&suno), "suno");
        assert_eq!(derive_service_provider_slug(&suno), "suno-platform");
        assert_eq!(derive_provider_surface_slug(&suno), "suno");
        assert_eq!(canonicalize_folder_surface_slug("suno-music"), "suno");
        assert_eq!(
            default_folder_sync_relative_path(
                &suno,
                &build_test_credential("cred-suno", "folder_sync", None, "active", None),
            ),
            "suno-platform/suno/session-auth/cred-suno.json"
        );

        let mut udio = build_test_provider_account(
            "Udio Images",
            "udio_compatible",
            "udio_images",
            "udio",
            "https://www.udio.com",
            Some("web_reverse_api"),
            Some("browser_challenge"),
        );
        udio.service_provider_key = "udio_platform".to_string();
        udio.service_provider_label = "Udio Platform".to_string();
        assert_eq!(derive_provider_family_slug(&udio), "udio");
        assert_eq!(derive_service_provider_slug(&udio), "udio-platform");
        assert_eq!(derive_provider_surface_slug(&udio), "udio");
        assert_eq!(canonicalize_folder_surface_slug("udio-videos"), "udio");
        assert_eq!(
            default_folder_sync_relative_path(
                &udio,
                &build_test_credential("cred-udio", "folder_sync", None, "active", None),
            ),
            "udio-platform/udio/session-auth/cred-udio.json"
        );

        let mut lumalabs = build_test_provider_account(
            "LumaLabs Videos",
            "lumalabs_compatible",
            "lumalabs_videos",
            "lumalabs",
            "https://app.lumalabs.ai",
            Some("web_reverse_api"),
            Some("browser_challenge"),
        );
        lumalabs.service_provider_key = "lumalabs_platform".to_string();
        lumalabs.service_provider_label = "LumaLabs Platform".to_string();
        assert_eq!(derive_provider_family_slug(&lumalabs), "lumalabs");
        assert_eq!(derive_service_provider_slug(&lumalabs), "lumalabs-platform");
        assert_eq!(derive_provider_surface_slug(&lumalabs), "lumalabs");
        assert_eq!(
            canonicalize_folder_surface_slug("luma-labs-videos"),
            "lumalabs"
        );
        assert_eq!(
            default_folder_sync_relative_path(
                &lumalabs,
                &build_test_credential("cred-luma", "folder_sync", None, "active", None),
            ),
            "lumalabs-platform/lumalabs/session-auth/cred-luma.json"
        );
    }

    #[test]
    fn derive_provider_surface_slug_maps_search_family_lines() {
        let mut perplexity = build_test_provider_account(
            "Perplexity Search",
            "search_api_compatible",
            "perplexity_search",
            "perplexity_search",
            "https://api.perplexity.ai",
            Some("official_vendor_api"),
            None,
        );
        perplexity.service_provider_key = "perplexity_platform".to_string();
        perplexity.service_provider_label = "Perplexity Search".to_string();
        assert_eq!(
            derive_provider_surface_slug(&perplexity),
            "perplexity-search"
        );

        let mut tavily = build_test_provider_account(
            "Tavily Search",
            "search_api_compatible",
            "tavily_search",
            "tavily",
            "https://api.tavily.com",
            Some("official_vendor_api"),
            None,
        );
        tavily.service_provider_key = "tavily_platform".to_string();
        tavily.service_provider_label = "Tavily Search".to_string();
        assert_eq!(derive_provider_surface_slug(&tavily), "tavily-search");

        let mut exa = build_test_provider_account(
            "Exa Search",
            "search_api_compatible",
            "exa_search",
            "exa",
            "https://api.exa.ai",
            Some("official_vendor_api"),
            None,
        );
        exa.service_provider_key = "exa_platform".to_string();
        exa.service_provider_label = "Exa Search".to_string();
        assert_eq!(derive_provider_surface_slug(&exa), "exa-search");

        let mut jina_search = build_test_provider_account(
            "Jina Search",
            "search_api_compatible",
            "jina_search",
            "jina_search",
            "https://s.jina.ai",
            Some("official_vendor_api"),
            None,
        );
        jina_search.service_provider_key = "jina_platform".to_string();
        jina_search.service_provider_label = "Jina Search".to_string();
        assert_eq!(derive_provider_surface_slug(&jina_search), "jina-search");

        let mut jina_reader = build_test_provider_account(
            "Jina Reader",
            "search_api_compatible",
            "jina_reader",
            "jina_reader",
            "https://r.jina.ai",
            Some("official_vendor_api"),
            None,
        );
        jina_reader.service_provider_key = "jina_platform".to_string();
        jina_reader.service_provider_label = "Jina Reader".to_string();
        assert_eq!(derive_provider_surface_slug(&jina_reader), "jina-reader");

        let mut linkup = build_test_provider_account(
            "Linkup Search",
            "search_api_compatible",
            "linkup_search",
            "linkup",
            "https://api.linkup.so",
            Some("official_vendor_api"),
            None,
        );
        linkup.service_provider_key = "linkup_platform".to_string();
        linkup.service_provider_label = "Linkup Search".to_string();
        assert_eq!(derive_provider_surface_slug(&linkup), "linkup-search");

        let mut you = build_test_provider_account(
            "You.com Search",
            "search_api_compatible",
            "you_search",
            "you_search",
            "https://api.ydc-index.io",
            Some("official_vendor_api"),
            None,
        );
        you.service_provider_key = "you_platform".to_string();
        you.service_provider_label = "You.com Search".to_string();
        assert_eq!(derive_provider_surface_slug(&you), "you-search");

        let mut websearchapi = build_test_provider_account(
            "WebSearchAPI Search",
            "search_api_compatible",
            "websearchapi_search",
            "websearchapi",
            "https://api.websearchapi.ai",
            Some("official_vendor_api"),
            None,
        );
        websearchapi.service_provider_key = "websearchapi_platform".to_string();
        websearchapi.service_provider_label = "WebSearchAPI Search".to_string();
        assert_eq!(
            derive_provider_surface_slug(&websearchapi),
            "websearchapi-search"
        );
    }

    #[test]
    fn derive_provider_surface_slug_maps_remaining_unfinished_platform_lines() {
        let mut xai = build_test_provider_account(
            "xAI OpenAI",
            "openai_compatible",
            "openai_compatible",
            "xai",
            "https://api.x.ai/v1",
            Some("official_vendor_api"),
            None,
        );
        xai.service_provider_key = "xai_platform".to_string();
        xai.service_provider_label = "xAI Platform".to_string();
        assert_eq!(derive_provider_family_slug(&xai), "xai");
        assert_eq!(derive_service_provider_slug(&xai), "xai-platform");
        assert_eq!(derive_provider_surface_slug(&xai), "xai-openai");
        assert_eq!(canonicalize_folder_surface_slug("xai"), "xai-openai");
        assert_eq!(
            default_folder_sync_relative_path(
                &xai,
                &build_test_credential("cred-xai", "folder_sync", None, "active", None),
            ),
            "xai-platform/xai-openai/api-key/cred-xai.json"
        );

        let mut perplexity_chat = build_test_provider_account(
            "Perplexity Chat",
            "openai_compatible",
            "openai_compatible",
            "perplexity_chat",
            "https://api.perplexity.ai",
            Some("official_vendor_api"),
            None,
        );
        perplexity_chat.service_provider_key = "perplexity_platform".to_string();
        perplexity_chat.service_provider_label = "Perplexity Platform".to_string();
        assert_eq!(derive_provider_family_slug(&perplexity_chat), "perplexity");
        assert_eq!(
            derive_service_provider_slug(&perplexity_chat),
            "perplexity-platform"
        );
        assert_eq!(
            derive_provider_surface_slug(&perplexity_chat),
            "perplexity-chat"
        );
        assert_eq!(
            default_folder_sync_relative_path(
                &perplexity_chat,
                &build_test_credential("cred-perplexity", "folder_sync", None, "active", None),
            ),
            "perplexity-platform/perplexity-chat/api-key/cred-perplexity.json"
        );

        let mut freebuff = build_test_provider_account(
            "FreeBuff",
            "freebuff_compatible",
            "freebuff",
            "freebuff",
            "https://www.codebuff.com",
            Some("web_reverse_api"),
            Some("browser_challenge"),
        );
        freebuff.service_provider_key = "freebuff_platform".to_string();
        freebuff.service_provider_label = "FreeBuff".to_string();
        assert_eq!(derive_provider_family_slug(&freebuff), "freebuff");
        assert_eq!(
            derive_provider_surface_slug(&freebuff),
            "freebuff-compatible"
        );
        assert_eq!(
            default_folder_sync_relative_path(
                &freebuff,
                &build_test_credential("cred-freebuff", "folder_sync", None, "active", None),
            ),
            "freebuff-platform/freebuff-compatible/session-auth/cred-freebuff.json"
        );

        let mut xfyun_openai = build_test_provider_account(
            "XFYun OpenAI",
            "openai_compatible",
            "openai_compatible",
            "xfyun_openai",
            "https://spark-api-open.xf-yun.com/v1",
            Some("official_vendor_api"),
            None,
        );
        xfyun_openai.service_provider_key = "xfyun_platform".to_string();
        xfyun_openai.service_provider_label = "XFYun Platform".to_string();
        assert_eq!(derive_provider_family_slug(&xfyun_openai), "xfyun");
        assert_eq!(derive_provider_surface_slug(&xfyun_openai), "xfyun-openai");
        assert_eq!(
            default_folder_sync_relative_path(
                &xfyun_openai,
                &build_test_credential("cred-xfyun-openai", "folder_sync", None, "active", None),
            ),
            "xfyun-platform/xfyun-openai/api-key/cred-xfyun-openai.json"
        );

        let mut xfyun_native = build_test_provider_account(
            "XFYun Native WebSocket",
            "xfyun_websocket_compatible",
            "xfyun_websocket",
            "xfyun_native_websocket",
            "wss://spark-api.xf-yun.com/v1.1/chat",
            Some("official_vendor_api"),
            None,
        );
        xfyun_native.service_provider_key = "xfyun_platform".to_string();
        xfyun_native.service_provider_label = "XFYun Platform".to_string();
        assert_eq!(derive_provider_family_slug(&xfyun_native), "xfyun");
        assert_eq!(
            derive_provider_surface_slug(&xfyun_native),
            "xfyun-native-websocket"
        );
        assert_eq!(
            default_folder_sync_relative_path(
                &xfyun_native,
                &build_test_credential("cred-xfyun-native", "folder_sync", None, "active", None),
            ),
            "xfyun-platform/xfyun-native-websocket/api-key/cred-xfyun-native.json"
        );

        let mut producer_music = build_test_provider_account(
            "Producer Music",
            "producer_compatible",
            "producer_music",
            "producer",
            "https://www.flowmusic.app",
            Some("web_reverse_api"),
            Some("browser_challenge"),
        );
        producer_music.service_provider_key = "producer_platform".to_string();
        producer_music.service_provider_label = "Producer.ai Platform".to_string();
        assert_eq!(derive_provider_family_slug(&producer_music), "producer");
        assert_eq!(
            derive_provider_surface_slug(&producer_music),
            "producer-music"
        );
        assert_eq!(
            canonicalize_folder_surface_slug("producer-videos"),
            "producer-videos"
        );
        assert_eq!(
            default_folder_sync_relative_path(
                &producer_music,
                &build_test_credential("cred-producer", "folder_sync", None, "active", None),
            ),
            "producer-platform/producer-music/session-auth/cred-producer.json"
        );

        let mut kiro = build_test_provider_account(
            "Kiro-compatible",
            "kiro_compatible",
            "kiro",
            "kiro",
            "https://codewhisperer.us-east-1.amazonaws.com",
            Some("official_vendor_api"),
            None,
        );
        kiro.service_provider_key = "kiro_platform".to_string();
        kiro.service_provider_label = "Kiro Platform".to_string();
        assert_eq!(derive_provider_family_slug(&kiro), "kiro");
        assert_eq!(derive_provider_surface_slug(&kiro), "kiro-compatible");
        assert_eq!(
            default_folder_sync_relative_path(
                &kiro,
                &build_test_credential("cred-kiro", "folder_sync", None, "active", None),
            ),
            "kiro-platform/kiro-compatible/bearer-token/cred-kiro.json"
        );
    }

    #[test]
    fn default_folder_sync_relative_path_uses_wave3_service_surface_layout() {
        let mut azure = build_test_provider_account(
            "Azure OpenAI",
            "openai_compatible",
            "openai",
            "azure_openai",
            "https://example.openai.azure.com/openai/v1",
            Some("official_vendor_api"),
            None,
        );
        azure.service_provider_key = "azure_openai_platform".to_string();
        azure.service_provider_label = "Azure OpenAI".to_string();
        let mut azure_credential =
            build_test_credential("cred-azure", "folder_sync", None, "active", None);
        azure_credential.payload = serde_json::json!({"apiKey":"sk-azure"});
        assert_eq!(
            default_folder_sync_relative_path(&azure, &azure_credential),
            "azure-openai-platform/azure-openai/api-key/cred-azure.json"
        );

        let mut bedrock = build_test_provider_account(
            "AWS Bedrock Converse",
            "bedrock_converse_compatible",
            "bedrock_converse",
            "aws_bedrock",
            "https://bedrock-runtime.us-east-1.amazonaws.com",
            Some("official_model_api"),
            None,
        );
        bedrock.service_provider_key = "aws_bedrock_platform".to_string();
        bedrock.service_provider_label = "AWS Bedrock Converse".to_string();
        let mut bedrock_credential =
            build_test_credential("cred-bedrock", "folder_sync", None, "active", None);
        bedrock_credential.payload = serde_json::json!({"authToken":"signed-token"});
        assert_eq!(
            default_folder_sync_relative_path(&bedrock, &bedrock_credential),
            "aws-bedrock-platform/bedrock-converse/bearer-token/cred-bedrock.json"
        );
    }

    #[test]
    fn derive_credential_material_kind_defaults_cover_gemini_modular_profiles() {
        let mut official = build_test_provider_account(
            "Gemini API Modular",
            "gemini_api_modular_compatible",
            "gemini_generate_content",
            GEMINI_API_MODULAR_PROFILE,
            "https://example.invalid",
            Some("official_model_api"),
            None,
        );
        official.service_provider_key = "gemini_platform".to_string();
        official.service_provider_label = "Gemini Platform".to_string();
        assert_eq!(derive_credential_material_kind(&official, None), "api_key");

        let mut web = build_test_provider_account(
            "Gemini Web Reverse Modular",
            "gemini_web_reverse_modular_compatible",
            "gemini_web_chat",
            GEMINI_WEB_REVERSE_MODULAR_PROFILE,
            "https://example.invalid",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        web.service_provider_key = "gemini_platform".to_string();
        web.service_provider_label = "Gemini Platform".to_string();
        assert_eq!(derive_credential_material_kind(&web, None), "session_auth");

        let mut browser = build_test_provider_account(
            "Gemini Canvas Browser Relay",
            "gemini_canvas_web_reverse_compatible",
            "gemini_canvas_images",
            GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE,
            "https://example.invalid",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        browser.service_provider_key = "gemini_platform".to_string();
        browser.service_provider_label = "Gemini Platform".to_string();
        assert_eq!(
            derive_credential_material_kind(&browser, None),
            "browser_state"
        );
    }

    #[test]
    fn default_folder_sync_relative_path_uses_modular_gemini_surface_slugs() {
        let mut browser = build_test_provider_account(
            "Gemini Canvas Browser Relay",
            "gemini_canvas_web_reverse_compatible",
            "gemini_canvas_images",
            GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE,
            "https://gemini.google.com",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        browser.service_provider_key = "gemini_platform".to_string();
        browser.service_provider_label = "Gemini Platform".to_string();

        let mut browser_credential =
            build_test_credential("cred-browser", "folder_sync", None, "active", None);
        browser_credential.payload = serde_json::json!({
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/browser/storage-state.json"
        });

        assert_eq!(
            default_folder_sync_relative_path(&browser, &browser_credential),
            "gemini-platform/gemini-canvas-browser-relay/browser-state/cred-browser.json"
        );

        let mut program = build_test_provider_account(
            "Gemini Canvas Program Relay",
            "gemini_canvas_program_web_reverse_compatible",
            "gemini_canvas_images",
            GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
            "https://gemini.google.com",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        program.service_provider_key = "gemini_platform".to_string();
        program.service_provider_label = "Gemini Platform".to_string();

        let mut program_credential =
            build_test_credential("cred-program", "folder_sync", None, "active", None);
        program_credential.payload = serde_json::json!({
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json"
        });

        assert_eq!(
            default_folder_sync_relative_path(&program, &program_credential),
            "gemini-platform/gemini-canvas-program-relay/browser-state/cred-program.json"
        );
    }

    #[test]
    fn normalize_qwen_web_import_payload_converts_browser_worker_output() {
        let provider = build_test_provider_account(
            "Qwen WebUI Replay Live",
            "qwen_web_compatible",
            "qwen_web_chat",
            "qwen_web_chat",
            "https://chat.qwen.ai",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        let payload = normalize_import_payload(
            "qwen-web-chat",
            &provider,
            serde_json::json!({
                "authToken": "token-123",
                "cookieHeader": "token=token-123; other=1",
                "expiresAt": "2099-01-01T00:00:00.000Z",
                "selectedModel": "qwen3-coder-plus",
                "selectedDisplayModel": "Qwen3-Coder",
                "authProbe": {
                    "userId": "user-123",
                    "email": "user@example.com"
                }
            }),
            None,
        )
        .expect("normalized qwen web payload");
        assert_eq!(
            payload.get("apiKey").and_then(Value::as_str),
            Some("token-123")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Cookie"))
                .and_then(Value::as_str),
            Some("token=token-123; other=1")
        );
        assert_eq!(
            payload
                .get("supportedModels")
                .and_then(Value::as_array)
                .and_then(|models| models.first())
                .and_then(Value::as_str),
            Some("qwen3-coder-plus")
        );
        assert_eq!(
            payload.get("selectedDisplayModel").and_then(Value::as_str),
            Some("Qwen3-Coder")
        );
        assert_eq!(
            payload.get("credentialMaterialKey").and_then(Value::as_str),
            Some("qwen-web-user:user-123")
        );
        assert_eq!(
            payload.get("serviceProviderKey").and_then(Value::as_str),
            Some("qwen_platform")
        );
        assert_eq!(
            payload.get("providerSurfaceKey").and_then(Value::as_str),
            Some("qwen-web-chat")
        );
        assert_eq!(
            payload
                .get("credentialMaterialKind")
                .and_then(Value::as_str),
            Some("session_auth")
        );
        assert_eq!(
            payload.get("accountName").and_then(Value::as_str),
            Some("user@example.com")
        );
    }

    #[test]
    fn normalize_qwen_web_import_payload_preserves_auth_seed() {
        let provider = build_test_provider_account(
            "Qwen WebUI Replay Live",
            "qwen_web_compatible",
            "qwen_web_chat",
            "qwen_web_chat",
            "https://chat.qwen.ai",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        let payload = normalize_import_payload(
            "qwen-web-chat",
            &provider,
            serde_json::json!({
                "authToken": "token-123",
                "email": "user@example.com",
                "passwordSha256": "abcdef123456"
            }),
            None,
        )
        .expect("normalized qwen web payload with auth seed");
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|body| body.get("authSeed"))
                .and_then(Value::as_object)
                .and_then(|seed| seed.get("email"))
                .and_then(Value::as_str),
            Some("user@example.com")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|body| body.get("authSeed"))
                .and_then(Value::as_object)
                .and_then(|seed| seed.get("passwordSha256"))
                .and_then(Value::as_str),
            Some("abcdef123456")
        );
    }

    #[test]
    fn normalize_chataibot_import_payload_accepts_manual_token_source() {
        let mut provider = build_test_provider_account(
            "ChatAIBot Images",
            "chataibot_compatible",
            "chataibot_images",
            "chataibot",
            "https://chataibot.pro",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        provider.service_provider_key = "chataibot_platform".to_string();
        provider.service_provider_label = "ChatAIBot".to_string();
        provider.payload = serde_json::json!({
            "baseUrl": "https://chataibot.pro",
            "defaultModel": "google-nano-banana-2"
        });

        let payload = normalize_import_payload(
            "chataibot-images",
            &provider,
            serde_json::json!({
                "authToken": "token-456",
                "cookieHeader": "cf_clearance=abc; locale=en",
                "selectedModel": "google-nano-banana-2",
                "accountName": "artist@example.com",
                "userId": "user-456",
                "expiresAt": "2099-01-01T00:00:00.000Z"
            }),
            None,
        )
        .expect("normalized chataibot payload from manual token source");

        assert_eq!(
            payload.get("apiKey").and_then(Value::as_str),
            Some("token-456")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Cookie"))
                .and_then(Value::as_str),
            Some("token=token-456; cf_clearance=abc; locale=en")
        );
        assert_eq!(
            payload
                .get("supportedModels")
                .and_then(Value::as_array)
                .and_then(|models| models.first())
                .and_then(Value::as_str),
            Some("google-nano-banana-2")
        );
        assert_eq!(
            payload.get("accountName").and_then(Value::as_str),
            Some("artist@example.com")
        );
        assert_eq!(
            payload.get("credentialMaterialKey").and_then(Value::as_str),
            Some("chataibot-user:user-456")
        );
        assert_eq!(
            payload.get("providerSurfaceKey").and_then(Value::as_str),
            Some("chataibot-images")
        );
        assert_eq!(
            payload
                .get("credentialMaterialKind")
                .and_then(Value::as_str),
            Some("session_auth")
        );
        assert_eq!(
            payload.get("serviceProviderKey").and_then(Value::as_str),
            Some("chataibot_platform")
        );
    }

    #[test]
    fn normalize_chataibot_import_payload_accepts_session_worker_output_shape() {
        let mut provider = build_test_provider_account(
            "ChatAIBot Images",
            "chataibot_compatible",
            "chataibot_images",
            "chataibot",
            "https://chataibot.pro",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        provider.service_provider_key = "chataibot_platform".to_string();
        provider.service_provider_label = "ChatAIBot".to_string();
        provider.payload = serde_json::json!({
            "baseUrl": "https://chataibot.pro",
            "defaultModel": "qwen-lora"
        });

        let payload = normalize_import_payload(
            "chataibot-images",
            &provider,
            serde_json::json!({
                "apiKey": "token-123",
                "headers": {
                    "Cookie": "token=token-123; cf_clearance=abc"
                },
                "supportedModels": ["qwen-lora", "google-nano-banana-2"],
                "selectedDisplayModel": "ChatAIBot Free Images",
                "accountName": "artist@example.com",
                "credentialMaterialKey": "chataibot-user:user-123",
                "expiresAt": "2099-01-01T00:00:00.000Z"
            }),
            None,
        )
        .expect("normalized chataibot payload from worker output");

        assert_eq!(
            payload.get("apiKey").and_then(Value::as_str),
            Some("token-123")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Cookie"))
                .and_then(Value::as_str),
            Some("token=token-123; cf_clearance=abc")
        );
        assert_eq!(
            payload
                .get("supportedModels")
                .and_then(Value::as_array)
                .map(|models| { models.iter().filter_map(Value::as_str).collect::<Vec<_>>() }),
            Some(vec!["qwen-lora", "google-nano-banana-2"])
        );
        assert_eq!(
            payload.get("selectedDisplayModel").and_then(Value::as_str),
            Some("ChatAIBot Free Images")
        );
        assert_eq!(
            payload.get("credentialMaterialKey").and_then(Value::as_str),
            Some("chataibot-user:user-123")
        );
        assert_eq!(
            payload
                .get("credentialMaterialKind")
                .and_then(Value::as_str),
            Some("session_auth")
        );
    }

    #[test]
    fn normalize_gemini_web_import_payload_converts_cookie_session_source() {
        let mut provider = build_test_provider_account(
            "Gemini Web Chat",
            "gemini_web_compatible",
            "gemini_web_chat",
            "gemini_web",
            "https://gemini.google.com",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        provider.service_provider_key = "gemini_platform".to_string();
        provider.service_provider_label = "Gemini Platform".to_string();

        let payload = normalize_import_payload(
            "gemini-web-chat",
            &provider,
            serde_json::json!({
                "__Secure-1PSID": "psid-primary",
                "__Secure-1PSIDTS": "psidts-secondary",
                "cookieHeader": "__Secure-1PSID=psid-primary; __Secure-1PSIDTS=psidts-secondary; NID=test",
                "defaultModel": "gemini-web-chat-live",
                "accessToken": "snlm0e-token",
                "buildLabel": "boq-gemini",
                "sessionId": "fsid-1",
                "language": "en",
                "requestContextHeader": "[\"ctx\",1]",
                "credentialMaterialKey": "gemini-web-session-main"
            }),
            Some("session_auth"),
        )
        .expect("normalized gemini web payload");

        assert_eq!(
            payload.get("apiKey").and_then(Value::as_str),
            Some("psid-primary")
        );
        assert_eq!(
            payload.get("authToken").and_then(Value::as_str),
            Some("psidts-secondary")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Cookie"))
                .and_then(Value::as_str),
            Some("__Secure-1PSID=psid-primary; __Secure-1PSIDTS=psidts-secondary; NID=test")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("accessToken"))
                .and_then(Value::as_str),
            Some("snlm0e-token")
        );
        assert_eq!(
            payload.get("providerSurfaceKey").and_then(Value::as_str),
            Some("gemini-web-chat")
        );
        assert_eq!(
            payload
                .get("credentialMaterialKind")
                .and_then(Value::as_str),
            Some("session_auth")
        );
    }

    #[test]
    fn normalize_accio_import_payload_converts_manager_account_file() {
        let provider = build_test_provider_account(
            "Accio Live",
            "accio_compatible",
            "accio",
            "openai_responses",
            "https://phoenix-gw.alibaba.com",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        let payload = normalize_import_payload(
            "accio",
            &provider,
            serde_json::json!({
                "id": "acct-123",
                "name": "Accio Team",
                "email": "accio@example.com",
                "accessToken": "accio-token-123",
                "refreshToken": "refresh-123",
                "utdid": "utd-accio-123",
                "cookie": "cna=test-cna; other=value",
                "disabledModels": {
                    "claude-opus-4-6": "quota_empty"
                },
                "expiresAt": "2099-01-01T00:00:00.000Z"
            }),
            None,
        )
        .expect("normalized accio payload");
        assert_eq!(
            payload.get("apiKey").and_then(Value::as_str),
            Some("accio-token-123")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("utdid"))
                .and_then(Value::as_str),
            Some("utd-accio-123")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("x-cna"))
                .and_then(Value::as_str),
            Some("test-cna")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("token"))
                .and_then(Value::as_str),
            Some("accio-token-123")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("accessToken"))
                .and_then(Value::as_str),
            Some("accio-token-123")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("empid"))
                .and_then(Value::as_str),
            Some("acct-123")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("tenant"))
                .and_then(Value::as_str),
            Some("")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("iaiTag"))
                .and_then(Value::as_str),
            Some("phoenix-desktop")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Cookie"))
                .and_then(Value::as_str),
            Some("cna=test-cna; other=value")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("x-language"))
                .and_then(Value::as_str),
            Some("zh-CN")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("x-os"))
                .and_then(Value::as_str),
            Some("win32")
        );
        assert_eq!(
            payload
                .get("excludedModels")
                .and_then(Value::as_array)
                .and_then(|models| models.first())
                .and_then(Value::as_str),
            Some("claude-opus-4-6")
        );
        assert_eq!(
            payload.get("credentialMaterialKey").and_then(Value::as_str),
            Some("accio-account:acct-123")
        );
        assert_eq!(
            payload.get("accountName").and_then(Value::as_str),
            Some("accio@example.com")
        );
    }

    #[test]
    fn normalize_chatgpt_web_import_payload_accepts_easyregister_failed_twice_shape() {
        let mut provider = build_test_provider_account(
            "ChatGPT Web Reverse",
            "chatgpt_web_reverse_compatible",
            "chatgpt_web_chat",
            "chatgpt_web_reverse",
            "https://chatgpt.com/backend-api/conversation",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        provider.service_provider_key = "chatgpt_platform".to_string();
        provider.service_provider_label = "ChatGPT Platform".to_string();

        let payload = normalize_import_payload(
            "chatgpt-web-reverse",
            &provider,
            serde_json::json!({
                "email": "fixture@example.com",
                "password": "fixture-password-123",
                "platformAuth": {
                    "deviceId": "platform-device-id-1"
                },
                "chatgptLogin": {
                    "deviceId": "chatgpt-login-device-id-1",
                    "authUrl": "https://auth.openai.com/api/accounts/authorize?client_id=fixture",
                    "mailboxRef": "moemail:fixture-session",
                    "mailboxSessionId": "fixture-session"
                },
                "chatgptLoginDetails": {
                    "oauthTokens": {
                        "refresh_token": "fixture-refresh-token",
                        "id_token": "fixture-id-token"
                    },
                    "clientBootstrap": {
                        "planType": "free",
                        "accessToken": "eyJhbGciOiJub25lIn0.eyJleHAiOjQxMDAwMDAwMDAsInNlc3Npb25faWQiOiJhdXRoc2Vzc19jaGF0Z3B0X2ZpeHR1cmUifQ."
                    }
                }
            }),
            Some("session_auth"),
        )
        .expect("normalized chatgpt web payload from easyregister shape");
        assert_eq!(
            payload.get("apiKey").and_then(Value::as_str),
            Some("eyJhbGciOiJub25lIn0.eyJleHAiOjQxMDAwMDAwMDAsInNlc3Npb25faWQiOiJhdXRoc2Vzc19jaGF0Z3B0X2ZpeHR1cmUifQ.")
        );
        assert_eq!(
            payload.get("baseUrl").and_then(Value::as_str),
            Some("https://chatgpt.com")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("deviceId"))
                .and_then(Value::as_str),
            Some("chatgpt-login-device-id-1")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("chatgptAuthUrl"))
                .and_then(Value::as_str),
            Some("https://auth.openai.com/api/accounts/authorize?client_id=fixture")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("mailboxRef"))
                .and_then(Value::as_str),
            Some("moemail:fixture-session")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("mailboxSessionId"))
                .and_then(Value::as_str),
            Some("fixture-session")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("authSeed"))
                .and_then(Value::as_object)
                .and_then(|seed| seed.get("email"))
                .and_then(Value::as_str),
            Some("fixture@example.com")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("authSeed"))
                .and_then(Value::as_object)
                .and_then(|seed| seed.get("password"))
                .and_then(Value::as_str),
            Some("fixture-password-123")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("sessionId"))
                .and_then(Value::as_str),
            Some("authsess_chatgpt_fixture")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("refreshToken"))
                .and_then(Value::as_str),
            Some("fixture-refresh-token")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("refreshStrategy"))
                .and_then(Value::as_str),
            Some("oauth_token")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("idToken"))
                .and_then(Value::as_str),
            Some("fixture-id-token")
        );
        assert_eq!(
            payload.get("expiresAt").and_then(Value::as_str),
            Some("2099-12-03T16:53:20Z")
        );
        assert_eq!(
            payload
                .get("credentialMaterialKind")
                .and_then(Value::as_str),
            Some("session_auth")
        );
    }

    #[test]
    fn normalize_chatgpt_codex_backend_import_payload_uses_chatgpt_surface_slug() {
        let mut provider = build_test_provider_account(
            "ChatGPT Codex Backend",
            "openai_compatible",
            "openai",
            "chatgpt_codex_backend",
            "https://chatgpt.com/backend-api/codex",
            Some("official_vendor_api"),
            None,
        );
        provider.service_provider_key = "chatgpt_platform".to_string();
        provider.service_provider_label = "ChatGPT Platform".to_string();

        let payload = normalize_import_payload(
            "chatgpt-codex-backend",
            &provider,
            serde_json::json!({
                "access_token": "codex-token-123",
                "account_id": "acct-123"
            }),
            None,
        )
        .expect("normalized chatgpt codex payload");

        assert_eq!(
            payload.get("apiKey").and_then(Value::as_str),
            Some("codex-token-123")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Originator"))
                .and_then(Value::as_str),
            Some("codex_cli_rs")
        );
    }

    #[test]
    fn normalize_chatgpt_codex_backend_import_payload_accepts_easyregister_failed_twice_shape() {
        let mut provider = build_test_provider_account(
            "ChatGPT Codex Backend",
            "openai_compatible",
            "openai",
            "chatgpt_codex_backend",
            "https://chatgpt.com/backend-api/codex",
            Some("official_vendor_api"),
            None,
        );
        provider.service_provider_key = "chatgpt_platform".to_string();
        provider.service_provider_label = "ChatGPT Platform".to_string();

        let payload = normalize_import_payload(
            "chatgpt-codex-backend",
            &provider,
            serde_json::json!({
                "email": "fixture@example.com",
                "chatgptLogin": {
                    "workspaceId": "workspace-id-1"
                },
                "chatgptLoginDetails": {
                    "clientBootstrap": {
                        "accountId": "account-id-1",
                        "accessToken": "codex-token-from-easyregister"
                    }
                }
            }),
            None,
        )
        .expect("normalized chatgpt codex payload from easyregister shape");

        assert_eq!(
            payload.get("apiKey").and_then(Value::as_str),
            Some("codex-token-from-easyregister")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Chatgpt-Account-Id"))
                .and_then(Value::as_str),
            Some("account-id-1")
        );
        assert_eq!(
            payload
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Originator"))
                .and_then(Value::as_str),
            Some("codex_cli_rs")
        );
    }

    #[test]
    fn normalize_chatgpt_codex_backend_payload_merges_without_duplicate_base_url_fields() {
        let mut provider = build_test_provider_account(
            "ChatGPT Codex Backend",
            "openai_compatible",
            "openai",
            "chatgpt_codex_backend",
            "https://chatgpt.com/backend-api/codex",
            Some("official_vendor_api"),
            None,
        );
        provider.service_provider_key = "chatgpt_platform".to_string();
        provider.service_provider_label = "ChatGPT Platform".to_string();

        let credential_payload = normalize_import_payload(
            "chatgpt-codex-backend",
            &provider,
            serde_json::json!({
                "access_token": "codex-token-123",
                "account_id": "acct-123"
            }),
            None,
        )
        .expect("normalized chatgpt codex payload");

        let merged = crate::db::merge_provider_account_and_credential_payloads(
            &provider.payload,
            &credential_payload,
        );
        let serde_compatible =
            crate::http::routes::internal_provider_accounts::make_provider_payload_serde_compatible(
                &provider, &merged,
            );

        let parsed = serde_json::from_value::<crate::routing::candidate::ProviderAccountPayload>(
            serde_compatible,
        );
        assert!(
            parsed.is_ok(),
            "merged codex payload should deserialize cleanly, got: {parsed:?}"
        );
    }

    #[test]
    fn describe_folder_import_path_supports_nested_service_surface_material_layout() {
        let descriptor = describe_folder_import_path(
            "gemini-platform/gemini-canvas-images/browser-state/main.json",
        )
        .expect("descriptor");
        assert_eq!(
            descriptor.service_provider_slug.as_deref(),
            Some("gemini-platform")
        );
        assert_eq!(descriptor.provider_surface_slug, "gemini-canvas-images");
        assert_eq!(
            descriptor.credential_material_kind.as_deref(),
            Some("browser_state")
        );
    }

    #[test]
    fn default_folder_sync_relative_path_uses_nested_rollout_for_qwen_and_gemini() {
        let mut qwen_provider = build_test_provider_account(
            "Qwen DashScope OpenAI",
            "openai_compatible",
            "openai",
            "qwen_dashscope_openai",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            Some("official_model_api"),
            None,
        );
        qwen_provider.service_provider_key = "qwen_platform".to_string();
        qwen_provider.service_provider_label = "Qwen Platform".to_string();

        let mut qwen_credential =
            build_test_credential("cred-qwen", "folder_sync", None, "active", None);
        qwen_credential.payload = serde_json::json!({
            "apiKey": "sk-test"
        });

        assert_eq!(
            default_folder_sync_relative_path(&qwen_provider, &qwen_credential),
            "qwen-platform/qwen-dashscope-openai/api-key/cred-qwen.json"
        );

        let mut gemini_provider = build_test_provider_account(
            "Gemini Canvas Images",
            "gemini_canvas_compatible",
            "gemini_canvas_images",
            "gemini_canvas",
            "https://gemini.google.com",
            Some("web_reverse_api"),
            Some("browser_challenge"),
        );
        gemini_provider.service_provider_key = "gemini_platform".to_string();
        gemini_provider.service_provider_label = "Gemini Platform".to_string();

        let mut gemini_credential =
            build_test_credential("cred-gemini", "folder_sync", None, "active", None);
        gemini_credential.payload = serde_json::json!({
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/main/storage-state.json"
        });

        assert_eq!(
            default_folder_sync_relative_path(&gemini_provider, &gemini_credential),
            "gemini-platform/gemini-canvas-images/browser-state/cred-gemini.json"
        );
    }

    #[test]
    fn default_folder_sync_relative_path_uses_nested_rollout_for_chataibot() {
        let mut provider = build_test_provider_account(
            "ChatAIBot Images",
            "chataibot_compatible",
            "chataibot_images",
            "chataibot",
            "https://chataibot.pro",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        provider.service_provider_key = "chataibot_platform".to_string();
        provider.service_provider_label = "ChatAIBot".to_string();

        let mut credential =
            build_test_credential("cred-chataibot", "folder_sync", None, "active", None);
        credential.payload = serde_json::json!({
            "apiKey": "token-123"
        });

        assert_eq!(
            default_folder_sync_relative_path(&provider, &credential),
            "chataibot-platform/chataibot-images/session-auth/cred-chataibot.json"
        );
    }

    #[test]
    fn default_folder_sync_relative_path_covers_all_qwen_canonical_surfaces() {
        let mut dashscope_provider = build_test_provider_account(
            "Qwen DashScope OpenAI",
            "openai_compatible",
            "openai",
            "qwen_dashscope_openai",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            Some("official_model_api"),
            None,
        );
        dashscope_provider.service_provider_key = "qwen_platform".to_string();
        dashscope_provider.service_provider_label = "Qwen Platform".to_string();

        let mut dashscope_credential =
            build_test_credential("cred-qwen-dashscope", "folder_sync", None, "active", None);
        dashscope_credential.payload = serde_json::json!({ "apiKey": "sk-test-dashscope" });
        assert_eq!(
            default_folder_sync_relative_path(&dashscope_provider, &dashscope_credential),
            "qwen-platform/qwen-dashscope-openai/api-key/cred-qwen-dashscope.json"
        );

        let mut coding_openai_provider = build_test_provider_account(
            "Qwen Coding Plan OpenAI",
            "openai_compatible",
            "openai",
            "qwen_coding_plan_openai",
            "https://coding.dashscope.aliyuncs.com/v1",
            Some("official_model_api"),
            None,
        );
        coding_openai_provider.service_provider_key = "qwen_platform".to_string();
        coding_openai_provider.service_provider_label = "Qwen Platform".to_string();

        let mut coding_openai_credential = build_test_credential(
            "cred-qwen-coding-openai",
            "folder_sync",
            None,
            "active",
            None,
        );
        coding_openai_credential.payload = serde_json::json!({ "apiKey": "sk-test-coding-openai" });
        assert_eq!(
            default_folder_sync_relative_path(&coding_openai_provider, &coding_openai_credential),
            "qwen-platform/qwen-coding-plan-openai/api-key/cred-qwen-coding-openai.json"
        );

        let mut coding_anthropic_provider = build_test_provider_account(
            "Qwen Coding Plan Anthropic",
            "anthropic_compatible",
            "anthropic",
            "qwen_coding_plan_anthropic",
            "https://coding.dashscope.aliyuncs.com/apps/anthropic",
            Some("official_model_api"),
            None,
        );
        coding_anthropic_provider.service_provider_key = "qwen_platform".to_string();
        coding_anthropic_provider.service_provider_label = "Qwen Platform".to_string();

        let mut coding_anthropic_credential = build_test_credential(
            "cred-qwen-coding-anthropic",
            "folder_sync",
            None,
            "active",
            None,
        );
        coding_anthropic_credential.payload =
            serde_json::json!({ "apiKey": "sk-test-coding-anthropic" });
        assert_eq!(
            default_folder_sync_relative_path(
                &coding_anthropic_provider,
                &coding_anthropic_credential
            ),
            "qwen-platform/qwen-coding-plan-anthropic/api-key/cred-qwen-coding-anthropic.json"
        );

        let mut web_provider = build_test_provider_account(
            "Qwen WebUI Replay Live",
            "qwen_web_compatible",
            "qwen_web_chat",
            "qwen_web_chat",
            "https://chat.qwen.ai",
            Some("web_reverse_api"),
            Some("direct_http_replay"),
        );
        web_provider.service_provider_key = "qwen_platform".to_string();
        web_provider.service_provider_label = "Qwen Platform".to_string();

        let mut web_credential =
            build_test_credential("cred-qwen-web", "folder_sync", None, "active", None);
        web_credential.payload = serde_json::json!({ "apiKey": "qwen-web-token" });
        assert_eq!(
            default_folder_sync_relative_path(&web_provider, &web_credential),
            "qwen-platform/qwen-web-chat/session-auth/cred-qwen-web.json"
        );
    }

    #[test]
    fn normalize_gemini_canvas_import_payload_converts_manual_export_output() {
        let mut provider = build_test_provider_account(
            "Gemini Canvas Images",
            "gemini_canvas_compatible",
            "gemini_canvas_images",
            "gemini_canvas",
            "https://gemini.google.com",
            Some("web_reverse_api"),
            Some("browser_challenge"),
        );
        provider.service_provider_key = "gemini_platform".to_string();
        provider.service_provider_label = "Gemini Platform".to_string();

        let payload = normalize_import_payload(
            "gemini-canvas-images",
            &provider,
            serde_json::json!({
                "runtimeStateObjectKey": "credential-runtime/gemini-canvas/demo/storage-state.json",
                "suggestedShareId": "share-123",
                "accountName": "Gemini Canvas Main",
                "baseUrl": "https://gemini.google.com",
                "credentialMaterialKey": "gemini-canvas-browser-main",
                "googleApiKey": "AIzaPrimaryKey123",
                "apiKeys": ["AIzaPrimaryKey123", "AIzaSecondaryKey456"]
            }),
            Some("browser_state"),
        )
        .expect("normalized gemini canvas payload");

        assert_eq!(
            payload.get("runtimeStateObjectKey").and_then(Value::as_str),
            Some("credential-runtime/gemini-canvas/demo/storage-state.json")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("shareId"))
                .and_then(Value::as_str),
            Some("share-123")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("googleApiKey"))
                .and_then(Value::as_str),
            Some("AIzaPrimaryKey123")
        );
        assert_eq!(
            payload
                .get("extraBody")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("apiKeys"))
                .and_then(Value::as_array)
                .map(|values| { values.iter().filter_map(Value::as_str).collect::<Vec<_>>() }),
            Some(vec!["AIzaPrimaryKey123", "AIzaSecondaryKey456"])
        );
        assert_eq!(
            payload
                .get("credentialMaterialKind")
                .and_then(Value::as_str),
            Some("browser_state")
        );
        assert_eq!(
            payload.get("credentialMaterialKey").and_then(Value::as_str),
            Some("gemini-canvas-browser-main")
        );
        assert_eq!(
            payload.get("providerSurfaceKey").and_then(Value::as_str),
            Some("gemini-canvas-images")
        );
        assert_eq!(
            payload.get("serviceProviderKey").and_then(Value::as_str),
            Some("gemini_platform")
        );
    }

    #[test]
    fn normalize_gemini_canvas_import_payload_preserves_program_handle_provenance() {
        let mut provider = build_test_provider_account(
            "Gemini Canvas Program Relay",
            "gemini_canvas_program_web_reverse_compatible",
            "gemini_canvas_images",
            "gemini_canvas_program_web_reverse_modular",
            "https://gemini.google.com",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        provider.service_provider_key = "gemini_platform".to_string();
        provider.service_provider_label = "Gemini Platform".to_string();

        let payload = normalize_import_payload(
            "gemini-canvas-program-relay",
            &provider,
            serde_json::json!({
                "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
                "shareUrl": "https://gemini.google.com/share/fe24c455a570",
                "shareId": "fe24c455a570",
                "canvasProgramUrl": "https://gemini.google.com/app/4abc4e7577b6149f",
                "beforeUrl": "https://gemini.google.com/canvas",
                "finalUrl": "https://gemini.google.com/app",
                "shareFollowKind": "same_page",
                "pageUrl": "https://gemini.google.com/app",
                "appPath": "/app/4abc4e7577b6149f",
                "programId": "4abc4e7577b6149f",
                "conversationId": "c_4abc4e7577b6149f",
                "responseId": "r_1d583cfb0fa7aee4",
                "invokeBaseUrl": "https://canvas-endpoint.example",
                "musicWsUrl": "wss://canvas-endpoint.example/ws/music",
                "videoInvokePath": "/v1beta/models/gemini-video:predictLongRunning",
                "canvasProgramAction": "music_generation",
                "canvasProgramActionInput": "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}",
                "canvasProgramInvokeContract": {
                    "operation": "music",
                    "transportKind": "official_music_ws_candidate",
                    "target": "wss://canvas-endpoint.example/ws/music",
                    "actionName": "music_generation",
                    "actionInput": "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}",
                    "prompt": "A short electronic cue.",
                    "durationSeconds": 30,
                    "uiState": "player_ready"
                },
                "googleApiKey": "AIzaProgramOwnedShouldDrop",
                "apiKeys": ["AIzaProgramOwnedShouldDrop"],
                "lastSeenConversationId": "c_4b6fc89f965c0d1e",
                "lastSeenResponseId": "r_7fa26ee4c86a2f0a",
                "candidatePairs": [
                    {
                        "appPath": "/app/4abc4e7577b6149f",
                        "conversationId": "c_4abc4e7577b6149f",
                        "responseId": "r_1d583cfb0fa7aee4"
                    }
                ],
                "aggregateHints": {
                    "appPaths": ["/app/4abc4e7577b6149f"],
                    "conversationIds": ["c_4abc4e7577b6149f"],
                    "responseIds": ["r_1d583cfb0fa7aee4"]
                },
                "newChatClicked": true,
                "modeSelected": true,
                "capturedAt": "2026-05-05T02:40:36.361Z",
                "lastValidatedAt": "2026-05-05T02:40:36.361Z"
            }),
            Some("browser_state"),
        )
        .expect("normalized program relay payload");

        let extra = payload
            .get("extraBody")
            .and_then(Value::as_object)
            .expect("extraBody");
        assert_eq!(
            extra.get("canvasProgramUrl").and_then(Value::as_str),
            Some("https://gemini.google.com/app/4abc4e7577b6149f")
        );
        assert_eq!(
            extra.get("programId").and_then(Value::as_str),
            Some("4abc4e7577b6149f")
        );
        assert_eq!(
            extra.get("lastSeenResponseId").and_then(Value::as_str),
            Some("r_7fa26ee4c86a2f0a")
        );
        assert_eq!(
            extra.get("invokeBaseUrl").and_then(Value::as_str),
            Some("https://canvas-endpoint.example")
        );
        assert_eq!(
            extra.get("musicWsUrl").and_then(Value::as_str),
            Some("wss://canvas-endpoint.example/ws/music")
        );
        assert_eq!(
            extra.get("videoInvokePath").and_then(Value::as_str),
            Some("/v1beta/models/gemini-video:predictLongRunning")
        );
        assert_eq!(
            extra.get("canvasProgramAction").and_then(Value::as_str),
            Some("music_generation")
        );
        assert_eq!(
            extra
                .get("canvasProgramActionInput")
                .and_then(Value::as_str),
            Some("{'prompt': 'A short electronic cue.', 'duration_seconds': 30}")
        );
        assert_eq!(
            extra
                .get("canvasProgramInvokeContract")
                .and_then(Value::as_object)
                .and_then(|value| value.get("transportKind"))
                .and_then(Value::as_str),
            Some("official_music_ws_candidate")
        );
        assert!(!extra.contains_key("googleApiKey"));
        assert!(!extra.contains_key("apiKeys"));
        assert_eq!(payload.get("apiKey").and_then(Value::as_str), Some(""));
        assert_eq!(
            extra
                .get("candidatePairs")
                .and_then(Value::as_array)
                .map(|value| value.len()),
            Some(1)
        );
        assert_eq!(
            extra
                .get("aggregateHints")
                .and_then(Value::as_object)
                .and_then(|value| value.get("appPaths"))
                .and_then(Value::as_array)
                .map(|value| value.len()),
            Some(1)
        );
        assert_eq!(
            payload.get("providerSurfaceKey").and_then(Value::as_str),
            Some("gemini-canvas-program-relay")
        );
    }

    #[test]
    fn normalize_gemini_canvas_import_payload_allows_source_credential_without_share_override() {
        let mut provider = build_test_provider_account(
            "Gemini Canvas Program Relay",
            "gemini_canvas_program_web_reverse_compatible",
            "gemini_canvas_images",
            "gemini_canvas_program_web_reverse_modular",
            "https://gemini.google.com",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        provider.service_provider_key = "gemini_platform".to_string();
        provider.service_provider_label = "Gemini Platform".to_string();

        let payload = normalize_import_payload(
            "gemini-canvas-program-relay",
            &provider,
            serde_json::json!({
                "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
                "cookieHeader": "__Secure-1PSID=psid-main; __Secure-1PSIDTS=psidts-main",
                "credentialMaterialKey": "google-session-main-001"
            }),
            Some("browser_state"),
        )
        .expect("normalized source credential without share override");

        assert_eq!(
            payload.get("runtimeStateObjectKey").and_then(Value::as_str),
            Some("credential-runtime/gemini-canvas/program/storage-state.json")
        );
        let extra = payload
            .get("extraBody")
            .and_then(Value::as_object)
            .expect("extraBody");
        assert!(!extra.contains_key("shareId"));
        assert!(!extra.contains_key("shareUrl"));
        assert_eq!(
            payload
                .get("credentialMaterialKind")
                .and_then(Value::as_str),
            Some("browser_state")
        );
    }

    #[test]
    fn normalize_gemini_canvas_import_payload_derives_share_id_from_share_url() {
        let mut provider = build_test_provider_account(
            "Gemini Canvas Program Relay",
            "gemini_canvas_program_web_reverse_compatible",
            "gemini_canvas_images",
            "gemini_canvas_program_web_reverse_modular",
            "https://gemini.google.com",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        provider.service_provider_key = "gemini_platform".to_string();
        provider.service_provider_label = "Gemini Platform".to_string();

        let payload = normalize_import_payload(
            "gemini-canvas-program-relay",
            &provider,
            serde_json::json!({
                "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
                "shareUrl": "https://gemini.google.com/share/derived-share-456"
            }),
            Some("browser_state"),
        )
        .expect("normalized shareUrl override");

        let extra = payload
            .get("extraBody")
            .and_then(Value::as_object)
            .expect("extraBody");
        assert_eq!(
            extra.get("shareId").and_then(Value::as_str),
            Some("derived-share-456")
        );
        assert_eq!(
            extra.get("shareUrl").and_then(Value::as_str),
            Some("https://gemini.google.com/share/derived-share-456")
        );
    }

    #[test]
    fn normalize_gemini_canvas_import_payload_unwraps_recursive_raw_source_chain() {
        let mut provider = build_test_provider_account(
            "Gemini Canvas Program Relay",
            "gemini_canvas_program_web_reverse_compatible",
            "gemini_canvas_images",
            "gemini_canvas_program_web_reverse_modular",
            "https://gemini.google.com",
            Some("web_reverse_api"),
            Some("browser_backed"),
        );
        provider.service_provider_key = "gemini_platform".to_string();
        provider.service_provider_label = "Gemini Platform".to_string();

        let payload = normalize_import_payload(
            "gemini-canvas-program-relay",
            &provider,
            serde_json::json!({
                "apiKey": "",
                "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
                "credentialMaterialKind": "browser_state",
                "providerSurfaceKey": "gemini-canvas-program-relay",
                "extraBody": {
                    "shareId": "fe24c455a570"
                },
                "rawSource": {
                    "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
                    "shareId": "fe24c455a570",
                    "canvasProgramHint": "matrix-live-bootstrap"
                }
            }),
            Some("browser_state"),
        )
        .expect("normalized recursive rawSource payload");

        let raw_source = payload
            .get("rawSource")
            .and_then(Value::as_object)
            .expect("rawSource object");
        assert!(!raw_source.contains_key("rawSource"));
        assert_eq!(
            raw_source
                .get("runtimeStateObjectKey")
                .and_then(Value::as_str),
            Some("credential-runtime/gemini-canvas/program/storage-state.json")
        );
        assert_eq!(
            raw_source.get("shareId").and_then(Value::as_str),
            Some("fe24c455a570")
        );
        assert_eq!(
            raw_source.get("canvasProgramHint").and_then(Value::as_str),
            Some("matrix-live-bootstrap")
        );
    }

    #[test]
    fn deleted_relative_paths_extract_removed_json_file() {
        let root = std::env::temp_dir().join("neuro-folder-sync-test-root");
        let deleted_path = root
            .join("codex")
            .join("codex-30c5b2fd-mo5e917c@sall.cc-team.json");
        let event =
            Event::new(EventKind::Remove(notify::event::RemoveKind::File)).add_path(deleted_path);

        assert_eq!(
            deleted_relative_paths_from_event(root.as_path(), &event),
            HashSet::from(["codex/codex-30c5b2fd-mo5e917c@sall.cc-team.json".to_string()])
        );
    }

    #[test]
    fn missing_folder_file_deletes_only_active_folder_synced_credentials() {
        let observed_paths = HashSet::from(["codex/keep.json".to_string()]);
        let synced_missing = build_test_credential(
            "cred-delete",
            "folder_sync",
            Some("codex/remove.json"),
            "active",
            None,
        );
        let synced_present = build_test_credential(
            "cred-keep",
            "folder_sync",
            Some("codex/keep.json"),
            "active",
            None,
        );
        let manual_missing = build_test_credential(
            "cred-manual",
            "manual",
            Some("codex/remove.json"),
            "active",
            None,
        );
        let archived_missing = build_test_credential(
            "cred-archived",
            "folder_sync",
            Some("codex/remove.json"),
            "archived",
            None,
        );
        let mut pending_export_missing = build_test_credential(
            "cred-pending-export",
            "folder_sync",
            Some("codex/pending.json"),
            "active",
            None,
        );
        pending_export_missing.source_kind = "manual".to_string();
        pending_export_missing.sync_state = "idle".to_string();

        assert!(should_delete_missing_folder_credential(
            &synced_missing,
            &observed_paths
        ));
        assert!(!should_delete_missing_folder_credential(
            &synced_present,
            &observed_paths
        ));
        assert!(!should_delete_missing_folder_credential(
            &manual_missing,
            &observed_paths
        ));
        assert!(!should_delete_missing_folder_credential(
            &archived_missing,
            &observed_paths
        ));
        assert!(!should_delete_missing_folder_credential(
            &pending_export_missing,
            &observed_paths
        ));
    }

    #[test]
    fn explicit_delete_event_removes_only_materialized_folder_credentials() {
        let observed_paths = HashSet::new();
        let explicit_deleted_paths = HashSet::from(["codex/remove.json".to_string()]);
        let mut imported_missing = build_test_credential(
            "cred-imported-delete",
            "folder_sync",
            Some("codex/remove.json"),
            "active",
            None,
        );
        imported_missing.source_kind = "folder_sync_import".to_string();
        imported_missing.sync_state = "imported".to_string();

        let mut pending_export_missing = build_test_credential(
            "cred-pending-export",
            "folder_sync",
            Some("codex/remove.json"),
            "active",
            None,
        );
        pending_export_missing.source_kind = "manual".to_string();
        pending_export_missing.sync_state = "idle".to_string();

        let recreated_path = build_test_credential(
            "cred-recreated",
            "folder_sync",
            Some("codex/remove.json"),
            "active",
            None,
        );

        let different_path = build_test_credential(
            "cred-keep",
            "folder_sync",
            Some("codex/keep.json"),
            "active",
            None,
        );

        assert!(should_delete_explicitly_removed_folder_credential(
            &imported_missing,
            &observed_paths,
            &explicit_deleted_paths
        ));
        assert!(!should_delete_explicitly_removed_folder_credential(
            &pending_export_missing,
            &observed_paths,
            &explicit_deleted_paths
        ));
        assert!(!should_delete_explicitly_removed_folder_credential(
            &recreated_path,
            &HashSet::from(["codex/remove.json".to_string()]),
            &explicit_deleted_paths
        ));
        assert!(!should_delete_explicitly_removed_folder_credential(
            &different_path,
            &observed_paths,
            &explicit_deleted_paths
        ));
    }

    #[test]
    fn explicit_delete_summary_updates_only_when_new_hits_exist() {
        let mut status = ProviderCredentialFolderSyncStatusView {
            last_explicit_delete_at: Some("2026-04-18T00:00:00Z".to_string()),
            last_explicit_delete_count: 1,
            last_explicit_delete_paths: vec!["codex/older.json".to_string()],
            recent_explicit_delete_events: vec![FolderSyncExplicitDeleteEventView {
                event_id: "event-old".to_string(),
                occurred_at: "2026-04-18T00:00:00Z".to_string(),
                deleted_count: 1,
                deleted_paths: vec!["codex/older.json".to_string()],
                provider_credential_ids: vec!["cred-old".to_string()],
            }],
            ..ProviderCredentialFolderSyncStatusView::default()
        };
        let new_events = vec![
            FolderSyncExplicitDeleteEventView {
                event_id: "event-new-1".to_string(),
                occurred_at: "2026-04-19T00:00:00Z".to_string(),
                deleted_count: 1,
                deleted_paths: vec!["codex/remove.json".to_string()],
                provider_credential_ids: vec!["cred-remove".to_string()],
            },
            FolderSyncExplicitDeleteEventView {
                event_id: "event-new-2".to_string(),
                occurred_at: "2026-04-19T00:02:00Z".to_string(),
                deleted_count: 2,
                deleted_paths: vec![
                    "codex/remove.json".to_string(),
                    "codex/second.json".to_string(),
                ],
                provider_credential_ids: vec!["cred-remove".to_string(), "cred-second".to_string()],
            },
        ];

        append_explicit_delete_events(&mut status, &new_events);

        assert_eq!(
            status.last_explicit_delete_at.as_deref(),
            Some("2026-04-19T00:02:00Z")
        );
        assert_eq!(status.last_explicit_delete_count, 2);
        assert_eq!(
            status.last_explicit_delete_paths,
            vec![
                "codex/remove.json".to_string(),
                "codex/second.json".to_string()
            ]
        );
        assert_eq!(
            status
                .recent_explicit_delete_events
                .iter()
                .map(|entry| entry.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["event-new-2", "event-new-1", "event-old"]
        );

        let preserved_status = status.clone();
        append_explicit_delete_events(&mut status, &[]);

        assert_eq!(
            status.last_explicit_delete_at,
            preserved_status.last_explicit_delete_at
        );
        assert_eq!(
            status.last_explicit_delete_count,
            preserved_status.last_explicit_delete_count
        );
        assert_eq!(
            status.last_explicit_delete_paths,
            preserved_status.last_explicit_delete_paths
        );
        assert_eq!(
            status.recent_explicit_delete_events,
            preserved_status.recent_explicit_delete_events
        );
    }

    #[test]
    fn build_explicit_delete_event_deduplicates_paths_and_ids() {
        let event = build_explicit_delete_event(&[
            FolderSyncExplicitDeleteHit {
                provider_credential_id: "cred-1".to_string(),
                source_path: "codex/remove.json".to_string(),
            },
            FolderSyncExplicitDeleteHit {
                provider_credential_id: "cred-1".to_string(),
                source_path: "codex/remove.json".to_string(),
            },
            FolderSyncExplicitDeleteHit {
                provider_credential_id: "cred-2".to_string(),
                source_path: "codex/second.json".to_string(),
            },
        ]);

        assert!(!event.event_id.is_empty());
        assert_eq!(event.deleted_count, 2);
        assert_eq!(
            event.deleted_paths,
            vec![
                "codex/remove.json".to_string(),
                "codex/second.json".to_string()
            ]
        );
        assert_eq!(
            event.provider_credential_ids,
            vec!["cred-1".to_string(), "cred-2".to_string()]
        );
    }

    #[test]
    fn materialized_folder_copy_requires_import_or_export_state() {
        let mut imported = build_test_credential(
            "cred-imported",
            "folder_sync",
            Some("codex/imported.json"),
            "active",
            None,
        );
        imported.source_kind = "folder_sync_import".to_string();
        imported.sync_state = "imported".to_string();

        let mut exported = build_test_credential(
            "cred-exported",
            "folder_sync",
            Some("codex/exported.json"),
            "active",
            None,
        );
        exported.source_kind = "manual".to_string();
        exported.sync_state = "exported".to_string();

        let mut pending = build_test_credential(
            "cred-pending",
            "folder_sync",
            Some("codex/pending.json"),
            "active",
            None,
        );
        pending.source_kind = "manual".to_string();
        pending.sync_state = "idle".to_string();

        assert!(has_materialized_folder_copy(&imported));
        assert!(has_materialized_folder_copy(&exported));
        assert!(!has_materialized_folder_copy(&pending));
    }
}
