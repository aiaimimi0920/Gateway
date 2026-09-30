//! Single-process fixed windows. Admission across all dimensions is atomic.
use super::{RateLimitBackendDecision, RateLimitRule, RateLimitStore, RateLimitStoreError};
use async_trait::async_trait;
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

const MAX_WINDOWS: usize = 8192;

#[derive(Default)]
pub struct MemoryRateLimitStore {
    windows: Mutex<HashMap<[u8; 32], (u64, Instant)>>,
}

#[async_trait]
impl RateLimitStore for MemoryRateLimitStore {
    async fn admit(
        &self,
        rules: &[RateLimitRule],
    ) -> Result<RateLimitBackendDecision, RateLimitStoreError> {
        let now = Instant::now();
        let keys: Vec<[u8; 32]> = rules
            .iter()
            .map(|rule| Sha256::digest(rule.key.as_bytes()).into())
            .collect();
        let mut windows = self.windows.lock();
        windows.retain(|_, (_, expiry)| *expiry > now);
        for (rule_index, (rule, key)) in rules.iter().zip(&keys).enumerate() {
            let (count, expiry) = windows.get(key).copied().unwrap_or((0, now));
            if count >= rule.max_requests {
                return Ok(RateLimitBackendDecision::Rejected {
                    rule_index,
                    retry_after_millis: (expiry
                        .saturating_duration_since(now)
                        .as_millis()
                        .min(u64::MAX as u128) as u64)
                        .saturating_add(1),
                });
            }
        }
        let new_keys = keys
            .iter()
            .filter(|key| !windows.contains_key(*key))
            .collect::<std::collections::HashSet<_>>()
            .len();
        if windows.len() + new_keys > MAX_WINDOWS {
            // Never evict a live counter: eviction would permit extra requests.
            return Err(RateLimitStoreError::Indeterminate(
                "Local rate-limit capacity reached".into(),
            ));
        }
        let expiries = rules
            .iter()
            .map(|rule| {
                now.checked_add(Duration::from_secs(rule.window_seconds.max(1)))
                    .ok_or_else(|| {
                        RateLimitStoreError::Indeterminate(
                            "Rate-limit window is out of range".into(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (key, expiry) in keys.into_iter().zip(expiries) {
            let counter = windows.entry(key).or_insert((0, expiry));
            counter.0 = counter.0.saturating_add(1);
        }
        Ok(RateLimitBackendDecision::Allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rate_limit::RateLimitScope;

    fn rule(key: &str, max_requests: u64) -> RateLimitRule {
        RateLimitRule {
            key: key.into(),
            max_requests,
            window_seconds: 60,
            scope: RateLimitScope::AccessKey,
        }
    }

    #[tokio::test]
    async fn concurrent_admission_and_rejected_dimensions_do_not_leak_capacity() {
        let store = std::sync::Arc::new(MemoryRateLimitStore::default());
        let calls = (0..100).map(|_| {
            let store = store.clone();
            tokio::spawn(async move { store.admit(&[rule("shared", 20)]).await.unwrap() })
        });
        let results = futures::future::join_all(calls).await;
        assert_eq!(
            results
                .into_iter()
                .filter(|result| matches!(result, Ok(RateLimitBackendDecision::Allowed)))
                .count(),
            20
        );
        assert!(matches!(
            store
                .admit(&[rule("new", 1), rule("shared", 20)])
                .await
                .unwrap(),
            RateLimitBackendDecision::Rejected { rule_index: 1, .. }
        ));
        assert_eq!(
            store.admit(&[rule("new", 1)]).await.unwrap(),
            RateLimitBackendDecision::Allowed
        );
        store
            .windows
            .lock()
            .values_mut()
            .for_each(|(_, expiry)| *expiry = Instant::now());
        assert_eq!(
            store.admit(&[rule("shared", 20)]).await.unwrap(),
            RateLimitBackendDecision::Allowed
        );
    }
}
