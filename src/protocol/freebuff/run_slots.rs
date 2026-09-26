use crate::error::GatewayError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(super) const MAX_TRACKED_RUNS: u32 = 128;

/// Admission and completion barrier for one run owner; closing never aborts existing work.
pub(super) struct RunSlots {
    slots: Arc<Semaphore>,
    capacity: u32,
    closing: AtomicBool,
}

impl Default for RunSlots {
    fn default() -> Self {
        Self::new(MAX_TRACKED_RUNS)
    }
}

impl RunSlots {
    fn new(capacity: u32) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(capacity as usize)),
            capacity,
            closing: AtomicBool::new(false),
        }
    }

    pub(super) fn ensure_open(&self) -> Result<(), GatewayError> {
        if self.closing.load(Ordering::Acquire) {
            return Err(closed_error());
        }
        Ok(())
    }

    pub(super) fn reserve(&self) -> Result<OwnedSemaphorePermit, GatewayError> {
        self.ensure_open()?;
        let permit = reserve_from(&self.slots)?;
        // The earlier check cannot authorize a permit acquired after shutdown closes admission.
        self.ensure_open()?;
        Ok(permit)
    }

    pub(super) fn close(&self) {
        self.closing.store(true, Ordering::Release);
    }

    pub(super) async fn wait_for_idle(&self, budget: Duration) -> Result<(), GatewayError> {
        self.close();
        // Do not close the semaphore: returned permits are the completion signal, including
        // detached START and FINISH operations. The barrier does not cancel those operations.
        let _barrier =
            tokio::time::timeout(budget, self.slots.clone().acquire_many_owned(self.capacity))
                .await
                .map_err(|_| {
                    GatewayError::service_unavailable("FreeBuff run shutdown timed out")
                        .with_code("freebuff_run_shutdown_timeout")
                        .with_provider("freebuff_compatible")
                })?
                .map_err(|_| closed_error())?;
        Ok(())
    }
}

fn closed_error() -> GatewayError {
    GatewayError::service_unavailable("FreeBuff run admission is closed")
        .with_code("freebuff_run_admission_closed")
        .with_provider("freebuff_compatible")
}

pub(super) fn reserve_from(slots: &Arc<Semaphore>) -> Result<OwnedSemaphorePermit, GatewayError> {
    slots.clone().try_acquire_owned().map_err(|_| {
        GatewayError::service_unavailable(
            "FreeBuff active/retired/finishing run capacity exhausted",
        )
        .with_code("freebuff_run_capacity_exhausted")
        .with_provider("freebuff_compatible")
    })
}

#[cfg(test)]
#[path = "run_slots_tests.rs"]
mod tests;
