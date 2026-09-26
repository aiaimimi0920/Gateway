use super::run_lifetime::RunLifetime;
use super::FreeBuffRuntimeConfig;
use crate::error::GatewayError;
#[cfg(test)]
use dashmap::DashMap;
use rquest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OwnedSemaphorePermit};
use tracing::warn;

#[path = "run_admission.rs"]
mod admission;

#[path = "run_registry.rs"]
mod registry;

#[path = "run_runtime.rs"]
mod runtime;
pub use runtime::RunRuntime;

#[cfg(test)]
#[path = "run_test_support.rs"]
mod test_support;
#[cfg(test)]
pub(super) use test_support::{
    acquire_run_lease, probe_run, remove_idle_bucket, run_bucket, run_buckets, test_runtime,
};

#[path = "run_start.rs"]
mod start;
use start::start_tracked_run;

#[cfg(test)]
#[path = "run_bucket_tests.rs"]
mod bucket_tests;

#[cfg(test)]
#[path = "run_retirement_tests.rs"]
mod retirement_tests;

#[derive(Debug, Default)]
pub(super) struct FreeBuffRunBucket {
    pub(super) active: Option<ManagedRun>,
    _registry_slot: Option<OwnedSemaphorePermit>,
}

#[derive(Debug, Clone)]
pub(super) struct ManagedRun {
    pub(super) run_id: String,
    started_at: Instant,
    lifetime: Arc<RunLifetime>,
}

impl ManagedRun {
    #[cfg(test)]
    pub(super) fn new(run_id: String) -> Self {
        Self {
            run_id,
            started_at: Instant::now(),
            lifetime: Arc::new(RunLifetime::untracked()),
        }
    }

    #[cfg(test)]
    pub(super) fn inflight(&self) -> u64 {
        self.lifetime.inflight()
    }

    fn is_stale(&self, rotation_interval: Duration) -> bool {
        self.lifetime.is_invalidated() || self.started_at.elapsed() >= rotation_interval
    }
}

#[derive(Debug)]
pub(super) struct FreeBuffRunLease {
    runtime: Arc<RunRuntime>,
    bucket_key: String,
    pub(super) run_id: String,
    lifetime: Arc<RunLifetime>,
    released: bool,
}

impl FreeBuffRunLease {
    fn release_inflight(&mut self) {
        if !std::mem::replace(&mut self.released, true) {
            self.lifetime.release();
        }
    }
}

impl Drop for FreeBuffRunLease {
    fn drop(&mut self) {
        // The counter belongs to the lease, including while cleanup waits for the state lock.
        self.release_inflight();
    }
}

impl RunRuntime {
    pub(super) async fn probe_run(
        &self,
        client: &Client,
        config: &FreeBuffRuntimeConfig,
    ) -> Result<(), GatewayError> {
        let permit =
            admission::reserve_with_reclamation(self.registry.buckets(), || self.slots.reserve())?;
        let (_, lifetime) = start_tracked_run(client, config, permit).await?;
        lifetime.finish_now().await
    }

    pub(super) async fn acquire_run_lease(
        self: &Arc<Self>,
        client: &Client,
        config: &FreeBuffRuntimeConfig,
    ) -> Result<FreeBuffRunLease, GatewayError> {
        self.ensure_open()?;
        let bucket = self
            .registry
            .get_or_insert_checked(config.bucket_key.as_str(), || self.ensure_open())?;
        let mut state = bucket.lock().await;
        self.ensure_open()?;
        let needs_new_run = state
            .active
            .as_ref()
            .map(|run| run.is_stale(config.rotation_interval))
            .unwrap_or(true);

        if needs_new_run {
            let permit =
                admission::reserve_for_rotation(&mut state, self.registry.buckets(), || {
                    self.slots.reserve()
                })?;
            let (run_id, lifetime) = start_tracked_run(client, config, permit).await?;
            self.ensure_open()?;
            let fresh_run = ManagedRun {
                lifetime: Arc::new(lifetime),
                run_id,
                started_at: Instant::now(),
            };
            // Old leases retain the retired lifetime; its final owner schedules bounded FINISH.
            state.active = Some(fresh_run);
        }

        let active = state.active.as_mut().ok_or_else(|| {
            GatewayError::server_error("FreeBuff run acquisition ended without an active run")
                .with_code("freebuff_missing_active_run")
        })?;
        active.lifetime.acquire()?;
        let lease = FreeBuffRunLease {
            runtime: self.clone(),
            bucket_key: config.bucket_key.clone(),
            run_id: active.run_id.clone(),
            lifetime: active.lifetime.clone(),
            released: false,
        };
        drop(state);

        Ok(lease)
    }
}

pub(super) async fn release_run_lease(mut lease: FreeBuffRunLease) {
    lease.release_inflight();
    let Some(bucket) = lease
        .runtime
        .registry
        .buckets()
        .get(lease.bucket_key.as_str())
        .map(|entry| entry.value().clone())
    else {
        return;
    };

    let state = bucket.lock().await;
    let remove_bucket = state.active.is_none();
    drop(state);

    if remove_bucket {
        remove_idle_bucket_from(&lease.runtime, lease.bucket_key.as_str(), &bucket);
    }
}

pub(super) async fn invalidate_run_lease(
    config: &FreeBuffRuntimeConfig,
    mut lease: FreeBuffRunLease,
    reason: &str,
) {
    lease.lifetime.invalidate();
    lease.release_inflight();
    let Some(bucket) = lease
        .runtime
        .registry
        .buckets()
        .get(lease.bucket_key.as_str())
        .map(|entry| entry.value().clone())
    else {
        return;
    };

    let mut state = bucket.lock().await;
    if state
        .active
        .as_ref()
        .is_some_and(|run| Arc::ptr_eq(&run.lifetime, &lease.lifetime))
    {
        state.active = None;
    }

    let remove_bucket = state.active.is_none();
    drop(state);

    if remove_bucket {
        remove_idle_bucket_from(&lease.runtime, lease.bucket_key.as_str(), &bucket);
    }

    if !reason.trim().is_empty() {
        warn!(
            bucket_key = %config.bucket_key,
            run_id = %lease.run_id,
            reason = reason.trim(),
            "freebuff run invalidated"
        );
    }
}

fn remove_idle_bucket_from(
    runtime: &RunRuntime,
    key: &str,
    bucket: &Arc<Mutex<FreeBuffRunBucket>>,
) -> bool {
    runtime
        .registry
        .buckets()
        .remove_if(key, |_, registered| {
            // Under the shard write lock, only registry + this cleanup may own the bucket.
            // Existing acquirers retain an Arc; try_lock avoids reversing the state/map lock order.
            Arc::ptr_eq(registered, bucket)
                && Arc::strong_count(registered) == 2
                && registered
                    .try_lock()
                    .is_ok_and(|state| state.active.is_none())
        })
        .is_some()
}
