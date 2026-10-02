//! Remote archive I/O is verified before route removal; failures preserve source credentials.
use super::{
    archive_record::{owned_record, revision_object_key, ArchiveRecord},
    RemoteStorage, StorageNotFound, NETWORK_DEADLINE,
};
use crate::{config::Config, routing::config::ProviderConfigYaml};
use std::collections::HashSet;

pub(crate) async fn count(config: &Config, provider: &ProviderConfigYaml) -> anyhow::Result<usize> {
    validate_location(provider)?;
    let Some(connection) = provider
        .credential_archive_connection
        .as_ref()
        .filter(|c| c.is_remote())
    else {
        return crate::credential_pool_automation::archive::checked_local_archive_count(
            config, provider,
        );
    };
    let store = RemoteStorage::new(connection, &provider.id, true)?;
    count_records(&store, &provider.id).await
}

pub(super) async fn count_records(
    store: &RemoteStorage,
    provider_id: &str,
) -> anyhow::Result<usize> {
    bounded("Archive count timed out", async {
        let mut count = 0;
        for key in store.list().await? {
            let bytes = store.get(&key).await?;
            if owned_record(&bytes, &key, provider_id).is_some() {
                count += 1;
            }
        }
        Ok(count)
    })
    .await
}

pub(crate) async fn preserve(
    config: &Config,
    provider: &ProviderConfigYaml,
    ids: &HashSet<String>,
    source_revision: &str,
) -> anyhow::Result<usize> {
    if ids.is_empty() || provider.credential_permanent_delete_enabled {
        return Ok(0);
    }
    validate_location(provider)?;
    let Some(connection) = provider
        .credential_archive_connection
        .as_ref()
        .filter(|c| c.is_remote())
    else {
        return crate::credential_pool_automation::archive::archive_pruned_credentials(
            config, provider, ids,
        );
    };
    let store = RemoteStorage::new(connection, &provider.id, true)?;
    preserve_records(&store, provider, ids, source_revision).await
}

pub(super) async fn preserve_records(
    store: &RemoteStorage,
    provider: &ProviderConfigYaml,
    ids: &HashSet<String>,
    source_revision: &str,
) -> anyhow::Result<usize> {
    bounded("Archive write timed out; source credentials were retained", async {
        let mut archived = 0;
        for (index, credential) in provider.credentials.iter().enumerate() {
            let id = credential.id.clone().unwrap_or_else(|| format!("{}-cred-{index}", provider.id));
            if !ids.contains(&id) { continue; }
            let key = revision_object_key(&id, credential, source_revision)?;
            match store.get(&key).await {
                Ok(bytes) => {
                    if owned_record(&bytes, &key, &provider.id).is_none() { anyhow::bail!("Existing archive object failed ownership verification"); }
                }
                Err(error) if error.is::<StorageNotFound>() => {
                    let record = ArchiveRecord {
                        schema_version: 2, source_revision: Some(source_revision.to_owned()), provider_id: provider.id.clone(), credential_id: id,
                        archived_at: time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?,
                        reason: "permanent_driver_rejection".into(), credential: credential.clone(),
                    };
                    let bytes = serde_json::to_vec_pretty(&record)?;
                    // A timeout can mean PUT succeeded. Read back the same deterministic key
                    // in either case; never remove source on an unverified write.
                    let write_result = store.put(&key, &bytes).await;
                    match store.get(&key).await {
                        Ok(actual) if owned_record(&actual, &key, &provider.id).is_some() => {},
                        _ => {
                            drop(write_result);
                            anyhow::bail!("Archive write could not be verified; source credentials were retained");
                        }
                    }
                }
                Err(error) => return Err(error),
            }
            archived += 1;
        }
        Ok(archived)
    }).await
}

async fn bounded<F: std::future::Future<Output = anyhow::Result<usize>>>(
    message: &str,
    future: F,
) -> anyhow::Result<usize> {
    tokio::time::timeout(NETWORK_DEADLINE, future)
        .await
        .map_err(|_| anyhow::anyhow!(message.to_owned()))?
}

fn validate_location(provider: &ProviderConfigYaml) -> anyhow::Result<()> {
    if let Some(connection) = &provider.credential_archive_connection {
        super::validate_connection(connection).map_err(anyhow::Error::msg)?;
    }
    Ok(())
}
