//! Explicit, provider-scoped cloud storage. Local paths keep their existing owner.
pub(crate) mod archive;
mod archive_record;
mod capabilities;
mod configuration;
pub(crate) mod purge;
pub(crate) use capabilities::archive_purge_support;
pub(crate) mod refill;
mod s3;
#[cfg(test)]
mod tests;
mod webdav;

use crate::routing::config::CredentialStorageConnection;
pub(crate) use configuration::{authentication_configured, namespace, validate_connection};
use std::time::Duration;

pub(super) const MAX_OBJECT_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_LIST_ENTRIES: usize = 1024;
pub(super) const NETWORK_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
#[error("Storage object was not found")]
pub(super) struct StorageNotFound;

pub(super) fn validate_object_key(key: &str) -> anyhow::Result<()> {
    configuration::validate_prefix(key).map_err(anyhow::Error::msg)?;
    if key.is_empty() || key.contains('/') || !key.ends_with(".json") || key.len() > 512 {
        anyhow::bail!("Storage object must be a relative JSON file");
    }
    Ok(())
}

enum RemoteStorage {
    Webdav(webdav::WebdavStorage),
    S3(s3::S3Storage),
}

impl RemoteStorage {
    pub(super) fn new(
        connection: &CredentialStorageConnection,
        provider_id: &str,
        archive: bool,
    ) -> anyhow::Result<Self> {
        validate_connection(connection).map_err(anyhow::Error::msg)?;
        let namespace = namespace(connection, provider_id, archive);
        match connection {
            CredentialStorageConnection::Webdav { .. } => Ok(Self::Webdav(
                webdav::WebdavStorage::new(connection, &namespace)?,
            )),
            CredentialStorageConnection::S3 { .. } => {
                Ok(Self::S3(s3::S3Storage::new(connection, &namespace)?))
            }
            CredentialStorageConnection::Local { .. } => {
                anyhow::bail!("Expected an explicit cloud storage connection")
            }
        }
    }

    #[cfg(test)]
    fn allow_fixture_capabilities(&mut self) {
        match self {
            Self::Webdav(store) => store.allow_fixture_capabilities(),
            Self::S3(store) => store.allow_fixture_capabilities(),
        }
    }

    pub(super) async fn list(&self) -> anyhow::Result<Vec<String>> {
        match self {
            Self::Webdav(store) => store.list().await,
            Self::S3(store) => store.list().await,
        }
    }
    pub(super) async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        validate_object_key(key)?;
        match self {
            Self::Webdav(store) => store.get(key).await,
            Self::S3(store) => store.get(key).await,
        }
    }
    pub(super) async fn get_version(&self, key: &str) -> anyhow::Result<(Vec<u8>, Option<String>)> {
        validate_object_key(key)?;
        match self {
            Self::Webdav(store) => store.get_version(key).await,
            Self::S3(store) => store.get_version(key).await,
        }
    }
    pub(super) async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        validate_object_key(key)?;
        if bytes.len() > MAX_OBJECT_BYTES {
            anyhow::bail!("Storage object exceeds byte limit");
        }
        match self {
            Self::Webdav(store) => store.put(key, bytes).await,
            Self::S3(store) => store.put(key, bytes).await,
        }
    }
    pub(super) async fn delete_verified(
        &self,
        key: &str,
        bytes: &[u8],
        etag: &str,
        current_revision: &(dyn Fn() -> bool + Send + Sync),
    ) -> anyhow::Result<()> {
        validate_object_key(key)?;
        match self {
            Self::Webdav(store) => {
                store
                    .delete_verified(key, bytes, etag, current_revision)
                    .await
            }
            Self::S3(store) => {
                store
                    .delete_verified(key, bytes, etag, current_revision)
                    .await
            }
        }
    }
}

#[cfg(test)]
mod archive_tests;
