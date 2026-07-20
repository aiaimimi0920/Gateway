// ---------------------------------------------------------------------------
// Retry policy + execute_with_retry
//
// Rust port of the legacy TypeScript retry helper.
// Provides exponential back-off with ±10 % jitter for upstream calls.
// ---------------------------------------------------------------------------

use std::time::{Duration, Instant};

use rand::Rng;
use tokio::time::sleep;
use tracing::warn;

use crate::error::GatewayError;

// ---------------------------------------------------------------------------
// RetryPolicy
// ---------------------------------------------------------------------------

/// Configuration for the retry loop that wraps each upstream call.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of *retries* (not total attempts). Default: 2.
    pub max_retries: usize,
    /// Delay before the first retry. Default: 500 ms.
    pub initial_delay: Duration,
    /// Hard cap on the computed delay (before jitter). Default: 5 s.
    pub max_delay: Duration,
    /// Exponential growth factor per attempt. Default: 2.0.
    pub backoff_multiplier: f64,
    /// HTTP status codes that qualify for a retry. Default: [429, 500, 502, 503, 504].
    pub retryable_statuses: Vec<u16>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            retryable_statuses: vec![429, 500, 502, 503, 504],
        }
    }
}

// ---------------------------------------------------------------------------
// Delay computation
// ---------------------------------------------------------------------------

/// Compute the sleep duration before attempt number `attempt` (0-indexed).
///
/// Formula: `clamp(initial * multiplier^attempt, 0, max) * jitter`
/// where jitter is a uniform factor in `[0.9, 1.1)`.
pub fn compute_delay(attempt: usize, policy: &RetryPolicy) -> Duration {
    let base_ms =
        policy.initial_delay.as_millis() as f64 * policy.backoff_multiplier.powi(attempt as i32);

    let capped_ms = base_ms.min(policy.max_delay.as_millis() as f64);

    // ±10 % jitter — uniform in [0.9, 1.1).
    let jitter: f64 = rand::thread_rng().gen_range(0.9..1.1);
    let jittered_ms = (capped_ms * jitter).floor() as u64;

    Duration::from_millis(jittered_ms)
}

// ---------------------------------------------------------------------------
// Retry eligibility
// ---------------------------------------------------------------------------

/// Return `true` when `error` should cause the call to be retried according
/// to `policy`.
///
/// An error is retryable when:
/// 1. Its `retryable` flag is set **and**
/// 2. Either the error has no HTTP status code, or the status code appears in
///    `policy.retryable_statuses`.
///
/// Client errors (400, 401, 403, 404) are never retried even if the
/// `GatewayError` `retryable` field happens to be set.
pub fn should_retry(error: &GatewayError, policy: &RetryPolicy) -> bool {
    if !error.retryable {
        return false;
    }

    if let Some(status) = error.http_status {
        // Hard-veto known permanent client errors regardless of policy.
        if matches!(status, 400 | 401 | 403 | 404) {
            return false;
        }
        return policy.retryable_statuses.contains(&status);
    }

    // No HTTP status — trust the `retryable` flag (e.g. network errors).
    true
}

// ---------------------------------------------------------------------------
// execute_with_retry
// ---------------------------------------------------------------------------

