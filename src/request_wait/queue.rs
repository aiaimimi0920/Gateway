//! Bounded retained requests, FIFO recovery admission and RAII removal on cancellation.
use crate::error::GatewayError;
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
mod identity;
#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod session_tests;

const MAX_REQUESTS: usize = 64;
const MAX_PER_KEY: usize = 16;
const MAX_PER_PROVIDER: usize = 32;
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Default)]
pub(crate) struct WaitRegistry(Mutex<Entries>);
#[derive(Debug, Default)]
struct Entries {
    count: usize,
    bytes: usize,
    keys: HashMap<String, KeyWaiters>,
    providers: HashMap<String, (usize, Arc<AsyncMutex<()>>)>,
    identities: crate::provider_identity::ProviderAliases,
}
pub(crate) struct WaitLease {
    inner: Arc<Lease>,
}
struct Lease {
    registry: Arc<WaitRegistry>,
    key: String,
    session: Option<[u8; 32]>,
    provider: String,
    bytes: usize,
    gate: Arc<AsyncMutex<()>>,
}
pub(crate) struct WaitTurn {
    // Unlock before releasing the final accounting owner.
    _guard: OwnedMutexGuard<()>,
    _lease: Arc<Lease>,
}
impl WaitRegistry {
    #[cfg(test)]
    pub(crate) fn enter(
        self: &Arc<Self>,
        key: String,
        provider: String,
        bytes: usize,
    ) -> Result<WaitLease, GatewayError> {
        self.enter_session(key, None, provider, bytes)
    }

