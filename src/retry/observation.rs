//! Observation follows the real future, including cancellation while it is pending.
use super::RetryAttemptObservation;
use crate::error::GatewayError;
use std::time::Instant;

pub(super) struct AttemptObservation<'a, O: for<'e> FnMut(RetryAttemptObservation<'e>)> {
    observer: Option<&'a mut O>,
    started_at: Instant,
    is_retry: bool,
}

impl<'a, O: for<'e> FnMut(RetryAttemptObservation<'e>)> AttemptObservation<'a, O> {
    pub fn new(observer: &'a mut O, is_retry: bool) -> Self {
        Self {
            observer: Some(observer),
            started_at: Instant::now(),
            is_retry,
        }
    }

    pub fn complete<T>(mut self, result: &Result<T, GatewayError>) {
        if let Some(observer) = self.observer.take() {
            observer(RetryAttemptObservation {
                succeeded: result.is_ok(),
                latency_ms: self.elapsed(),
                error: result.as_ref().err(),
                is_retry: self.is_retry,
            });
        }
    }

    fn elapsed(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

impl<O: for<'e> FnMut(RetryAttemptObservation<'e>)> Drop for AttemptObservation<'_, O> {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.take() {
            // Synthetic observation only; it never replaces a returned upstream error.
            let error = GatewayError::server_error("upstream attempt cancelled")
                .with_code("attempt_cancelled");
            observer(RetryAttemptObservation {
                succeeded: false,
                latency_ms: self.elapsed(),
                error: Some(&error),
                is_retry: self.is_retry,
            });
        }
    }
}
