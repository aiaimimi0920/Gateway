use super::fixture::{candidate, context, Upstream};
use axum::http::StatusCode;
use neuro_gateway::balance::BalanceStatus;
use neuro_gateway::routing::{
    candidate::RouteCandidate,
    latency::{LatencyKey, RoutingLatency},
    queue::{build_candidate_queue, build_candidate_queue_with_latency, HealthSnapshot},
};
use std::{collections::HashMap, time::Duration};

fn ids(queue: &[RouteCandidate]) -> Vec<&str> {
    queue
        .iter()
        .map(|c| c.provider_account_id.as_str())
        .collect()
}

#[tokio::test]
async fn latency_breaks_only_equal_eligible_default_ties() {
    let upstream = Upstream::start(StatusCode::OK, false, "{}").await;
    let candidates = vec![
        candidate(&upstream, "slow"),
        candidate(&upstream, "unknown"),
        candidate(&upstream, "fast"),
    ];
    let req = context(false, vec![]).canonical_req;
    let samples = RoutingLatency::default();
    let old = build_candidate_queue(
        &candidates,
        "priority_weighted",
        None,
        &HashMap::new(),
        None,
    );
    let cold = build_candidate_queue_with_latency(
        &candidates,
        "priority_weighted",
        None,
        &HashMap::new(),
        None,
        Some((&samples, &req)),
    );
    assert_eq!(ids(&old), ids(&cold));
    samples.record(
        LatencyKey::for_request(&candidates[0], &req).unwrap(),
        Duration::from_millis(2_000),
    );
    samples.record(
        LatencyKey::for_request(&candidates[2], &req).unwrap(),
        Duration::from_millis(10),
    );
    let queue = build_candidate_queue_with_latency(
        &candidates,
        "priority_weighted",
        None,
        &HashMap::new(),
        None,
        Some((&samples, &req)),
    );
    assert_eq!(
        ids(&queue),
        vec![
            candidates[2].provider_account_id.as_str(),
            candidates[1].provider_account_id.as_str(),
            candidates[0].provider_account_id.as_str()
        ]
    );
    let rr = build_candidate_queue_with_latency(
        &candidates,
        "round_robin",
        None,
        &HashMap::new(),
        None,
        Some((&samples, &req)),
    );
    assert_eq!(ids(&rr), ids(&candidates));
    let mut weighted = candidates.clone();
    weighted[0].weight += 1;
    let queue = build_candidate_queue_with_latency(
        &weighted,
        "priority_weighted",
        None,
        &HashMap::new(),
        None,
        Some((&samples, &req)),
    );
    assert_eq!(
        queue[0].provider_account_id,
        candidates[0].provider_account_id
    );
    let mut prioritized = candidates.clone();
    prioritized[0].priority += 1;
    let queue = build_candidate_queue_with_latency(
        &prioritized,
        "priority_weighted",
        None,
        &HashMap::new(),
        None,
        Some((&samples, &req)),
    );
    assert_eq!(
        queue[0].provider_account_id,
        candidates[0].provider_account_id
    );
    let sticky = build_candidate_queue_with_latency(
        &candidates,
        "priority_weighted",
        Some(&candidates[0].provider_account_id),
        &HashMap::new(),
        None,
        Some((&samples, &req)),
    );
    assert_eq!(
        sticky[0].provider_account_id,
        candidates[0].provider_account_id
    );
    upstream.finish().await;
}

#[tokio::test]
async fn latency_and_sticky_never_resurrect_breaker_or_exhausted_quota() {
    let upstream = Upstream::start(StatusCode::OK, false, "{}").await;
    let candidates = vec![candidate(&upstream, "slow"), candidate(&upstream, "fast")];
    let req = context(false, vec![]).canonical_req;
    let samples = RoutingLatency::default();
    samples.record(
        LatencyKey::for_request(&candidates[1], &req).unwrap(),
        Duration::from_millis(1),
    );
    let fast_id = candidates[1].runtime_subject_id().to_string();
    let mut health = HashMap::from([(
        fast_id.clone(),
        HealthSnapshot {
            status: "active".into(),
            active_concurrency: 0,
            failure_count: 0,
            breaker_open: true,
            balance_status: None,
        },
    )]);
    for strategy in ["priority_weighted", "round_robin", "random"] {
        let queue = build_candidate_queue_with_latency(
            &candidates,
            strategy,
            Some(&candidates[1].provider_account_id),
            &health,
            None,
            Some((&samples, &req)),
        );
        assert_eq!(
            queue[0].provider_account_id,
            candidates[0].provider_account_id
        );
    }
    health.get_mut(&fast_id).unwrap().breaker_open = false;
    health.get_mut(&fast_id).unwrap().balance_status = Some(BalanceStatus {
        provider_account_id: fast_id.clone(),
        display: "empty".into(),
        reason: None,
        should_deprioritize: false,
        is_unavailable: true,
        remaining_ratio: Some(0.0),
    });
    let queue = build_candidate_queue_with_latency(
        &candidates,
        "priority_weighted",
        Some(&candidates[1].provider_account_id),
        &health,
        None,
        Some((&samples, &req)),
    );
    assert_eq!(queue.len(), 1);
    assert_eq!(
        queue[0].provider_account_id,
        candidates[0].provider_account_id
    );
    upstream.finish().await;
}

#[tokio::test]
async fn oversized_keys_are_rejected_and_effective_upstream_alias_is_used() {
    let upstream = Upstream::start(StatusCode::OK, false, "{}").await;
    let mut selected = candidate(&upstream, "keys");
    let req = context(false, vec![]).canonical_req;
    let samples = RoutingLatency::default();
    let key = LatencyKey::for_request(&selected, &req).unwrap();
    samples.record(key.clone(), Duration::from_millis(10));
    let mut alias = req.clone();
    alias.requested_model = Some("different-caller-alias".into());
    assert_eq!(key, LatencyKey::for_request(&selected, &alias).unwrap());
    selected.upstream_model = Some("another-upstream-model".into());
    assert!(samples
        .estimate_ms(&LatencyKey::for_request(&selected, &req).unwrap())
        .is_none());
    selected.upstream_model = Some("x".repeat(257));
    assert!(LatencyKey::for_request(&selected, &req).is_none());
    upstream.finish().await;
}
