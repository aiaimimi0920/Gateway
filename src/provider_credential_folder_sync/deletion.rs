use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

use deadpool_redis::Pool;
use notify::Event;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db;
use crate::error::GatewayError;
use crate::provider_runtime;
use crate::state::AppState;

use super::credential_paths::{CredentialPathIndex, IndexedCredential, SourcePathSet};
use super::path_budget::BoundedPathSet;
use super::status::update_latest_timestamp;
use super::{
    folder_sync_path_should_trigger, format_timestamp, normalize_source_path_key, resolve_root_dir,
    FolderSyncCounters, FolderSyncExplicitDeleteEventView, ProviderCredentialFolderSyncStatusView,
};

use super::paths;

#[derive(Debug, Clone)]
struct FolderSyncExplicitDeleteHit {
    provider_credential_id: String,
    source_path: String,
}

pub fn delete_synced_credential_file(
    state: &AppState,
    relative_path: &str,
) -> Result<bool, GatewayError> {
    if !state.provider_credential_folder_sync.enabled() {
        return Ok(false);
    }
    let Some(relative_path) = paths::validated_relative_path(relative_path)? else {
        return Ok(false);
    };
    let root_dir = resolve_root_dir(&state.config)?;
    paths::delete_file(&root_dir, &relative_path)
}

pub(super) async fn delete_explicitly_removed_folder_credentials(
    pg_pool: &sqlx::PgPool,
    redis_pool: &Pool,
    existing_credentials: &CredentialPathIndex,
    observed_paths: &SourcePathSet,
    explicit_deleted_paths: &HashSet<String>,
    counters: &mut FolderSyncCounters,
) -> Result<(), GatewayError> {
    let mut deleted_hits = Vec::new();
    for indexed in existing_credentials.iter().filter(|credential| {
        should_delete_explicitly_removed_folder_credential_indexed(
            credential,
            observed_paths,
            explicit_deleted_paths,
        )
    }) {
        let credential = &indexed.credential;
        if let Some(source_path) = &indexed.source_path {
            deleted_hits.push(FolderSyncExplicitDeleteHit {
                provider_credential_id: credential.id.clone(),
                source_path: source_path.to_string(),
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

pub(super) async fn delete_missing_folder_credentials(
    pg_pool: &sqlx::PgPool,
    redis_pool: &Pool,
    existing_credentials: &CredentialPathIndex,
    observed_paths: &SourcePathSet,
    explicit_deleted_paths: &HashSet<String>,
    counters: &mut FolderSyncCounters,
) -> Result<(), GatewayError> {
    for indexed in existing_credentials.iter().filter(|credential| {
        should_delete_missing_folder_credential_indexed(
            credential,
            observed_paths,
            explicit_deleted_paths,
        )
    }) {
        let credential = &indexed.credential;
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

pub(super) fn deleted_relative_paths_from_event(root_dir: &Path, event: &Event) -> BoundedPathSet {
    let mut deleted_paths = BoundedPathSet::default();
    for path in event
        .paths
        .iter()
        .filter(|path| folder_sync_path_should_trigger(path.as_path()))
        .filter(|path| !path.exists())
        .filter_map(|path| normalize_deleted_relative_path(root_dir, path.as_path()))
    {
        deleted_paths.insert(path);
        if deleted_paths.overflowed() {
            break;
        }
    }
    deleted_paths
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
        if !deleted_paths.iter().any(|path| path == &hit.source_path) {
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
    let Some(latest_event) = events
        .iter()
        .min_by(|left, right| compare_delete_events(left, right))
    else {
        return;
    };
    let mut new_events = events.to_vec();
    new_events.extend(status.recent_explicit_delete_events.iter().cloned());
    new_events.sort_by(compare_delete_events);
    new_events.truncate(RECENT_EXPLICIT_DELETE_EVENT_LIMIT);
    status.recent_explicit_delete_events = new_events;
    if update_latest_timestamp(
        &mut status.last_explicit_delete_at,
        latest_event.occurred_at.clone(),
    ) {
        status.last_explicit_delete_count = latest_event.deleted_count;
        status.last_explicit_delete_paths = latest_event.deleted_paths.clone();
    }
}

fn compare_delete_events(
    left: &FolderSyncExplicitDeleteEventView,
    right: &FolderSyncExplicitDeleteEventView,
) -> Ordering {
    match (
        OffsetDateTime::parse(
            &left.occurred_at,
            &time::format_description::well_known::Rfc3339,
        ),
        OffsetDateTime::parse(
            &right.occurred_at,
            &time::format_description::well_known::Rfc3339,
        ),
    ) {
        (Ok(left), Ok(right)) => right.cmp(&left),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => Ordering::Equal,
    }
}

pub(super) fn apply_explicit_delete_summary(
    status: &mut ProviderCredentialFolderSyncStatusView,
    counters: &FolderSyncCounters,
) {
    append_explicit_delete_events(status, &counters.explicit_delete_events);
}

fn should_delete_missing_folder_credential_indexed(
    indexed: &IndexedCredential,
    observed_paths: &SourcePathSet,
    explicit_deleted_paths: &HashSet<String>,
) -> bool {
    let credential = &indexed.credential;
    credential.archived_at.is_none()
        && credential.status != "archived"
        && credential.sync_mode == "folder_sync"
        && has_materialized_folder_copy(credential)
        && indexed.source_path.as_ref().is_some_and(|source_path| {
            observed_paths.get(source_path.as_ref()).is_none()
                && !source_path_matches_explicit_delete(
                    source_path.as_ref(),
                    explicit_deleted_paths,
                )
        })
}

fn should_delete_explicitly_removed_folder_credential_indexed(
    indexed: &IndexedCredential,
    observed_paths: &SourcePathSet,
    explicit_deleted_paths: &HashSet<String>,
) -> bool {
    let credential = &indexed.credential;
    credential.archived_at.is_none()
        && credential.status != "archived"
        && credential.sync_mode == "folder_sync"
        && has_materialized_folder_copy(credential)
        && indexed.source_path.as_ref().is_some_and(|source_path| {
            source_path_matches_explicit_delete(source_path.as_ref(), explicit_deleted_paths)
                && observed_paths.get(source_path.as_ref()).is_none()
        })
}

#[cfg(test)]
fn should_delete_missing_folder_credential(
    credential: &db::ProviderCredentialImportMetadata,
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
                !observed_paths.contains(&source_path)
                    && !source_path_matches_explicit_delete(&source_path, explicit_deleted_paths)
            })
}

#[cfg(test)]
fn should_delete_explicitly_removed_folder_credential(
    credential: &db::ProviderCredentialImportMetadata,
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

fn has_materialized_folder_copy(credential: &crate::db::ProviderCredentialImportMetadata) -> bool {
    credential.source_kind == "folder_sync_import"
        || matches!(credential.sync_state.as_str(), "imported" | "exported")
}

#[cfg(test)]
mod tests;
