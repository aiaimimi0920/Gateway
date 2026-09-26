#[cfg(test)]
use super::run_slots::{reserve_from, MAX_TRACKED_RUNS};
use super::transport::finish_run;
use super::FreeBuffRuntimeConfig;
use crate::error::GatewayError;
use rquest::Client;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(test)]
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OwnedSemaphorePermit;
#[cfg(test)]
use tokio::sync::Semaphore;
use tracing::warn;

const FINISH_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
#[path = "run_lifetime_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "run_invalidation_tests.rs"]
mod invalidation_tests;

struct FinishJob {
    client: Client,
    config: FreeBuffRuntimeConfig,
    run_id: String,
    _permit: OwnedSemaphorePermit,
}

pub(super) struct RunLifetime {
    inflight: AtomicU64,
    request_count: AtomicU64,
    invalidated: AtomicBool,
    finish: Option<FinishJob>,
}

impl std::fmt::Debug for RunLifetime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunLifetime")
            .field("inflight", &self.inflight())
            .finish_non_exhaustive()
    }
}

impl RunLifetime {
    pub(super) fn tracked(
        client: Client,
        config: FreeBuffRuntimeConfig,
        run_id: String,
        permit: OwnedSemaphorePermit,
    ) -> Self {
        Self {
            inflight: AtomicU64::new(0),
            request_count: AtomicU64::new(0),
            invalidated: AtomicBool::new(false),
            finish: Some(FinishJob {
                client,
                config,
                run_id,
                _permit: permit,
            }),
        }
    }

    #[cfg(test)]
    pub(super) fn untracked() -> Self {
        Self {
            inflight: AtomicU64::new(0),
            request_count: AtomicU64::new(0),
            invalidated: AtomicBool::new(false),
            finish: None,
        }
    }

    pub(super) fn acquire(&self) -> Result<(), GatewayError> {
        if self.is_invalidated() {
            return Err(invalidated_error());
        }
        self.inflight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count.checked_add(1)
            })
            .map_err(|_| {
                GatewayError::service_unavailable("FreeBuff run lease count exhausted")
                    .with_code("freebuff_run_inflight_overflow")
            })?;
        // Admission linearizes at this final validity check. Earlier leases remain valid;
        // a concurrent invalidation observed here rolls back only this provisional counter.
        if self.is_invalidated() {
            self.release();
            return Err(invalidated_error());
        }
        let _ = self
            .request_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                Some(count.saturating_add(1))
            });
        Ok(())
    }

    pub(super) fn release(&self) {
        self.inflight.fetch_sub(1, Ordering::Release);
    }
    pub(super) fn inflight(&self) -> u64 {
        self.inflight.load(Ordering::Acquire)
    }
    pub(super) fn invalidate(&self) {
        self.invalidated.store(true, Ordering::Release);
    }
    pub(super) fn is_invalidated(&self) -> bool {
        self.invalidated.load(Ordering::Acquire)
    }

    pub(super) async fn finish_now(mut self) -> Result<(), GatewayError> {
        let Some(job) = self.finish.take() else {
            return Ok(());
        };
        // The reserved slot remains in the task even if the awaiting probe is cancelled.
        spawn_finish(job, self.request_count.load(Ordering::Acquire))?
            .await
            .map_err(|_| {
                GatewayError::server_error("FreeBuff FINISH task was cancelled")
                    .with_code("freebuff_finish_task_cancelled")
            })?
    }
}

fn invalidated_error() -> GatewayError {
    GatewayError::service_unavailable("FreeBuff run was invalidated before lease admission")
        .with_code("freebuff_run_invalidated")
        .with_provider("freebuff_compatible")
}

impl Drop for RunLifetime {
    fn drop(&mut self) {
        let Some(job) = self.finish.take() else {
            return;
        };
        if self.is_invalidated() {
            return;
        }
        // Arc invokes this destructor once: no per-lease task or parked-run queue is required.
        if let Err(error) = spawn_finish(job, self.request_count.load(Ordering::Acquire)) {
            warn!(error = %error, "freebuff FINISH could not be scheduled; remote state unresolved");
        }
    }
}

fn spawn_finish(
    job: FinishJob,
    steps: u64,
) -> Result<tokio::task::JoinHandle<Result<(), GatewayError>>, GatewayError> {
    let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
        GatewayError::service_unavailable("FreeBuff FINISH requires a running runtime")
            .with_code("freebuff_finish_runtime_unavailable")
    })?;
    Ok(runtime.spawn(async move {
        let result = tokio::time::timeout(FINISH_TIMEOUT,
            finish_run(&job.client, &job.config, &job.run_id, steps)).await
            .unwrap_or_else(|_| Err(GatewayError::service_unavailable("FreeBuff FINISH timed out")
                .with_code("freebuff_finish_timeout")));
        if let Err(error) = &result {
            warn!(run_id = %job.run_id, error = %error, "freebuff FINISH failed; remote state unresolved");
        }
        // Keep the admission permit through the entire network operation.
        drop(job);
        result
    }))
}
