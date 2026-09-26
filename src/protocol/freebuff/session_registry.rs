use super::{
    build_synthetic_session_snapshot, FreeBuffRuntimeConfig, FreeBuffSessionSnapshot,
    FreeBuffSessionState, FreeBuffWaitingRoomRejection, SessionObservation,
};
use crate::error::GatewayError;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

#[cfg(test)]
#[path = "session_registry_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "session_observation_tests.rs"]
mod observation_tests;

#[derive(Debug, Default)]
pub(super) struct FreeBuffSessionBucket {
    pub(super) snapshot: Option<FreeBuffSessionSnapshot>,
    pub(super) revision: Arc<()>,
    _registry_slot: Option<OwnedSemaphorePermit>,
}

impl FreeBuffSessionBucket {
    pub(super) fn set_snapshot(&mut self, snapshot: Option<FreeBuffSessionSnapshot>) {
        self.snapshot = snapshot;
        // A retained Arc identity cannot wrap or suffer ABA like an incrementing generation.
        self.revision = Arc::new(());
    }
}

pub(super) struct SessionRegistry {
    buckets: DashMap<String, Arc<Mutex<FreeBuffSessionBucket>>>,
    slots: Arc<Semaphore>,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new(256)
    }
}

impl SessionRegistry {
    fn new(capacity: usize) -> Self {
        Self {
            buckets: DashMap::new(),
            slots: Arc::new(Semaphore::new(capacity)),
        }
    }

    #[cfg(test)]
    pub(super) fn buckets(&self) -> &DashMap<String, Arc<Mutex<FreeBuffSessionBucket>>> {
        &self.buckets
    }

    pub(super) fn get_or_insert(
        &self,
        key: &str,
    ) -> Result<Arc<Mutex<FreeBuffSessionBucket>>, GatewayError> {
        if let Some(entry) = self.buckets.get(key) {
            return Ok(entry.value().clone());
        }
        let reserve = || self.slots.clone().try_acquire_owned();
        let permit = match reserve() {
            Ok(permit) => permit,
            Err(_) => {
                self.evict_terminal();
                reserve().map_err(|_| {
                    GatewayError::service_unavailable(
                        "FreeBuff session registry capacity exhausted",
                    )
                    .with_code("freebuff_session_registry_capacity_exhausted")
                    .with_provider("freebuff_compatible")
                })?
            }
        };
        // Entry races return the unused reservation; an escaped bucket keeps its own permit.
        Ok(self
            .buckets
            .entry(key.to_owned())
            .or_insert_with(|| {
                Arc::new(Mutex::new(FreeBuffSessionBucket {
                    snapshot: None,
                    _registry_slot: Some(permit),
                    ..Default::default()
                }))
            })
            .clone())
    }

    fn evict_terminal(&self) {
        self.buckets.retain(|_, bucket| {
            // Refresh expiry is not claim expiry; observations also pin their original bucket.
            let removable = Arc::strong_count(bucket) == 1
                && bucket.try_lock().is_ok_and(|state| {
                    state.snapshot.as_ref().is_none_or(|snapshot| {
                        matches!(
                            snapshot.state,
                            FreeBuffSessionState::Disabled
                                | FreeBuffSessionState::None
                                | FreeBuffSessionState::Expired
                                | FreeBuffSessionState::Superseded
                        )
                    })
                });
            !removable
        });
    }

    pub(super) async fn record_rejection(
        &self,
        config: &FreeBuffRuntimeConfig,
        observation: &SessionObservation,
        rejection: FreeBuffWaitingRoomRejection,
        body: &str,
    ) {
        let Some((observed_bucket, revision)) = &observation.source else {
            return;
        };
        let Some(bucket) = self
            .buckets
            .get(&config.session_bucket_key)
            .map(|entry| entry.value().clone())
        else {
            return;
        };
        if !Arc::ptr_eq(&bucket, observed_bucket) {
            return;
        }
        let mut state = bucket.lock().await;
        // Recheck under the write lock: a poll may have completed while this response waited.
        if !Arc::ptr_eq(&state.revision, revision) {
            return;
        }
        if state.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.instance_id.is_some()
                && matches!(
                    snapshot.state,
                    FreeBuffSessionState::Active
                        | FreeBuffSessionState::Queued
                        | FreeBuffSessionState::Draining
                )
                && snapshot.instance_id.as_deref() != observation.instance_id()
        }) {
            return;
        }
        state.set_snapshot(Some(build_synthetic_session_snapshot(
            rejection,
            observation.instance_id(),
            body,
            config,
        )));
    }
}
