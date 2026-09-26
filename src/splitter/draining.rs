use super::{now_rfc3339, ManagedWorker, SplitterManager, SplitterWorkerStatus};
use anyhow::anyhow;
use axum::http::StatusCode;
use std::sync::Arc;
use std::time::Duration;
impl SplitterManager {
    pub(super) fn mark_worker_draining(
        &self,
        worker: &ManagedWorker,
        reason: &str,
    ) -> anyhow::Result<()> {
        self.transition_worker_status(worker, SplitterWorkerStatus::Draining)?;
        *worker.drain_requested_at.write() = Some(now_rfc3339());
        *worker.drain_reason.write() = Some(reason.to_string());
        Ok(())
    }

    pub(super) fn transition_worker_status(
        &self,
        worker: &ManagedWorker,
        next: SplitterWorkerStatus,
    ) -> anyhow::Result<()> {
        worker.transition_to(next)
    }

    pub(super) async fn drain_and_wait_for_exit(
        &self,
        worker: Arc<ManagedWorker>,
        shutdown_timeout_secs: u64,
    ) -> anyhow::Result<()> {
        let total_budget = Duration::from_secs(shutdown_timeout_secs.max(1));
        let hard_deadline = tokio::time::Instant::now() + total_budget;
        let kill_grace = (total_budget / 4)
            .max(Duration::from_millis(100))
            .min(Duration::from_secs(2));
        let graceful_deadline = hard_deadline - kill_grace;

        let requests_drained = worker
            .wait_for_no_active_requests_until(graceful_deadline)
            .await;
        if requests_drained {
            let drain_reason = worker
                .drain_reason
                .read()
                .clone()
                .unwrap_or_else(|| "splitter_shutdown".to_string());
            let drain_request = self
                .inner
                .ready_client
                .post(format!(
                    "{}/v1/internal/gateway/runtime/drain",
                    worker.base_url
                ))
                .header(
                    "x-management-token",
                    self.inner
                        .config
                        .gateway_management_token
                        .clone()
                        .unwrap_or_default(),
                )
                .json(&serde_json::json!({ "reason": drain_reason }))
                .send();
            match tokio::time::timeout_at(graceful_deadline, drain_request).await {
                Ok(Ok(response))
                    if response.status().is_success()
                        || response.status() == StatusCode::ACCEPTED =>
                {
                    tracing::info!(worker_id = %worker.id, "Requested graceful drain for gateway worker");
                }
                Ok(Ok(response)) => {
                    tracing::warn!(worker_id = %worker.id, status = %response.status(), "Gateway worker drain request returned non-success status");
                }
                Ok(Err(error)) => {
                    tracing::warn!(worker_id = %worker.id, ?error, "Gateway worker drain request failed");
                }
                Err(_) => {
                    tracing::warn!(worker_id = %worker.id, "Gateway worker drain request exceeded the shutdown deadline");
                }
            }
        } else {
            tracing::warn!(
                worker_id = %worker.id,
                active_requests = worker.active_requests(),
                "Gateway worker still has in-flight splitter requests at the graceful shutdown deadline"
            );
        }

        let Some(mut child) = worker.child.lock().await.take() else {
            if *worker.status.read() != SplitterWorkerStatus::Exited {
                self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
            }
            return Ok(());
        };

        if requests_drained {
            match tokio::time::timeout_at(graceful_deadline, child.wait()).await {
                Ok(Ok(status)) => {
                    self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
                    *worker.exit_status.write() = Some(status.to_string());
                    return Ok(());
                }
                Ok(Err(error)) => {
                    self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
                    *worker.exit_status.write() = Some(format!("wait_error: {error}"));
                    return Ok(());
                }
                Err(_) => {}
            }
        }

        tracing::warn!(worker_id = %worker.id, "Gateway worker exceeded graceful shutdown; forcing kill");
        let _ = child.start_kill();
        match tokio::time::timeout_at(hard_deadline, child.wait()).await {
            Ok(Ok(status)) => {
                self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
                *worker.exit_status.write() = Some(format!("forced_kill: {status}"));
                Ok(())
            }
            Ok(Err(error)) => {
                self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
                *worker.exit_status.write() = Some(format!("forced_kill_wait_error: {error}"));
                Ok(())
            }
            Err(_) => {
                *worker.exit_status.write() = Some("forced_kill_pending".to_string());
                *worker.child.lock().await = Some(child);
                Err(anyhow!(
                    "worker {} did not terminate within {} seconds",
                    worker.id,
                    shutdown_timeout_secs.max(1)
                ))
            }
        }
    }
}
