//! Read key restrictions and identity aliases in one SQLite snapshot.
//! The shared migration registry is authority, not a provider-name heuristic.
use super::{storage_error, LocalRuntime};
use crate::{error::GatewayError, provider_identity::ProviderAliases};
use sqlx::SqliteConnection;

pub(super) async fn expand_restrictions(
    connection: &mut SqliteConnection,
    allowed: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, GatewayError> {
    let Some(allowed) = allowed else {
        return Ok(None);
    };
    if allowed.is_empty() {
        return Ok(Some(allowed));
    }
    Ok(Some(read(connection).await?.expand(&allowed)))
}

impl LocalRuntime {
    pub(crate) async fn provider_identity_snapshot(&self) -> Result<ProviderAliases, GatewayError> {
        read(&mut *self.pool.acquire().await.map_err(storage_error)?).await
    }
}

async fn read(connection: &mut SqliteConnection) -> Result<ProviderAliases, GatewayError> {
    // LIMIT includes one sentinel row. Never truncate authority silently.
    let pairs: Vec<(String, String)> = sqlx::query_as(
        "SELECT alias,canonical FROM model_provider_aliases ORDER BY alias LIMIT 257",
    )
    .fetch_all(connection)
    .await
    .map_err(storage_error)?;
    ProviderAliases::new(pairs).map_err(|message| {
        GatewayError::server_error(message).with_code("provider_identity_registry_invalid")
    })
}
