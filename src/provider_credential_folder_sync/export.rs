//! Export live database credentials and reconcile stale materialized files.
use super::filesystem::{delete_stale_files, serialize_export_payload, write_export_file};
use super::layout::default_folder_sync_relative_path;
use super::limits::MATERIAL_BYTES;
use super::payload_metadata::materialize_export_payload;
use super::FolderSyncCounters;
use crate::db;
use crate::error::GatewayError;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(super) async fn export_database_credentials(
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
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| default_folder_sync_relative_path(provider_account, &credential));
        let export_payload = materialize_export_payload(provider_account, &credential.payload);
        let serialized = serialize_export_payload(&export_payload, MATERIAL_BYTES)?;
        let (relative, hash, written) = write_export_file(root_dir, &relative, &serialized)?;
        expected_paths.insert(relative.clone());
        if written {
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
        delete_stale_files(root_dir, &expected_paths, &mut counters.deleted_count)?;
    }

    Ok(())
}
