//! Virtual-time retry proof: no supplier, credentials or wall-clock sleeps.
use super::attempt_budget::tests::gate;
use crate::{
    error::{classify_upstream_error, GatewayError},
    retry::RetryPolicy,
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

fn capacity() -> GatewayError {
    classify_upstream_error(
        429,
        r#"{"error":{"code":"turn_rate_limit_exceeded","message":"Codex is at capacity, please try again later"}}"#,
        Some("chatgpt"),
    )
}

#[tokio::test(start_paused = true)]
async fn first_stream_capacity_can_retry_once_but_other_errors_cannot() {
    let gate = gate();
    let sends = AtomicUsize::new(0);
    let policy = RetryPolicy {
        max_retries: 0,
        ..Default::default()
    };
    let start = tokio::time::Instant::now();
    gate.execute_with_capacity_retry(
        || async {
            if sends.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(capacity())
            } else {
                Ok(())
            }
        },
        |_| {},
        &policy,
        true,
    )
    .await
    .unwrap();
    assert_eq!(sends.load(Ordering::SeqCst), 2);
    assert_eq!(gate.request_budget.attempts(), 2);
    assert_eq!(start.elapsed(), Duration::from_secs(2));
    let other = super::attempt_budget::tests::gate();
    let result: Result<(), _> = other
        .execute_observed(
            || async { Err(GatewayError::server_error("ordinary failure")) },
            |_| {},
            &policy,
        )
        .await;
    assert!(result.is_err());
    assert_eq!(other.request_budget.attempts(), 1);
    let disabled = super::attempt_budget::tests::gate();
    let result: Result<(), _> = disabled
        .execute_observed(|| async { Err(capacity()) }, |_| {}, &policy)
        .await;
    assert!(result.is_err());
    assert_eq!(
        disabled.request_budget.attempts(),
        1,
        "explicit adapter retry prohibition stays intact"
    );
}

#[tokio::test(start_paused = true)]
async fn repeated_capacity_has_one_retry_across_clones_and_recovery_loops() {
    let gate = gate();
    let start = tokio::time::Instant::now();
    for cloned in [gate.clone(), gate.clone()] {
        let result: Result<(), _> = cloned
            .execute_observed(
                || async { Err(capacity()) },
                |_| {},
                &RetryPolicy::default(),
            )
            .await;
        assert_eq!(
            result.unwrap_err().code.as_deref(),
            Some("turn_rate_limit_exceeded")
        );
    }
    assert_eq!(
        gate.request_budget.attempts(),
        3,
        "two initial sends and only one retry"
    );
    assert_eq!(start.elapsed(), Duration::from_secs(2));
    assert_eq!(
        gate.smart_routing
            .score("diagnostic-provider", "fixture-model"),
        100.0
    );
}

#[tokio::test(start_paused = true)]
async fn unsafe_deadline_and_excessive_hint_do_not_retry_or_clip_delay() {
    for (attempts, timeout, hint) in [
        (1, 30, 2_000),
        (6, 1, 2_000),
        (6, 300, u64::MAX),
        (6, 300, 16_000),
    ] {
        let mut gate = gate();
        gate.request_budget = crate::pipeline::request_budget::RequestBudget::new(
            attempts,
            Duration::from_secs(timeout),
        );
        let mut error = capacity();
        error.fallback_hint = crate::error::FallbackHint::Retry {
            delay_ms: hint,
            reason: "fixture".into(),
        };
        let start = tokio::time::Instant::now();
        let result: Result<(), _> = gate
            .execute_observed(
                || async { Err(error.clone()) },
                |_| {},
                &RetryPolicy::default(),
            )
            .await;
        let returned = result.unwrap_err();
        assert_eq!(returned.http_status, Some(429));
        assert_eq!(
            serde_json::to_value(returned.fallback_hint).unwrap(),
            serde_json::to_value(error.fallback_hint).unwrap()
        );
        assert_eq!(gate.request_budget.attempts(), 1);
        assert!(start.elapsed().is_zero());
    }
}

#[tokio::test(start_paused = true)]
async fn handed_off_owner_cannot_retry() {
    let gate = gate();
    let _stream = gate
        .request_budget
        .constrain_stream(futures::stream::empty::<Result<bytes::Bytes, GatewayError>>());
    assert!(!gate.request_budget.claim_capacity_retry(&capacity()));
    assert_eq!(gate.request_budget.attempts(), 0);
}
