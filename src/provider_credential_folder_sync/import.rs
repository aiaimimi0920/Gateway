//! Import folder material and retain database/deletion ordering.
use super::account_cache::HydratedAccountCache;
use super::accounts::{build_provider_maps, select_provider_account_for_path};
use super::credential_paths::{CredentialPathIndex, SourcePathInterner, SourcePathSet};
use super::deletion::{
    delete_explicitly_removed_folder_credentials, delete_missing_folder_credentials,
};
use super::filesystem::{collect_json_files, read_provider_credential_json_with_retry};
use super::layout::{
    credential_label_from_path, normalize_relative_path, resolve_import_path_descriptor,
};
use super::limits::{
    PROVIDER_ACCOUNT_INLINE_PAYLOAD_BYTES, PROVIDER_ACCOUNT_ROWS,
    PROVIDER_CREDENTIAL_METADATA_ROWS, SOURCE_PATH_RETAINED_BYTES,
};
use super::normalization::normalize_import_payload;
use super::FolderSyncCounters;
use crate::db;
use crate::error::GatewayError;
use deadpool_redis::Pool;
use std::collections::HashSet;
use std::path::Path;

#[cfg(test)]
mod tests;

pub(super) async fn import_folder_credentials(
    pg_pool: &sqlx::PgPool,
    redis_pool: &Pool,
    root_dir: &Path,
    delete_missing: bool,
    explicit_deleted_paths: &HashSet<String>,
    counters: &mut FolderSyncCounters,
) -> Result<(), GatewayError> {
    let provider_accounts =
        db::list_provider_account_import_metadata(pg_pool, PROVIDER_ACCOUNT_ROWS).await?;
    let provider_maps = build_provider_maps(&provider_accounts);
    let existing_credentials = db::list_provider_credential_import_metadata(
        pg_pool,
        PROVIDER_CREDENTIAL_METADATA_ROWS,
        SOURCE_PATH_RETAINED_BYTES,
    )
    .await?;
    let mut hydrated_accounts = HydratedAccountCache::new();
    let mut path_interner = SourcePathInterner::new();
    let credential_paths = CredentialPathIndex::new(existing_credentials, &mut path_interner)?;
    let files = collect_json_files(root_dir)?;
    let mut observed_paths = SourcePathSet::new();

    for file_path in files {
        let relative =
            path_interner.intern_normalized(normalize_relative_path(root_dir, &file_path)?)?;
        observed_paths.insert(relative.clone());

        let (hash, raw_payload) =
            read_provider_credential_json_with_retry(root_dir, &file_path).await?;
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
        let existing = credential_paths.get(&relative);
        if existing
            .is_some_and(|credential| credential.source_hash.as_deref() == Some(hash.as_str()))
        {
            counters.skipped_count += 1;
            continue;
        }
        let provider_account =
            if let Some(provider_account) = hydrated_accounts.get(provider_account_id.as_str()) {
                provider_account
            } else {
                let provider_account = db::get_provider_account_for_folder_sync(
                    pg_pool,
                    &provider_account_id,
                    PROVIDER_ACCOUNT_INLINE_PAYLOAD_BYTES,
                )
                .await?
                .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
                hydrated_accounts.insert(provider_account)
            };
        let payload = normalize_import_payload(
            &path_descriptor.provider_surface_slug,
            &provider_account,
            raw_payload,
            path_descriptor.credential_material_kind.as_deref(),
        )?;

        match existing {
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
                        source_path: Some(relative.to_string()),
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
                        source_path: Some(relative.to_string()),
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
        &credential_paths,
        &observed_paths,
        explicit_deleted_paths,
        counters,
    )
    .await?;

    if delete_missing {
        delete_missing_folder_credentials(
            pg_pool,
            redis_pool,
            &credential_paths,
            &observed_paths,
            explicit_deleted_paths,
            counters,
        )
        .await?;
    }

    Ok(())
}
