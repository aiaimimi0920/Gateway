//! Durable single-machine runtime owners. Server mode keeps its existing stores.
use crate::error::GatewayError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::{path::Path, time::Duration};
mod access_balances;
pub mod access_keys;
mod affinity;
mod audit_queries;
pub mod audits;
mod finalizers;
mod maintenance;
mod model_routing;
pub mod model_states;
mod pressure;
mod refill_lock;
pub use maintenance::spawn_maintenance;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct LocalRuntime {
    pub(crate) pool: SqlitePool,
    pub(crate) owner: String,
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
        sqlx::raw_sql(access_keys::SCHEMA).execute(&mut *tx).await?;
        access_balances::initialize(&mut tx)
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        tx.commit().await?;
        let lock_directory = directory.join("refill-locks");
        tokio::fs::create_dir_all(&lock_directory).await?;
        let runtime = Self {
            pool,
            owner: uuid::Uuid::new_v4().to_string(),
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
}

pub(crate) fn storage_error(error: sqlx::Error) -> GatewayError {
    tracing::error!(error = %error, "Local runtime storage operation failed");
    GatewayError::server_error("Local runtime storage operation failed")
        .with_code("local_runtime_storage_error")
}
