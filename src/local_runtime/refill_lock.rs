//! Bounded cross-process delivery fencing; OS releases locks on crash.
use super::LocalRuntime;
use crate::error::GatewayError;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions, TryLockError};

impl LocalRuntime {
    pub(crate) async fn prune_refill_tasks(&self) -> Result<(), GatewayError> {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM refill_tasks WHERE expires <= ? AND lease_until <= ? LIMIT 500",
        )
        .bind(now)
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(super::storage_error)?;
        for id in ids {
            let Some(_guard) = self.try_refill_lock(&id).await? else {
                continue;
            };
            sqlx::query(
                "DELETE FROM refill_tasks WHERE id = ? AND expires <= ? AND lease_until <= ?",
            )
            .bind(id)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(super::storage_error)?;
        }
        Ok(())
    }

    pub(crate) async fn try_refill_lock(
        &self,
        task_id: &str,
    ) -> Result<Option<File>, GatewayError> {
        // A fixed stripe set bounds disk metadata and never incorporates user path text.
        let stripe = Sha256::digest(task_id.as_bytes())[0];
        let path = self.lock_directory.join(format!("{stripe:02x}.lock"));
        tokio::task::spawn_blocking(move || {
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(path)
                .map_err(|_| lock_error())?;
            match file.try_lock() {
                Ok(()) => Ok(Some(file)),
                Err(TryLockError::WouldBlock) => Ok(None),
                Err(TryLockError::Error(_)) => Err(lock_error()),
            }
        })
        .await
        .map_err(|_| lock_error())?
    }
}
fn lock_error() -> GatewayError {
    GatewayError::server_error("Cannot acquire local refill delivery lock")
        .with_code("local_refill_lock_error")
}
