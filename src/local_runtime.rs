//! Durable single-machine runtime owners. Server mode keeps its existing stores.
use crate::error::GatewayError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::{path::Path, time::Duration};
pub(crate) mod access_balances;
pub mod access_keys;
mod affinity;
mod audit_queries;
pub mod audits;
mod backup;
mod finalizers;
pub(crate) mod freebuff_devices;
pub(crate) mod freebuff_sessions;
mod maintenance;
mod model_identity;
mod model_routing;
pub mod model_states;
mod pressure;
mod provider_identity;
mod refill_lock;
mod secret_seal;
pub(crate) use finalizers::FinalizerGuard;
pub use maintenance::spawn_maintenance;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct LocalRuntime {
    pub(crate) pool: SqlitePool,
    pub(crate) owner: String,
    key_secrets: access_keys::secrets::KeySecretStore,
    secret_seal: secret_seal::LocalSecretSeal,
    lock_directory: std::path::PathBuf,
    finalizers: std::sync::Arc<finalizers::Finalizers>,
    affinity: std::sync::Arc<affinity::AffinityCache>,
    pub(crate) rate_limits: std::sync::Arc<crate::rate_limit::MemoryRateLimitStore>,
}

impl LocalRuntime {
    pub async fn open(directory: &Path) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(directory).await?;
        let options = SqliteConnectOptions::new()
            .filename(directory.join("runtime.sqlite3"))
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(10));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut *tx)
            .await?;
        anyhow::ensure!(
            version <= 1,
            "Local runtime database was created by a newer Gateway"
        );
        if version == 0 {
            sqlx::raw_sql(
                "CREATE TABLE refill_tasks (
                    id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, payload TEXT NOT NULL,
                    terminal INTEGER NOT NULL DEFAULT 0, created INTEGER NOT NULL,
                    expires INTEGER NOT NULL, lease_token TEXT, lease_until INTEGER NOT NULL DEFAULT 0,
                    idempotency_key TEXT);
                 CREATE UNIQUE INDEX refill_outstanding ON refill_tasks(provider_id) WHERE terminal = 0;
                 CREATE UNIQUE INDEX refill_idempotency ON refill_tasks(provider_id,idempotency_key)
                    WHERE idempotency_key IS NOT NULL;
                 CREATE INDEX refill_recent ON refill_tasks(created DESC);
                 CREATE TABLE refill_deliveries (task_id TEXT PRIMARY KEY REFERENCES refill_tasks(id) ON DELETE CASCADE,
                    payload TEXT NOT NULL, mode TEXT NOT NULL, expected_count INTEGER NOT NULL);
                 CREATE TABLE request_audits (
                    id TEXT PRIMARY KEY, payload TEXT NOT NULL, created TEXT NOT NULL,
                    status TEXT NOT NULL, owner TEXT NOT NULL);
                 CREATE INDEX audit_recent ON request_audits(created DESC);
                 CREATE TABLE provider_pricing (provider_id TEXT PRIMARY KEY, payload TEXT NOT NULL);
                 CREATE TABLE credential_model_states (id TEXT PRIMARY KEY, payload TEXT NOT NULL, updated TEXT NOT NULL);
                 CREATE TABLE runtime_instances (id TEXT PRIMARY KEY, heartbeat INTEGER NOT NULL);
                 PRAGMA user_version = 1;"
            ).execute(&mut *tx).await?;
        }
        // Additive initialization also runs for already released schema-1 databases.
        sqlx::raw_sql(maintenance::AUDIT_INDEXES)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(access_keys::SCHEMA).execute(&mut *tx).await?;
        sqlx::raw_sql(freebuff_devices::SCHEMA)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(freebuff_sessions::SCHEMA)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(model_identity::SCHEMA)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(crate::cash_billing::SCHEMA)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(crate::provider_runtime::test_store::LOCAL_SCHEMA)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(crate::protocol::sap_aicore::responses_lifecycle::store::SCHEMA)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(crate::protocol::sap_aicore::responses_lifecycle::store::MUTATION_SCHEMA)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(crate::protocol::sap_aicore::responses_lifecycle::store::ROUTE_SCHEMA)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(crate::protocol::sap_aicore::responses_lifecycle::store::RECOVERY_SCHEMA)
            .execute(&mut *tx)
            .await?;
        access_balances::initialize(&mut tx)
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let sealed: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM local_access_key_secrets) + (SELECT count(*) FROM local_freebuff_device_keys) + (SELECT count(*) FROM local_freebuff_registrations) + (SELECT count(*) FROM local_freebuff_registration_replacements) + (SELECT count(*) FROM local_freebuff_session_claims) + (SELECT count(*) FROM local_freebuff_session_closeouts) + (SELECT count(*) FROM local_freebuff_runs)")
            .fetch_one(&mut *tx)
            .await?;
        let has_sap_pins: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM gateway_sap_response_route_pins)")
                .fetch_one(&mut *tx)
                .await?;
        let secret_directory = directory.to_owned();
        let secret_seal = tokio::task::spawn_blocking(move || {
            secret_seal::LocalSecretSeal::open(&secret_directory, sealed > 0 || has_sap_pins)
        })
        .await?;
        tx.commit().await?;
        let lock_directory = directory.join("refill-locks");
        tokio::fs::create_dir_all(&lock_directory).await?;
        let runtime = Self {
            pool,
            owner: uuid::Uuid::new_v4().to_string(),
            key_secrets: access_keys::secrets::KeySecretStore(secret_seal.clone()),
            secret_seal,
            lock_directory,
            finalizers: Default::default(),
            affinity: Default::default(),
            rate_limits: Default::default(),
        };
        runtime
            .maintain()
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        Ok(runtime)
    }

    pub async fn close(&self) {
        self.finalizers.wait().await;
        if let Err(error) = self.retire().await {
            tracing::warn!(code = ?error.code, "Could not retire local runtime instance");
        }
        self.pool.close().await;
    }

    pub fn track_finalizer(&self) -> finalizers::FinalizerGuard {
        self.finalizers.track()
    }

    #[allow(dead_code)] // SAP lifecycle dispatch is not public yet.
    pub(crate) fn sap_response_key(&self) -> Option<&[u8; 32]> {
        self.secret_seal.sap_response_key()
    }
}

pub(crate) fn storage_error(error: sqlx::Error) -> GatewayError {
    tracing::error!(error = %error, "Local runtime storage operation failed");
    GatewayError::server_error("Local runtime storage operation failed")
        .with_code("local_runtime_storage_error")
}