/// Execute an async closure `f`, retrying on transient errors as dictated by
/// `policy`.
///
/// - The closure is called up to `policy.max_retries + 1` times total.
/// - Between each failed attempt that qualifies for a retry, the task sleeps
///   for `compute_delay(attempt, policy)`.
/// - If all attempts fail, the **last** error is returned.
///
/// # Example
///
/// ```rust,ignore
/// let result = execute_with_retry(
///     || async { call_upstream(&client, &req).await },
///     &RetryPolicy::default(),
/// )
/// .await?;
/// ```
pub async fn execute_with_retry<T, F, Fut>(
    mut f: F,
    policy: &RetryPolicy,
) -> Result<T, GatewayError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, GatewayError>>,
{
    let mut last_error: Option<GatewayError> = None;

    for attempt in 0..=policy.max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt < policy.max_retries && should_retry(&e, policy) {
                    let delay = compute_delay(attempt, policy);
                    warn!(
                        attempt,
                        delay_ms = delay.as_millis() as u64,
                        kind = ?e.kind,
                        provider = ?e.provider_name,
                        "upstream call failed; retrying"
                    );
                    sleep(delay).await;
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    // Unreachable in practice: the loop above always returns on the last
    // attempt (either Ok or the non-retryable Err branch), but the compiler
    // cannot prove that without this fallback.
    Err(last_error.expect("retry loop exited without a result"))
}

/// Execute an upstream closure whose first transport attempt was already
/// admitted, invoking `admit_retry` immediately before every later retry.
/// If retry admission fails, that fail-closed admission error is terminal.
pub async fn execute_with_retry_after_admission<T, F, Fut, A, AFut>(
    f: F,
    admit_retry: A,
    policy: &RetryPolicy,
) -> Result<T, GatewayError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, GatewayError>>,
    A: FnMut() -> AFut,
    AFut: std::future::Future<Output = Result<(), GatewayError>>,
{
    execute_with_retry_after_admission_observed(f, admit_retry, |_| {}, policy).await
}

#[derive(Debug, Clone, Copy)]
pub struct RetryAttemptObservation<'a> {
    pub succeeded: bool,
    pub latency_ms: u64,
    pub error: Option<&'a GatewayError>,
}

