use std::collections::HashMap;
use std::sync::Arc;

use futures::{stream, StreamExt};

use crate::concurrency::aimd::ConcurrencySnapshot;
use crate::provider_quota;
use crate::routing::candidate::RouteCandidate;
use crate::routing::queue::HealthSnapshot;
use crate::state::AppState;

use super::route_health_cache;

const MAX_CONCURRENT_QUOTA_REFRESHES: usize = 8;

pub(super) async fn collect_candidate_health_snapshots(
    candidates: Vec<RouteCandidate>,
    state: Arc<AppState>,
    concurrency_snapshots: HashMap<String, ConcurrencySnapshot>,
) -> (Vec<RouteCandidate>, HashMap<String, HealthSnapshot>) {
    let mut quota_slots = Vec::with_capacity(candidates.len());
    let mut quota_runtime_ids = Vec::new();
    for candidate in &candidates {
        if !candidate.provider_account_id.starts_with("cred:")
            && provider_quota::provider_supports_quota(&candidate.payload)
        {
            quota_slots.push(Some(quota_runtime_ids.len()));
            quota_runtime_ids.push((
                candidate.provider_account_id.as_str(),
                candidate.provider_credential_id.as_deref(),
            ));
        } else {
            quota_slots.push(None);
        }
    }
    let breaker_runtime_ids = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.provider_account_id.as_str(),
                candidate.provider_credential_id.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    let (cached_quota, breaker_open) = tokio::join!(
        route_health_cache::read_cached_quota_snapshots(&state.redis_pool, &quota_runtime_ids),
        route_health_cache::read_breakers_open(&state.redis_pool, &breaker_runtime_ids),
    );
    let mut cached_quota = cached_quota.unwrap_or_else(|_| vec![None; quota_runtime_ids.len()]);
    drop(quota_runtime_ids);
    drop(breaker_runtime_ids);

    let prepared = candidates
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| {
            let cached_quota = quota_slots[index]
                .and_then(|slot| cached_quota.get_mut(slot))
                .and_then(Option::take);
            let breaker_open = breaker_open.get(index).copied().unwrap_or(false);
            (candidate, cached_quota, breaker_open)
        })
        .collect::<Vec<_>>();

    // Only stale or missing quota entries perform network refresh work. Buffered
    // execution preserves input order and the prior duplicate-ID last-write rule.
    let results = stream::iter(prepared)
        .map(move |(candidate, cached_quota, breaker_open)| {
            let state = Arc::clone(&state);
            let active_concurrency = concurrency_snapshots
                .get(&candidate.provider_account_id)
                .map(|snapshot| snapshot.active_count)
                .unwrap_or(0);
            async move {
                let balance_status = if candidate.provider_account_id.starts_with("cred:") {
                    None
                } else {
                    provider_quota::get_or_refresh_runtime_quota_snapshot_with_cache(
                        &state.redis_pool,
                        state.config.upstream_timeout_secs,
                        &candidate.provider_account_id,
                        candidate.provider_credential_id.as_deref(),
                        &candidate.payload,
                        cached_quota,
                    )
                    .await
                    .as_ref()
                    .map(provider_quota::quota_to_balance_status)
                };
                let runtime_subject_id = candidate.runtime_subject_id().to_string();
                let health_snapshot = HealthSnapshot {
                    status: "active".to_string(),
                    active_concurrency,
                    failure_count: candidate.failure_count,
                    breaker_open,
                    balance_status,
                };
                (candidate, runtime_subject_id, health_snapshot)
            }
        })
        .buffered(MAX_CONCURRENT_QUOTA_REFRESHES)
        .collect::<Vec<_>>()
        .await;
    let mut candidates = Vec::with_capacity(results.len());
    let mut health_snapshots = HashMap::with_capacity(results.len());
    for (candidate, runtime_subject_id, health_snapshot) in results {
        candidates.push(candidate);
        health_snapshots.insert(runtime_subject_id, health_snapshot);
    }
    (candidates, health_snapshots)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn bounded_collect_preserves_order_and_limits_parallelism() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let output = stream::iter(0..8)
            .map(|index| {
                let active = Arc::clone(&active);
                let peak = Arc::clone(&peak);
                async move {
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(current, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis((8 - index) * 2)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    index
                }
            })
            .buffered(3)
            .collect::<Vec<_>>()
            .await;

        assert_eq!(output, (0..8).collect::<Vec<_>>());
        assert!(peak.load(Ordering::SeqCst) > 1);
        assert!(peak.load(Ordering::SeqCst) <= 3);
    }
}
