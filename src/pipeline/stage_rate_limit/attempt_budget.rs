//! Only the send-start seam consumes an admitted request's shared attempt slot.
use super::ProviderAttemptGate;
use crate::error::GatewayError;
use crate::retry::{
    execute_with_retry_after_admission_observed_started, RetryAttemptObservation, RetryPolicy,
};

impl ProviderAttemptGate {
    pub fn check_remaining(&self) -> Result<(), GatewayError> {
        self.request_budget.check()
    }

    pub fn begin_attempt(&self) -> Result<(), GatewayError> {
        self.request_budget.begin_attempt()?;
        self.outbound_attempt_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut attempted = self.attempted_provider_ids.lock();
        if !attempted
            .iter()
            .any(|value| value == &self.provider_account_id)
        {
            attempted.push(self.provider_account_id.clone());
        }
        Ok(())
    }

    pub fn observe_error(&self, error: &GatewayError) {
        self.request_budget.observe_error(error);
    }

    pub fn observe_result<T>(&self, result: &Result<T, GatewayError>) {
        if let Err(error) = result {
            self.observe_error(error);
        }
    }

    pub async fn execute_observed<T, F, Fut, O>(
        &self,
        f: F,
        mut observe: O,
        policy: &RetryPolicy,
    ) -> Result<T, GatewayError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, GatewayError>>,
        O: for<'a> FnMut(RetryAttemptObservation<'a>),
    {
        let mut policy = policy.clone();
        policy.max_retries = policy
            .max_retries
            .min(self.request_budget.remaining_attempts().saturating_sub(1) as usize);
        execute_with_retry_after_admission_observed_started(
            f,
            || self.admit(),
            || self.begin_attempt(),
            |observation| {
                if let Some(error) = observation.error {
                    // Synthetic cancellation belongs to metrics, not the last real failure.
                    if error.code.as_deref() != Some("attempt_cancelled") {
                        self.observe_error(error);
                    }
                }
                observe(observation);
            },
            &policy,
        )
        .await
    }
}