/// Execute a pre-admitted upstream closure and report each real transport
/// attempt after it completes. Retry admission failures do not produce an
/// observation because no upstream future was executed.
pub async fn execute_with_retry_after_admission_observed<T, F, Fut, A, AFut, O>(
    mut f: F,
    mut admit_retry: A,
    mut observe_attempt: O,
    policy: &RetryPolicy,
) -> Result<T, GatewayError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, GatewayError>>,
    A: FnMut() -> AFut,
    AFut: std::future::Future<Output = Result<(), GatewayError>>,
    O: for<'a> FnMut(RetryAttemptObservation<'a>),
{
    let mut last_error: Option<GatewayError> = None;

    for attempt in 0..=policy.max_retries {
        if attempt > 0 {
            admit_retry().await?;
        }
        let attempt_started_at = Instant::now();
        let result = f().await;
        observe_attempt(RetryAttemptObservation {
            succeeded: result.is_ok(),
            latency_ms: u64::try_from(attempt_started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
            error: result.as_ref().err(),
        });
        match result {
            Ok(result) => return Ok(result),
            Err(error) => {
                if attempt < policy.max_retries && should_retry(&error, policy) {
                    let delay = compute_delay(attempt, policy);
                    warn!(
                        attempt,
                        delay_ms = delay.as_millis() as u64,
                        kind = ?error.kind,
                        provider = ?error.provider_name,
                        "upstream call failed; retrying"
                    );
                    last_error = Some(error);
                    sleep(delay).await;
                } else {
                    return Err(error);
                }
            }
        }
    }

    Err(last_error.expect("retry loop exited without a result"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorKind, FallbackHint};

    // ── helpers ──────────────────────────────────────────────────────────

    fn make_error(kind: ErrorKind, retryable: bool, http_status: Option<u16>) -> GatewayError {
        GatewayError {
            kind,
            message: "test".to_string(),
            code: None,
            http_status,
            retryable,
            fallback_hint: FallbackHint::Abort {
                reason: "test".to_string(),
            },
            provider_name: None,
        }
    }

    fn retryable_server_error() -> GatewayError {
        make_error(ErrorKind::ServerError, true, Some(500))
    }

    fn non_retryable_auth_error() -> GatewayError {
        make_error(ErrorKind::Authentication, false, Some(401))
    }

    fn retryable_network_error() -> GatewayError {
        // Network errors have no HTTP status.
        make_error(ErrorKind::Network, true, None)
    }

    // ── should_retry ─────────────────────────────────────────────────────

    #[test]
    fn should_retry_returns_true_for_retryable_status() {
        let policy = RetryPolicy::default();
        let e = retryable_server_error();
        assert!(should_retry(&e, &policy));
    }

    #[test]
    fn should_retry_returns_false_when_retryable_flag_is_false() {
        let policy = RetryPolicy::default();
        let e = non_retryable_auth_error();
        assert!(!should_retry(&e, &policy));
    }

    #[test]
    fn should_retry_returns_false_for_client_error_status_even_if_retryable_flag_set() {
        let policy = RetryPolicy::default();
        // 400 is a client error — should never be retried.
        let e = make_error(ErrorKind::BadRequest, true, Some(400));
        assert!(!should_retry(&e, &policy));
    }

    #[test]
    fn should_retry_returns_false_when_status_not_in_policy() {
        let policy = RetryPolicy {
            retryable_statuses: vec![429],
            ..Default::default()
        };
        // 500 is retryable in the error, but NOT in our custom policy.
        let e = make_error(ErrorKind::ServerError, true, Some(500));
        assert!(!should_retry(&e, &policy));
    }

    #[test]
    fn should_retry_returns_true_for_network_error_without_status() {
        let policy = RetryPolicy::default();
        let e = retryable_network_error();
        assert!(should_retry(&e, &policy));
    }

    #[test]
    fn should_retry_returns_false_for_429_with_policy_that_excludes_it() {
        let policy = RetryPolicy {
            retryable_statuses: vec![500],
            ..Default::default()
        };
        let e = make_error(ErrorKind::RateLimit, true, Some(429));
        assert!(!should_retry(&e, &policy));
    }

    // ── compute_delay ────────────────────────────────────────────────────

    #[test]
    fn compute_delay_is_within_jitter_band() {
        let policy = RetryPolicy {
            initial_delay: Duration::from_millis(1_000),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(10),
            ..Default::default()
        };
        // Attempt 0: base = 1000ms — expect [900, 1100)
        for _ in 0..50 {
            let d = compute_delay(0, &policy);
            assert!(
                d >= Duration::from_millis(900) && d < Duration::from_millis(1_100),
                "attempt 0 delay out of jitter band: {d:?}"
            );
        }
    }

    #[test]
    fn compute_delay_grows_exponentially() {
        let policy = RetryPolicy {
            initial_delay: Duration::from_millis(500),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(60),
            ..Default::default()
        };
        // Without jitter: attempt 0 = 500ms, attempt 1 = 1000ms, attempt 2 = 2000ms.
        // With ±10 % jitter the values should still be strictly increasing on average.
        // We run many trials and assert the median ordering holds.
        let mut wins = 0u32;
        for _ in 0..200 {
            let d0 = compute_delay(0, &policy);
            let d1 = compute_delay(1, &policy);
            if d1 > d0 {
                wins += 1;
            }
        }
        // We expect d1 > d0 the vast majority of the time (base ratio 2x, jitter ±10%).
        assert!(
            wins > 150,
            "exponential growth not observed consistently ({wins}/200)"
        );
    }

    #[test]
    fn compute_delay_is_capped_at_max_delay() {
        let policy = RetryPolicy {
            initial_delay: Duration::from_secs(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            ..Default::default()
        };
        // Even attempt 0 would normally be 100 s — must be capped to ~5 s (±10%).
        for _ in 0..20 {
            let d = compute_delay(0, &policy);
            assert!(
                d <= Duration::from_millis(5_500),
                "delay exceeded max_delay + jitter: {d:?}"
            );
        }
    }

    // ── execute_with_retry ───────────────────────────────────────────────

    #[tokio::test]
    async fn execute_with_retry_returns_ok_immediately() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_millis(1),
            ..Default::default()
        };
        let mut calls = 0usize;
        let result = execute_with_retry(
            || {
                calls += 1;
                async { Ok::<_, GatewayError>(42u32) }
            },
            &policy,
        )
        .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls, 1, "should succeed on first call");
    }

    #[tokio::test]
    async fn execute_with_retry_retries_on_transient_error_then_succeeds() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            ..Default::default()
        };

        let mut calls = 0usize;
        let result = execute_with_retry(
            || {
                calls += 1;
                let attempt = calls;
                async move {
                    if attempt < 3 {
                        Err(retryable_server_error())
                    } else {
                        Ok(99u32)
                    }
                }
            },
            &policy,
        )
        .await;

        assert_eq!(result.unwrap(), 99);
        assert_eq!(calls, 3);
    }

    #[tokio::test]
    async fn execute_with_retry_does_not_retry_non_retryable_error() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_millis(1),
            ..Default::default()
        };

        let mut calls = 0usize;
        let result = execute_with_retry(
            || {
                calls += 1;
                async { Err::<u32, _>(non_retryable_auth_error()) }
            },
            &policy,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(
            calls, 1,
            "non-retryable error should not cause additional attempts"
        );
    }

    #[tokio::test]
    async fn execute_with_retry_exhausts_max_retries_and_returns_last_error() {
        let policy = RetryPolicy {
            max_retries: 2,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            ..Default::default()
        };

        let mut calls = 0usize;
        let result = execute_with_retry(
            || {
                calls += 1;
                async { Err::<u32, _>(retryable_server_error()) }
            },
            &policy,
        )
        .await;

        assert!(result.is_err());
        // 1 initial attempt + 2 retries = 3 total calls.
        assert_eq!(calls, 3);
    }

    #[tokio::test]
    async fn retry_admission_runs_before_each_retry_but_not_before_the_pre_admitted_first_call() {
        let policy = RetryPolicy {
            max_retries: 2,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
            ..Default::default()
        };
        let mut calls = 0usize;
        let mut retry_admissions = 0usize;

        let result = execute_with_retry_after_admission(
            || {
                calls += 1;
                let attempt = calls;
                async move {
                    if attempt < 3 {
                        Err(retryable_server_error())
                    } else {
                        Ok(42u32)
                    }
                }
            },
            || {
                retry_admissions += 1;
                async { Ok(()) }
            },
            &policy,
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls, 3);
        assert_eq!(retry_admissions, 2);
    }

    #[tokio::test]
    async fn retry_admission_rejection_returns_the_fail_closed_admission_error() {
        let policy = RetryPolicy {
            max_retries: 2,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
            ..Default::default()
        };
        let mut calls = 0usize;

        let error = execute_with_retry_after_admission(
            || {
                calls += 1;
                async { Err::<u32, _>(retryable_server_error()) }
            },
            || async {
                Err(GatewayError::service_unavailable(
                    "rate-limit admission result is indeterminate",
                )
                .with_code("rate_limit_admission_indeterminate"))
            },
            &policy,
        )
        .await
        .expect_err("retry admission must stop the retry loop");

        assert_eq!(calls, 1);
        assert_eq!(error.http_status, Some(503));
        assert_eq!(error.kind, ErrorKind::ServiceUnavailable);
        assert_eq!(
            error.code.as_deref(),
            Some("rate_limit_admission_indeterminate")
        );
    }

    #[tokio::test]
    async fn observed_retry_reports_every_real_upstream_attempt_and_elapsed_time() {
        let policy = RetryPolicy {
            max_retries: 2,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
            ..Default::default()
        };
        let mut calls = 0usize;
        let mut observations = Vec::new();

        let result = execute_with_retry_after_admission_observed(
            || {
                calls += 1;
                let attempt = calls;
                async move {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                    if attempt < 3 {
                        Err(retryable_server_error())
                    } else {
                        Ok(42u32)
                    }
                }
            },
            || async { Ok(()) },
            |observation| {
                observations.push((
                    observation.succeeded,
                    observation.error.and_then(|error| error.http_status),
                    observation.latency_ms,
                ));
            },
            &policy,
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls, 3);
        assert_eq!(observations.len(), 3);
        assert_eq!(
            observations
                .iter()
                .map(|(succeeded, status, _)| (*succeeded, *status))
                .collect::<Vec<_>>(),
            vec![(false, Some(500)), (false, Some(500)), (true, None)]
        );
        assert!(
            observations
                .iter()
                .all(|(_, _, latency_ms)| *latency_ms > 0),
            "attempt latency must come from the real upstream future"
        );
    }

    // ── RetryPolicy default ──────────────────────────────────────────────

    #[test]
    fn default_policy_has_expected_values() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_retries, 2);
        assert_eq!(p.initial_delay, Duration::from_millis(500));
        assert_eq!(p.max_delay, Duration::from_secs(5));
        assert!((p.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(p.retryable_statuses, vec![429, 500, 502, 503, 504]);
    }
}
