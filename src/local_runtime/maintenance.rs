//! Instance heartbeats isolate crash recovery from other live Gateway processes.
use super::{storage_error, LocalRuntime};
use crate::{error::GatewayError, state::AppState};
use std::{sync::Arc, time::Duration};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

// Keep the predicates and age expression aligned with the partial indexes;
// this work runs under the SQLite writer lock even when there is nothing to clean up.
pub(super) const AUDIT_INDEXES: &str =
    "CREATE INDEX IF NOT EXISTS audit_running_owner ON request_audits(owner) WHERE status = 'running';
     CREATE INDEX IF NOT EXISTS audit_retention_age ON request_audits(julianday(created)) WHERE status != 'running';
     CREATE INDEX IF NOT EXISTS audit_luma_pending ON request_audits(created) WHERE json_extract(payload, '$.routeTrace.lumaReconciliation.state') = 'pending';
     CREATE INDEX IF NOT EXISTS credential_model_states_updated ON credential_model_states(updated DESC);
     CREATE INDEX IF NOT EXISTS refill_tasks_expiry_lease ON refill_tasks(expires, lease_until);";

pub(super) const CANCEL_ORPHANED_AUDITS: &str = "UPDATE request_audits SET status = 'cancelled',
    payload = json_set(payload, '$.status', 'cancelled', '$.completedAt', ?, '$.updatedAt', ?,
        '$.errorSummary', 'Gateway instance stopped before request completion')
    WHERE status = 'running' AND owner NOT IN (SELECT id FROM runtime_instances)";

pub(super) const PRUNE_EXPIRED_AUDITS: &str =
    "DELETE FROM request_audits WHERE id IN (SELECT id FROM request_audits
    WHERE status != 'running' AND julianday(created) < julianday('now', '-90 days') LIMIT 500)";

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
        sqlx::query(CANCEL_ORPHANED_AUDITS)
            .bind(&time)
            .bind(&time)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        // Prune in small batches so housekeeping cannot hold the writer lock for an unbounded interval.
        sqlx::query(PRUNE_EXPIRED_AUDITS)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
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
    let weak = Arc::downgrade(state);
    let stop = state.lifecycle.local_maintenance_stop.clone();
    state.lifecycle.background_tasks.spawn(
        crate::runtime::background_tasks::Worker::LocalMaintenance,
        &state.lifecycle.local_maintenance_stop,
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    biased;
                    _ = stop.wait() => break,
                    _ = interval.tick() => {},
                }
                let Some(state) = weak.upgrade() else {
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
        },
    );
}
