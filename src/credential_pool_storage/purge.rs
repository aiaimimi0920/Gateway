//! Frozen archive candidates. Only the coordinator may execute after durable CAS success.
use super::{archive_purge_support, archive_record::owned_record, RemoteStorage, NETWORK_DEADLINE};
use crate::credential_pool_automation::{
    archive_snapshot::LocalArchiveSnapshot, storage_paths::provider_archive_path,
};
use crate::{config::Config, routing::config::ProviderConfigYaml};
use sha2::{Digest, Sha256};

struct FrozenObject {
    key: String,
    digest: [u8; 32],
    etag: String,
}
pub(crate) enum ArchivePurgePlan {
    Local(LocalArchiveSnapshot),
    Remote(RemotePurgePlan),
}
pub(crate) struct RemotePurgePlan {
    store: RemoteStorage,
    records: Vec<FrozenObject>,
}
impl ArchivePurgePlan {
    pub(crate) async fn capture(
        config: &Config,
        provider: &ProviderConfigYaml,
    ) -> anyhow::Result<Self> {
        if let Some(connection) = &provider.credential_archive_connection {
            super::validate_connection(connection).map_err(anyhow::Error::msg)?;
        }
        if let Some(path) = &provider.credential_archive_path {
            crate::credential_pool_automation::storage_paths::validate_storage_path(path)
                .map_err(anyhow::Error::msg)?;
        }
        let (supported, reason) =
            archive_purge_support(provider.credential_archive_connection.as_ref());
        if !supported {
            anyhow::bail!(reason.unwrap_or("Archive purge capability is unavailable"));
        }
        if let Some(connection) = provider
            .credential_archive_connection
            .as_ref()
            .filter(|c| c.is_remote())
        {
            let store = RemoteStorage::new(connection, &provider.id, true)?;
            return Ok(Self::Remote(
                RemotePurgePlan::capture(store, &provider.id).await?,
            ));
        }
        let directory = provider_archive_path(config, provider)
            .ok_or_else(|| anyhow::anyhow!("Archive path is unavailable"))?;
        Ok(Self::Local(LocalArchiveSnapshot::capture(
            &directory,
            &provider.id,
        )?))
    }
    #[cfg(test)]
    pub(crate) async fn capture_fixture(provider: &ProviderConfigYaml) -> anyhow::Result<Self> {
        let mut store = RemoteStorage::new(
            provider.credential_archive_connection.as_ref().unwrap(),
            &provider.id,
            true,
        )?;
        store.allow_fixture_capabilities();
        Ok(Self::Remote(
            RemotePurgePlan::capture(store, &provider.id).await?,
        ))
    }
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Local(plan) => plan.is_empty(),
            Self::Remote(plan) => plan.records.is_empty(),
        }
    }
    pub(crate) async fn execute(
        self,
        current_revision: &(dyn Fn() -> bool + Send + Sync),
    ) -> anyhow::Result<usize> {
        match self {
            Self::Local(plan) => plan.execute(current_revision),
            Self::Remote(plan) => plan.execute(current_revision).await,
        }
    }
}
impl RemotePurgePlan {
    async fn capture(store: RemoteStorage, provider_id: &str) -> anyhow::Result<Self> {
        let records = tokio::time::timeout(NETWORK_DEADLINE, async {
            let mut records = Vec::new();
            let mut metadata_bytes = 0usize;
            for key in store.list().await? {
                let (bytes, etag) = store.get_version(&key).await?;
                if owned_record(&bytes, &key, provider_id).is_none() {
                    continue;
                }
                let etag = etag
                    .filter(|v| strong_etag(v))
                    .ok_or_else(|| anyhow::anyhow!("Archive snapshot requires a strong ETag"))?;
                metadata_bytes = metadata_bytes.saturating_add(key.len() + etag.len() + 32);
                if records.len() >= 1024 || metadata_bytes > 2 * 1024 * 1024 {
                    anyhow::bail!("Archive snapshot metadata limit exceeded");
                }
                records.push(FrozenObject {
                    key,
                    digest: Sha256::digest(bytes).into(),
                    etag,
                });
            }
            Ok::<_, anyhow::Error>(records)
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Archive snapshot timed out; no purge barrier or deletion was performed"
            )
        })??;
        Ok(Self { store, records })
    }
    async fn execute(
        self,
        current_revision: &(dyn Fn() -> bool + Send + Sync),
    ) -> anyhow::Result<usize> {
        let mut removed = 0;
        let result = tokio::time::timeout(NETWORK_DEADLINE, async {
            for record in self.records {
                let bytes = self.store.get(&record.key).await?;
                let digest: [u8; 32] = Sha256::digest(&bytes).into();
                if digest != record.digest {
                    anyhow::bail!("Archive changed after purge snapshot");
                }
                self.store
                    .delete_verified(&record.key, &bytes, &record.etag, current_revision)
                    .await?;
                removed += 1;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await;
        match result {
            Ok(Ok(())) => Ok(removed),
            Ok(Err(error)) => anyhow::bail!("Archive purge stopped after {removed} confirmed deletions; an unconfirmed DELETE may have completed; the purge barrier remains committed: {error}"),
            Err(_) => anyhow::bail!("Archive purge stopped after {removed} confirmed deletions; a timed-out delete may have completed; the purge barrier remains committed"),
        }
    }
}
fn strong_etag(value: &str) -> bool {
    value.len() >= 2
        && value.len() <= 1024
        && value.starts_with('"')
        && value.ends_with('"')
        && value[1..value.len() - 1]
            .bytes()
            .all(|b| b >= 0x21 && b != b'"' && b != 0x7f)
}

#[cfg(test)]
pub(super) async fn fixture_purge(
    store: RemoteStorage,
    provider_id: &str,
    current_revision: &(dyn Fn() -> bool + Send + Sync),
) -> anyhow::Result<usize> {
    RemotePurgePlan::capture(store, provider_id)
        .await?
        .execute(current_revision)
        .await
}
