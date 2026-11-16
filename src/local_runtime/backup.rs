//! Consistent SQLite snapshot and reuse of the local seal for saved sync credentials.
use super::LocalRuntime;
use futures::FutureExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;

impl LocalRuntime {
    pub(crate) fn seal_backup_destination(&self, bytes: &[u8]) -> Result<String, String> {
        self.secret_seal
            .seal(b"gateway-backup-destination-v1", bytes, 32 * 1024)
            .map_err(|_| "Cannot protect sync credentials".into())
    }
    pub(crate) fn open_backup_destination(&self, value: &str) -> Result<Vec<u8>, String> {
        self.secret_seal
            .open_bytes(
                b"gateway-backup-destination-v1",
                value,
                48 * 1024,
                32 * 1024,
            )
            .map_err(|_| "Cannot read protected sync credentials".into())
    }
    pub(crate) async fn backup_database(&self, path: &Path) -> Result<(), String> {
        // VACUUM INTO is SQLite's consistent online snapshot, not a live WAL file copy.
        sqlx::query("VACUUM INTO ?")
            .bind(path.to_string_lossy().as_ref())
            .execute(&self.pool)
            .boxed()
            .await
            .map_err(|_| "Cannot create consistent database snapshot")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(path))
            .await
            .map_err(|_| "Cannot open database snapshot")?;
        let result = async {
            // Retain running requests and monetary/idempotency journals, never repeat uncertain charges.
            sqlx::query("DELETE FROM request_audits WHERE status != 'running'")
                .execute(&pool)
                .boxed()
                .await?;
            sqlx::query("DELETE FROM credential_test_results")
                .execute(&pool)
                .boxed()
                .await?;
            // Scrub free pages so excluded history is not recoverable from the backup file.
            sqlx::query("VACUUM").execute(&pool).boxed().await?;
            Ok::<(), sqlx::Error>(())
        }
        .await;
        pool.close().await;
        result.map_err(|_| "Cannot sanitize database snapshot".into())
    }
}
