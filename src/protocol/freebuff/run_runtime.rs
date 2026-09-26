use super::registry::RunRegistry;
use crate::error::GatewayError;
use crate::protocol::freebuff::run_slots::RunSlots;
use std::time::Duration;

#[cfg(test)]
#[path = "run_runtime_tests.rs"]
mod tests;

/// Runtime-owned run cache and admission; network jobs never retain this owner through config.
#[derive(Default)]
pub struct RunRuntime {
    pub(super) registry: RunRegistry,
    pub(super) slots: RunSlots,
}

impl std::fmt::Debug for RunRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FreeBuffRunRuntime").finish_non_exhaustive()
    }
}

impl RunRuntime {
    pub(crate) fn ensure_open(&self) -> Result<(), GatewayError> {
        self.slots.ensure_open()
    }

    pub(crate) async fn shutdown(&self, budget: Duration) -> Result<(), GatewayError> {
        self.slots.close();
        tokio::time::timeout(budget, async {
            // Admission is closed before taking the bounded snapshot. Late empty bucket inserts
            // cannot install an active run because acquisition rechecks closure under its lock.
            let buckets: Vec<_> = self
                .registry
                .buckets()
                .iter()
                .map(|entry| entry.value().clone())
                .collect();
            for bucket in buckets {
                bucket.lock().await.active.take();
            }
            self.registry.buckets().clear();
            self.slots.wait_for_idle(budget).await
        })
        .await
        .map_err(|_| {
            GatewayError::service_unavailable("FreeBuff run shutdown timed out")
                .with_code("freebuff_run_shutdown_timeout")
                .with_provider("freebuff_compatible")
        })?
    }
}
