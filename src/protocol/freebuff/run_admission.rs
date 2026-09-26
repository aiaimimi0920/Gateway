use super::FreeBuffRunBucket;
use crate::error::GatewayError;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedSemaphorePermit};

#[cfg(test)]
#[path = "run_admission_tests.rs"]
mod tests;

pub(super) fn reserve_with_reclamation(
    buckets: &DashMap<String, Arc<Mutex<FreeBuffRunBucket>>>,
    mut reserve: impl FnMut() -> Result<OwnedSemaphorePermit, GatewayError>,
) -> Result<OwnedSemaphorePermit, GatewayError> {
    if let Ok(permit) = reserve() {
        return Ok(permit);
    }
    for entry in buckets.iter() {
        // Acquirers increment under this mutex. Never wait while holding a map shard guard.
        if let Ok(mut state) = entry.value().try_lock() {
            if state
                .active
                .as_ref()
                .is_some_and(|run| run.lifetime.inflight() == 0)
            {
                state.active = None;
            }
        }
    }
    // FINISH retains its permit until completion: pressure may still require a caller retry.
    reserve()
}

pub(super) fn reserve_for_rotation(
    state: &mut FreeBuffRunBucket,
    buckets: &DashMap<String, Arc<Mutex<FreeBuffRunBucket>>>,
    reserve: impl FnMut() -> Result<OwnedSemaphorePermit, GatewayError>,
) -> Result<OwnedSemaphorePermit, GatewayError> {
    // The caller already established staleness. Retire before admission so a full pool can drain.
    // Live old leases keep their lifetime; keeping the state mutex prevents duplicate STARTs.
    state.active = None;
    reserve_with_reclamation(buckets, reserve)
}
