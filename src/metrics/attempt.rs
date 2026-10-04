//! Cancellation-only handoff guard for direct attempts outside the observed retry loop.
use super::request::{bounded_label, GatewayMetrics, ProviderMetricOutcome};
use std::time::Instant;

pub(crate) struct ProviderAttemptCancellation<'a> {
    metrics: &'a GatewayMetrics,
    provider: String,
    model: String,
    started_at: Instant,
    armed: bool,
}

impl<'a> ProviderAttemptCancellation<'a> {
    pub fn new(
        metrics: &'a GatewayMetrics,
        provider: &str,
        model: &str,
        started_at: Instant,
    ) -> Self {
        Self {
            metrics,
            provider: bounded_label(provider, "unknown_provider"),
            model: bounded_label(model, "unknown_model"),
            started_at,
            armed: true,
        }
    }

    /// A completed send/error or the stream terminal owner now accounts for this attempt.
    pub fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for ProviderAttemptCancellation<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.metrics
                .observe_provider_outcome(ProviderMetricOutcome {
                    provider: &self.provider,
                    model: Some(&self.model),
                    success: false,
                    latency_ms: u64::try_from(self.started_at.elapsed().as_millis())
                        .unwrap_or(u64::MAX),
                    failure_class: Some("cancelled"),
                });
        }
    }
}
