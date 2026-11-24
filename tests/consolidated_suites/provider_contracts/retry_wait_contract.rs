//! The plain and pre-admitted retry APIs must not send before an upstream hint.
use std::time::Duration;

use neuro_gateway::error::GatewayError;
use neuro_gateway::retry::{
    execute_with_retry, execute_with_retry_after_admission_observed, RetryPolicy,
};
use tokio::time::Instant;

#[tokio::test(start_paused = true)]
async fn plain_retry_waits_for_upstream_hint() {
    let policy = RetryPolicy {
        max_retries: 1,
        initial_delay: Duration::from_millis(1),
        ..Default::default()
    };
    let started = Instant::now();
    let mut calls = 0;
    let result = execute_with_retry(
        || {
            calls += 1;
            async { Err::<(), _>(GatewayError::rate_limited("fixture", 2_000)) }
        },
        &policy,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(calls, 2);
    assert_eq!(started.elapsed(), Duration::from_secs(2));
}

#[tokio::test(start_paused = true)]
async fn admitted_retry_waits_before_readmission_and_reports_only_real_calls() {
    let policy = RetryPolicy {
        max_retries: 1,
        initial_delay: Duration::from_millis(1),
        ..Default::default()
    };
    let started = Instant::now();
    let mut admitted_at = Vec::new();
    let mut observations = 0;
    let result = execute_with_retry_after_admission_observed(
        || async { Err::<(), _>(GatewayError::rate_limited("fixture", 2_000)) },
        || {
            admitted_at.push(started.elapsed());
            async { Ok(()) }
        },
        |_| observations += 1,
        &policy,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(admitted_at, vec![Duration::from_secs(2)]);
    assert_eq!(observations, 2);
}
