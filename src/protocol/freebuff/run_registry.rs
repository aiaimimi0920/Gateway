use super::FreeBuffRunBucket;
use crate::error::GatewayError;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

#[cfg(test)]
#[path = "run_registry_tests.rs"]
mod tests;

pub(super) struct RunRegistry {
    buckets: DashMap<String, Arc<Mutex<FreeBuffRunBucket>>>,
    slots: Arc<Semaphore>,
}

impl Default for RunRegistry {
    fn default() -> Self {
        Self::new(256)
    }
}

impl RunRegistry {
    fn new(capacity: usize) -> Self {
        Self {
            buckets: DashMap::new(),
            slots: Arc::new(Semaphore::new(capacity)),
        }
    }

    pub(super) fn buckets(&self) -> &DashMap<String, Arc<Mutex<FreeBuffRunBucket>>> {
        &self.buckets
    }

    #[cfg(test)]
    pub(super) fn get_or_insert(
        &self,
        key: &str,
    ) -> Result<Arc<Mutex<FreeBuffRunBucket>>, GatewayError> {
        self.get_or_insert_checked(key, || Ok(()))
    }

    pub(super) fn get_or_insert_checked(
        &self,
        key: &str,
        admit: impl FnOnce() -> Result<(), GatewayError>,
    ) -> Result<Arc<Mutex<FreeBuffRunBucket>>, GatewayError> {
        if let Some(entry) = self.buckets.get(key) {
            return Ok(entry.value().clone());
        }
        let reserve = || self.slots.clone().try_acquire_owned();
        let permit = match reserve() {
            Ok(permit) => permit,
            Err(_) => {
                self.evict_idle();
                reserve().map_err(|_| {
                    registry_error(
                        "freebuff_run_registry_capacity_exhausted",
                        "FreeBuff run registry capacity exhausted",
                    )
                })?
            }
        };
        // Atomic reservation counts preparing entries and escaped owners, not a racy map length.
        // If another creator installed this key, dropping the unused closure returns its permit.
        let entry = self.buckets.entry(key.to_owned());
        // Hold the shard write lock across the admission check and insertion. Shutdown's
        // snapshot/clear must then see this bucket, or closure must reject its late creation.
        admit()?;
        Ok(entry
            .or_insert_with(|| {
                Arc::new(Mutex::new(FreeBuffRunBucket {
                    active: None,
                    _registry_slot: Some(permit),
                }))
            })
            .clone())
    }

    fn evict_idle(&self) {
        self.buckets.retain(|_, bucket| {
            // Under the shard write lock, a borrowed acquirer cannot appear after this check.
            let idle = Arc::strong_count(bucket) == 1
                && bucket.try_lock().is_ok_and(|state| {
                    state
                        .active
                        .as_ref()
                        .is_none_or(|run| run.lifetime.inflight() == 0)
                });
            !idle
        });
    }
}

fn registry_error(code: &'static str, message: &'static str) -> GatewayError {
    GatewayError::service_unavailable(message)
        .with_code(code)
        .with_provider("freebuff_compatible")
}
