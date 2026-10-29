//! In-process recovery waiting. This is not a durable job queue or an execution retry owner.
#[cfg(test)]
mod oauth_tests;
mod policy;
mod queue;
pub(crate) mod transport;
pub use policy::{RequestWaitMode, RequestWaitPolicy};
pub(crate) use queue::WaitRegistry;

use crate::error::{ErrorKind, FallbackHint, GatewayError};
use std::time::Duration;

/// Only refusals known to precede generation may be replayed. A bare 503/timeout is insufficient.
pub(crate) fn recoverable(error: &GatewayError) -> bool {
    error.request_budget_stop_reason().is_none()
        && (error.kind == ErrorKind::ModelCapacity
            || matches!(
                error.code.as_deref(),
                Some(
                    "model_circuit_open"
                        | "gateway_network_unstable"
                        | "upstream_credential_cooling"
                )
            ))
}

pub(crate) fn delay(error: &GatewayError, round: u32) -> Duration {
    let local = Duration::from_secs([2, 4, 8, 15, 30, 60][round.min(5) as usize]);
    let hint = match error.fallback_hint {
        FallbackHint::Retry { delay_ms, .. } => delay_ms,
        _ => 0,
    }
    .max(error.upstream_retry_after_ms.unwrap_or(0));
    local.max(Duration::from_millis(hint))
}

/// Preserve huge upstream lower bounds without overflowing the platform's Instant.
/// Chunking never re-admits a request early; the enclosing budget/caller owns cancellation.
pub(crate) async fn sleep_delay(mut delay: Duration) {
    while !delay.is_zero() {
        let chunk = delay.min(Duration::from_secs(3600));
        tokio::time::sleep(chunk).await;
        delay = delay.saturating_sub(chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_execution_and_permanent_errors_never_enter_waiting() {
        for error in [
            GatewayError::service_unavailable("unknown 503"),
            GatewayError::bad_request("bad input"),
            GatewayError::service_unavailable("disabled")
                .with_code("upstream_credential_unavailable"),
        ] {
            assert!(!recoverable(&error));
        }
        assert!(recoverable(
            &GatewayError::service_unavailable("cooling").with_code("model_circuit_open")
        ));
    }
    #[test]
    fn excessive_retry_after_is_never_shortened() {
        let mut error = GatewayError::service_unavailable("fixture");
        error.upstream_retry_after_ms = Some(900_000);
        assert_eq!(delay(&error, 100), Duration::from_secs(900));
        error.upstream_retry_after_ms = None;
        assert_eq!(delay(&error, 100), Duration::from_secs(60));
    }

    #[tokio::test(start_paused = true)]
    async fn huge_wait_hint_is_cancellable_and_cannot_overflow_a_timer() {
        let task = tokio::spawn(sleep_delay(Duration::from_millis(u64::MAX)));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(7200)).await;
        assert!(!task.is_finished());
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
