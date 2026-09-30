//! Instance heartbeats isolate crash recovery from other live Gateway processes.
use super::{storage_error, LocalRuntime};
use crate::{error::GatewayError, state::AppState};
use std::{sync::Arc, time::Duration};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

impl LocalRuntime {
    pub(crate) async fn maintain(&self) -> Result<(), GatewayError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let now = OffsetDateTime::now_utc().unix_timestamp();
        sqlx::query(
            "INSERT INTO runtime_instances(id, heartbeat) VALUES (?, ?)
            ON CONFLICT(id) DO UPDATE SET heartbeat = excluded.heartbeat",
        )
        .bind(&self.owner)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(storage_error)?;
        sqlx::query("DELETE FROM runtime_instances WHERE heartbeat < ?")
            .bind(now - 120)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        super::access_balances::cleanup(&mut tx).await?;
        let time = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("timestamp");
        sqlx::query("UPDATE request_audits SET status = 'cancelled',
            payload = json_set(payload, '$.status', 'cancelled', '$.completedAt', ?, '$.updatedAt', ?,
                '$.errorSummary', 'Gateway instance stopped before request completion')
            WHERE status = 'running' AND owner NOT IN (SELECT id FROM runtime_instances)")
            .bind(&time).bind(&time).execute(&mut *tx).await.map_err(storage_error)?;
        // Prune in small batches so housekeeping cannot hold the writer lock for an unbounded interval.
        sqlx::query("DELETE FROM request_audits WHERE id IN (SELECT id FROM request_audits
            WHERE status != 'running' AND julianday(created) < julianday('now', '-90 days') LIMIT 500)")
            .execute(&mut *tx).await.map_err(storage_error)?;
        tx.commit().await.map_err(storage_error)?;
        self.prune_refill_tasks().await
    }

    pub(super) async fn retire(&self) -> Result<(), GatewayError> {
        let now = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("timestamp");
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        sqlx::query("UPDATE request_audits SET status = 'cancelled',
            payload = json_set(payload, '$.status', 'cancelled', '$.completedAt', ?, '$.updatedAt', ?)
            WHERE owner = ? AND status = 'running'")
            .bind(&now).bind(&now).bind(&self.owner).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("DELETE FROM runtime_instances WHERE id = ?")
            .bind(&self.owner)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        super::access_balances::cleanup(&mut tx).await?;
        tx.commit().await.map_err(storage_error)
    }
}

pub fn spawn_maintenance(state: &Arc<AppState>) {
    if state.local_runtime.is_none() {
        return;
    }
    let state = Arc::downgrade(state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            let Some(state) = state.upgrade() else {
                break;
            };
            if let Some(local) = &state.local_runtime {
                if local.pool.is_closed() {
                    break;
                }
                if let Err(error) = local.maintain().await {
                    tracing::warn!(code = ?error.code, "Local runtime maintenance failed");
                }
            }
        }
    });
}
