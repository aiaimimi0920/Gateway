//! Shared wait decisions must remain bounded, cancellable and fail closed.
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use neuro_gateway::error::{ErrorKind, GatewayError};
use neuro_gateway::retry::{
    execute_with_retry, execute_with_retry_after_admission_observed, RetryPolicy,
};
use tokio::time::Instant;

async fn limited(
    admitted: bool,
    hint: u64,
    limit: u64,
    retries: usize,
) -> (usize, usize, Duration, GatewayError) {
    let policy = RetryPolicy {
        max_retries: retries,
        max_total_wait: Duration::from_millis(limit),
        initial_delay: Duration::ZERO,
        ..Default::default()
    };
    let started = Instant::now();
    let mut calls = 0;
    let mut admissions = 0;
    let mut observations = 0;
    let f = || {
        calls += 1;
        let error = GatewayError::rate_limited("last real upstream error", hint)
            .with_provider("fixture")
            .with_code(format!("real-call-{calls}"));
        async { Err::<(), _>(error) }
    };
    let result = if admitted {
        execute_with_retry_after_admission_observed(
            f,
            || {
                admissions += 1;
                async { Ok(()) }
            },
            |_| observations += 1,
            &policy,
        )
        .await
    } else {
        execute_with_retry(f, &policy).await
    };
    if admitted {
        assert_eq!(observations, calls);
        assert_eq!(admissions, calls - 1);
    }
    (calls, admissions, started.elapsed(), result.unwrap_err())
}

#[tokio::test(start_paused = true)]
async fn cumulative_wait_never_exceeds_limit_and_preserves_last_error() {
    for admitted in [false, true] {
        let (calls, _, elapsed, error) = limited(admitted, 6, 15, 9).await;
        assert_eq!(calls, 3);
        assert_eq!(elapsed, Duration::from_millis(12));
        assert_eq!(error.code.as_deref(), Some("real-call-3"));
        assert_eq!(error.kind, ErrorKind::RateLimit);
        assert_eq!(error.http_status, Some(429));
        assert_eq!(error.provider_name.as_deref(), Some("fixture"));
    }
}

#[tokio::test(start_paused = true)]
async fn excessive_and_overflow_hints_stop_without_sleep_or_readmission() {
    for admitted in [false, true] {
        for hint in [16, u64::MAX] {
            let (calls, admissions, elapsed, error) = limited(admitted, hint, 15, 9).await;
            assert_eq!((calls, admissions), (1, 0));
            assert_eq!(elapsed, Duration::ZERO);
            assert_eq!(error.code.as_deref(), Some("real-call-1"));
        }
    }
}

#[tokio::test(start_paused = true)]
async fn exact_limit_and_zero_hint_do_not_create_off_by_one_retries() {
    for admitted in [false, true] {
        let (calls, _, elapsed, _) = limited(admitted, 5, 15, 9).await;
        assert_eq!(calls, 4);
        assert_eq!(elapsed, Duration::from_millis(15));
        let (calls, _, _, _) = limited(admitted, 0, 0, 2).await;
        assert_eq!(calls, 3);
    }
}

#[tokio::test(start_paused = true)]
async fn local_backoff_also_requires_wait_admission() {
    let policy = RetryPolicy {
        initial_delay: Duration::from_secs(1),
        max_total_wait: Duration::from_millis(1),
        ..Default::default()
    };
    for admitted in [false, true] {
        let mut calls = 0;
        let f = || {
            calls += 1;
            async { Err::<(), _>(GatewayError::server_error("fixture")) }
        };
        let start = Instant::now();
        let result = if admitted {
            execute_with_retry_after_admission_observed(
                f,
                || async { panic!("must not readmit") },
                |_| {},
                &policy,
            )
            .await
        } else {
            execute_with_retry(f, &policy).await
        };
        assert!(result.is_err());
        assert_eq!(calls, 1);
        assert_eq!(start.elapsed(), Duration::ZERO);
    }
}

#[tokio::test(start_paused = true)]
async fn permanent_statuses_override_retry_hints_for_both_apis() {
    for admitted in [false, true] {
        for status in [400, 401, 403, 404] {
            let mut calls = 0;
            let f = || {
                calls += 1;
                let mut error = GatewayError::rate_limited("permanent", 1);
                error.http_status = Some(status);
                async { Err::<(), _>(error) }
            };
            let policy = RetryPolicy::default();
            let error = if admitted {
                execute_with_retry_after_admission_observed(
                    f,
                    || async { panic!("must not readmit") },
                    |_| {},
                    &policy,
                )
                .await
            } else {
                execute_with_retry(f, &policy).await
            }
            .unwrap_err();
            assert_eq!(calls, 1);
            assert_eq!(error.http_status, Some(status));
        }
    }
}

#[tokio::test(start_paused = true)]
async fn admission_rejection_is_not_an_upstream_attempt() {
    let mut observations = 0;
    let started = Instant::now();
    let error = execute_with_retry_after_admission_observed(
        || async { Err::<(), _>(GatewayError::rate_limited("fixture", 2_000)) },
        || async { Err(GatewayError::unauthorized("admission rejected")) },
        |_| observations += 1,
        &RetryPolicy::default(),
    )
    .await
    .unwrap_err();
    assert_eq!(started.elapsed(), Duration::from_secs(2));
    assert_eq!(observations, 1);
    assert_eq!(error.kind, ErrorKind::Authentication);
}

#[tokio::test(start_paused = true)]
async fn dropping_either_retry_future_during_sleep_prevents_later_sends() {
    for admitted in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let admissions = Arc::new(AtomicUsize::new(0));
        let task_calls = Arc::clone(&calls);
        let task_admissions = Arc::clone(&admissions);
        let mut future = Box::pin(async move {
            let f = || {
                task_calls.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(GatewayError::rate_limited("fixture", 5_000)) }
            };
            if admitted {
                execute_with_retry_after_admission_observed(
                    f,
                    || {
                        task_admissions.fetch_add(1, Ordering::SeqCst);
                        async { Ok(()) }
                    },
                    |_| {},
                    &RetryPolicy::default(),
                )
                .await
            } else {
                execute_with_retry(f, &RetryPolicy::default()).await
            }
        });
        assert!(futures::poll!(future.as_mut()).is_pending());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        drop(future);
        tokio::time::advance(Duration::from_secs(60)).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(admissions.load(Ordering::SeqCst), 0);
    }
}
