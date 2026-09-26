use super::{SplitterManager, SplitterWorkerStatus};
use std::sync::atomic::Ordering;
use std::time::Duration;
impl SplitterManager {
    pub(super) fn start_worker_supervisor(&self) {
        self.inner
            .supervisor_shutdown
            .store(false, Ordering::SeqCst);
        let manager = self.clone();
        tokio::spawn(async move {
            while !manager.inner.supervisor_shutdown.load(Ordering::SeqCst) {
                manager.reconcile_worker_processes().await;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
    }

    pub(super) async fn reconcile_worker_processes(&self) {
        let workers: Vec<_> = self.inner.workers.read().values().cloned().collect();
        for worker in workers {
            let mut child_guard = worker.child.lock().await;
            let Some(child) = child_guard.as_mut() else {
                continue;
            };
            let exit_status = match child.try_wait() {
                Ok(Some(status)) => Some(status.to_string()),
                Ok(None) => None,
                Err(error) => Some(format!("wait_error: {error}")),
            };
            let Some(exit_status) = exit_status else {
                continue;
            };

            let was_active = worker.status.read().can_receive_traffic();
            let current_status = *worker.status.read();
            if current_status != SplitterWorkerStatus::Exited {
                let _ = worker.transition_to(SplitterWorkerStatus::Exited);
            }
            *worker.exit_status.write() = Some(if was_active {
                format!("crashed: {exit_status}")
            } else {
                exit_status
            });
            *child_guard = None;
            let clears_active =
                self.inner.active_worker_id.read().as_deref() == Some(worker.id.as_str());
            if clears_active {
                *self.inner.active_worker_id.write() = None;
            }
            worker.request_notify.notify_waiters();
        }
    }

    pub(super) async fn shutdown_all_workers(&self) {
        let workers: Vec<_> = self.inner.workers.read().values().cloned().collect();
        for worker in workers {
            if *worker.status.read() != SplitterWorkerStatus::Exited {
                if let Err(error) = self.mark_worker_draining(worker.as_ref(), "splitter_shutdown")
                {
                    tracing::warn!(worker_id = %worker.id, ?error, "Failed to mark worker draining during splitter exit");
                }
            }
            if let Err(error) = self
                .drain_and_wait_for_exit(
                    worker,
                    self.inner.config.splitter_reload_shutdown_timeout_secs,
                )
                .await
            {
                tracing::warn!(?error, "Failed to shut down worker during splitter exit");
            }
        }
    }
}
