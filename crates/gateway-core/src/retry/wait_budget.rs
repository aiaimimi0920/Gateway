//! Per-loop sleep admission. Request-wide attempt/deadline accounting is separate.
use super::{compute_delay, RetryPolicy};
use crate::error::{FallbackHint, GatewayError};
use std::time::Duration;

pub(super) struct WaitBudget {
    remaining: Duration,
}

impl WaitBudget {
    pub(super) fn new(limit: Duration) -> Self {
        Self { remaining: limit }
    }

    pub(super) fn next_delay(
        &mut self,
        error: &GatewayError,
        attempt: usize,
        policy: &RetryPolicy,
    ) -> Option<Duration> {
        let delay = match error.fallback_hint {
            FallbackHint::Retry { delay_ms, .. } => Duration::from_millis(delay_ms),
            _ => compute_delay(attempt, policy),
        };
        // Never clip an upstream minimum to fit the remaining budget. No jitter
        // is applied to a hint; only the existing local backoff uses jitter.
        self.remaining = self.remaining.checked_sub(delay)?;
        Some(delay)
    }
}