    pub(crate) fn enter_session(
        self: &Arc<Self>,
        key: String,
        session: Option<[u8; 32]>,
        provider: String,
        bytes: usize,
    ) -> Result<WaitLease, GatewayError> {
        let mut entries = self.0.lock();
        let provider = entries.identities.original(&provider).to_owned();
        if let Some(limit) = entries.full_reason(&key, session.as_ref(), &provider, bytes) {
            // Counts only: never log the caller's key/session label, credential or retained body.
            tracing::warn!(
                limit,
                requests = entries.count,
                key_slots = entries.keys.get(&key).map_or(0, |waiters| waiters.slots),
                provider_requests = entries.providers.get(&provider).map_or(0, |(n, _)| *n),
                retained_bytes = entries.bytes,
                requested_retained_bytes = bytes,
                "Recovery waiting queue admission rejected"
            );
            return Err(
                GatewayError::service_unavailable("Recovery waiting queue is full")
                    .with_code("recovery_wait_queue_full"),
            );
        }
        let (count, gate) = entries.providers.entry(provider.clone()).or_default();
        *count += 1;
        let gate = gate.clone();
        entries.keys.entry(key.clone()).or_default().add(session);
        entries.count += 1;
        entries.bytes += bytes;
        Ok(WaitLease {
            inner: Arc::new(Lease {
                registry: self.clone(),
                key,
                session,
                provider,
                bytes,
                gate,
            }),
        })
    }
}
impl WaitLease {
    pub(crate) async fn turn(&self) -> WaitTurn {
        WaitTurn {
            _guard: self.inner.gate.clone().lock_owned().await,
            _lease: self.inner.clone(),
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let mut entries = self.registry.0.lock();
        entries.count -= 1;
        entries.bytes -= self.bytes;
        if let Some(waiters) = entries.keys.get_mut(&self.key) {
            waiters.remove(self.session);
            if waiters.slots == 0 {
                entries.keys.remove(&self.key);
            }
        }
        let provider = entries.identities.original(&self.provider).to_owned();
        if let Some((count, _)) = entries.providers.get_mut(&provider) {
            *count -= 1;
            if *count == 0 {
                entries.providers.remove(&provider);
            }
        }
    }
}

#[derive(Debug, Default)]
struct KeyWaiters {
    slots: usize,
    sessions: HashMap<[u8; 32], usize>,
}
impl KeyWaiters {
    fn needs_slot(&self, session: Option<&[u8; 32]>) -> bool {
        session.is_none_or(|id| !self.sessions.contains_key(id))
    }
    fn add(&mut self, session: Option<[u8; 32]>) {
        if self.needs_slot(session.as_ref()) {
            self.slots += 1;
        }
        if let Some(id) = session {
            *self.sessions.entry(id).or_default() += 1;
        }
    }
    fn remove(&mut self, session: Option<[u8; 32]>) {
        if let Some(id) = session {
            let count = self.sessions.get_mut(&id).expect("live waiting session");
            *count -= 1;
            if *count != 0 {
                return;
            }
            self.sessions.remove(&id);
        }
        self.slots -= 1;
    }
}
impl Entries {
    fn full_reason(
        &self,
        key: &str,
        session: Option<&[u8; 32]>,
        provider: &str,
        bytes: usize,
    ) -> Option<&'static str> {
        if self.count >= MAX_REQUESTS {
            Some("global_requests")
        } else if bytes > MAX_BYTES.saturating_sub(self.bytes) {
            Some("retained_bytes")
        } else if self
            .keys
            .get(key)
            .is_some_and(|waiters| waiters.slots >= MAX_PER_KEY && waiters.needs_slot(session))
        {
            Some("key_slots")
        } else if self
            .providers
            .get(provider)
            .is_some_and(|(n, _)| *n >= MAX_PER_PROVIDER)
        {
            Some("provider_requests")
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn queue_is_bounded_and_drop_releases_every_identity() {
        let queue = Arc::new(WaitRegistry::default());
        let leases: Vec<_> = (0..MAX_PER_KEY)
            .map(|_| queue.enter("key".into(), "p".into(), 10).unwrap())
            .collect();
        assert!(queue.enter("key".into(), "p".into(), 10).is_err());
        assert!(queue.enter("other".into(), "p".into(), MAX_BYTES).is_err());
        drop(leases);
        let e = queue.0.lock();
        assert_eq!(
            (e.count, e.bytes, e.keys.len(), e.providers.len()),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn global_provider_and_retained_bytes_limits_are_independent() {
        let queue = Arc::new(WaitRegistry::default());
        let mut leases = Vec::new();
        for provider in ["a", "b"] {
            for i in 0..MAX_PER_PROVIDER {
                leases.push(
                    queue
                        .enter(format!("{provider}:{i}"), provider.into(), 1)
                        .unwrap(),
                );
            }
            assert!(queue.enter("overflow".into(), provider.into(), 1).is_err());
        }
        assert!(queue.enter("another".into(), "c".into(), 1).is_err());
        drop(leases);
        let all_bytes = queue.enter("key".into(), "a".into(), MAX_BYTES).unwrap();
        assert!(queue.enter("other".into(), "b".into(), 1).is_err());
        drop(all_bytes);
        assert_eq!(queue.0.lock().bytes, 0);
    }

    #[tokio::test]
    async fn fifo_waiter_cancellation_releases_identity_and_does_not_block_the_next() {
        let queue = Arc::new(WaitRegistry::default());
        let first = queue.enter("first".into(), "p".into(), 1).unwrap();
        let turn = first.turn().await;
        let second = queue.enter("second".into(), "p".into(), 1).unwrap();
        let second = tokio::spawn(async move { second.turn().await });
        tokio::task::yield_now().await;
        let third = queue.enter("third".into(), "p".into(), 1).unwrap();
        let third = tokio::spawn(async move { third.turn().await });
        tokio::task::yield_now().await;
        assert!(!second.is_finished() && !third.is_finished());
        second.abort();
        assert!(matches!(second.await, Err(error) if error.is_cancelled()));
        drop(turn);
        drop(third.await.unwrap());
        drop(first);
        assert_eq!(queue.0.lock().count, 0);
    }
}
