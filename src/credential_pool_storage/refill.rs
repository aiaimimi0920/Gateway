//! Read-only material collection obeys the same delivery limits as local folders.
use super::{RemoteStorage, NETWORK_DEADLINE};
use crate::routing::config::{CredentialStorageConnection, ProviderCredentialYaml};
use std::collections::HashSet;

pub(crate) async fn collect(
    connection: &CredentialStorageConnection,
    provider_id: &str,
    paths: &[String],
    requested: usize,
) -> anyhow::Result<Vec<ProviderCredentialYaml>> {
    let store = RemoteStorage::new(connection, provider_id, false)?;
    tokio::time::timeout(NETWORK_DEADLINE, async {
        let paths = if paths.is_empty() {
            store.list().await?
        } else {
            paths.to_vec()
        };
        if paths.len() > requested.min(128) {
            anyhow::bail!(
                "Refill file count exceeds requested batch; select explicit relativePaths"
            );
        }
        let mut selected = HashSet::new();
        let mut total = 0usize;
        let mut credentials = Vec::with_capacity(paths.len());
        for path in paths {
            super::validate_object_key(&path)?;
            if !selected.insert(path.clone()) {
                anyhow::bail!("Refill file selection contains duplicate paths");
            }
            let bytes = store.get(&path).await?;
            total = total.saturating_add(bytes.len());
            if bytes.len() > 1024 * 1024 || total > 4 * 1024 * 1024 {
                anyhow::bail!("Refill JSON exceeds bounded delivery size");
            }
            let credential = serde_json::from_slice(&bytes).map_err(|_| {
                anyhow::anyhow!("Refill file must contain one ProviderCredentialYaml JSON object")
            })?;
            credentials.push(credential);
        }
        Ok(credentials)
    })
    .await
    .map_err(|_| anyhow::anyhow!("Refill storage operation deadline exceeded"))?
}
