//! Retain a received successor before the first persistence await; retries perform storage only.
use super::{error, SessionAttempt, SessionState, SessionStore};
use crate::error::GatewayError;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct Pending {
    expected: String,
    next: String,
    _permit: Arc<tokio::sync::OwnedSemaphorePermit>,
}
impl SessionStore {
    pub(super) async fn retain_and_save(
        &self,
        attempt: &SessionAttempt,
        next: SessionState,
    ) -> Result<(), GatewayError> {
        let pending = Pending {
            expected: attempt.state.encode(),
            next: next.encode(),
            _permit: attempt.permit.clone(),
        };
        {
            let mut entries = self.pending.lock();
            if let Some(old) = entries.get(&attempt.key) {
                if old.expected != pending.expected || old.next != pending.next {
                    return Err(error("lumalabs_session_store_conflict"));
                }
            } else {
                entries.insert(attempt.key.clone(), pending);
            }
        }
        self.flush_key(&attempt.key).await
    }
    pub(super) async fn flush_key(&self, key: &str) -> Result<(), GatewayError> {
        let Some(pending) = self.pending.lock().get(key).cloned() else {
            return Ok(());
        };
        let saved = self.cas(key, Some(&pending.expected), &pending.next).await;
        if saved
            .as_ref()
            .is_err_and(|e| e.code.as_deref() != Some("lumalabs_session_store_conflict"))
        {
            return saved;
        }
        let mut entries = self.pending.lock();
        if entries.get(key).is_some_and(|current| {
            current.expected == pending.expected && current.next == pending.next
        }) {
            entries.remove(key);
        }
        saved
    }
    pub(crate) async fn flush_pending(&self) {
        let keys: Vec<_> = self.pending.lock().keys().cloned().collect();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        for key in keys {
            let Ok(result) = tokio::time::timeout_at(deadline, self.flush_key(&key)).await else {
                tracing::warn!("Luma Web session persistence flush deadline reached");
                break;
            };
            if let Err(error) = result {
                tracing::warn!(code = ?error.code, "Luma Web session receipt persistence remains pending");
            }
        }
    }

    pub(crate) async fn release_unstarted(
        &self,
        attempt: &SessionAttempt,
    ) -> Result<(), GatewayError> {
        let mut next = attempt.state.clone();
        next.attempt = None;
        self.retain_and_save(attempt, next).await
    }
}
