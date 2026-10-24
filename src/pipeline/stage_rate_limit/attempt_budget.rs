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
        if let Some(circuit) = &self.circuit {
            circuit.begin()?;
        }
        if let Err(error) = self.request_budget.begin_attempt() {
            self.finish_neutral();
            return Err(error);
        }
        self.smart_routing.begin_request(
            self.sticky_scope.as_deref(),
            &self.requested_model,
            &self.provider_account_id,
            self.req_id,
        );
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
        self.finish_circuit(crate::routing::model_circuit::failure_outcome(error));
        self.request_budget.observe_error(error);
        if provider_failed(error) {
            self.smart_routing
                .outcome(&self.provider_account_id, &self.requested_model, false);
        }
    }

    pub(crate) fn finish_error(&self, error: &GatewayError) {
        self.finish_circuit(crate::routing::model_circuit::failure_outcome(error));
        if provider_failed(error) {
            self.release_failed_sticky();
        }
    }

    pub(crate) fn finish_success(&self) {
        self.finish_circuit(crate::routing::model_circuit::Outcome::Success);
    }
    pub(crate) fn finish_neutral(&self) {
        self.finish_circuit(crate::routing::model_circuit::Outcome::Neutral);
    }
    pub(crate) fn finish_stream_failure(&self) {
        self.finish_circuit(crate::routing::model_circuit::Outcome::Failure(0));
    }
    fn finish_circuit(&self, outcome: crate::routing::model_circuit::Outcome) {
        if let Some(circuit) = &self.circuit {
            circuit.finish(outcome);
        }
    }

    pub(crate) fn release_failed_sticky(&self) {
        self.smart_routing.release_failed_request(
            self.sticky_scope.as_deref(),
            &self.requested_model,
            &self.provider_account_id,
            self.req_id,
        );
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

fn provider_failed(error: &GatewayError) -> bool {
    error.request_budget_stop_reason().is_none()
        && !matches!(
            error.code.as_deref(),
            Some(
                "attempt_cancelled"
                    | "upstream_credential_unavailable"
                    | "rate_limit_admission_indeterminate"
                    | "model_circuit_open"
                    | "site_health_route_restricted"
            )
        )
        && !matches!(
            error.kind,
            crate::error::ErrorKind::BadRequest
                | crate::error::ErrorKind::ContextLength
                | crate::error::ErrorKind::ContentFilter
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> ProviderAttemptGate {
        ProviderAttemptGate {
            req_id: uuid::Uuid::new_v4(),
            redis_pool: crate::redis::pool::disabled_pool().unwrap(),
            local_limits: None,
            rules: Vec::new(),
            outbound_attempt_count: Default::default(),
            attempted_provider_ids: Default::default(),
            provider_account_id: "diagnostic-provider".into(),
            request_budget: crate::pipeline::request_budget::RequestBudget::new(
                6,
                std::time::Duration::from_secs(30),
            ),
            smart_routing: Default::default(),
            sticky_scope: Some("diagnostic-session".into()),
            requested_model: "fixture-model".into(),
            eligibility: None,
            circuit: None,
        }
    }

    #[tokio::test]
    async fn diagnostic_html_failure_is_scored_once_and_releases_sticky() {
        let gate = gate();
        let result: Result<(), GatewayError> = gate
            .execute_observed(
                || async {
                    Err(crate::error::classify_upstream_error(
                        403,
                        "<html><title>系统终端 - 异常状态诊断</title></html>",
                        None,
                    ))
                },
                |_| {},
                &RetryPolicy::default(),
            )
            .await;
        let error = result.unwrap_err();
        assert_eq!(
            gate.request_budget.attempts(),
            1,
            "never retry the rejected credential"
        );
        let score = gate
            .smart_routing
            .score("diagnostic-provider", "fixture-model");
        assert!(
            (85.0..85.1).contains(&score),
            "expected exactly one -15 penalty: {score}"
        );
        assert_eq!(
            gate.smart_routing.score("other-provider", "fixture-model"),
            100.0
        );
        assert_eq!(
            gate.smart_routing
                .score("diagnostic-provider", "other-model"),
            100.0
        );
        gate.finish_error(&error);
        assert!(gate
            .smart_routing
            .sticky(Some("diagnostic-session"), "fixture-model")
            .is_none());
        assert!(
            (85.0..85.1).contains(
                &gate
                    .smart_routing
                    .score("diagnostic-provider", "fixture-model")
            ),
            "finishing failure must not deduct twice"
        );
    }

    #[test]
    fn request_side_errors_do_not_reduce_availability() {
        let gate = gate();
        for error in [
            GatewayError::bad_request("invalid parameters"),
            crate::error::classify_upstream_error(400, "maximum context length", None),
            GatewayError::content_filtered("policy violation"),
            GatewayError::bad_request("cancelled").with_code("attempt_cancelled"),
        ] {
            gate.observe_error(&error);
        }
        assert_eq!(
            gate.smart_routing
                .score("diagnostic-provider", "fixture-model"),
            100.0
        );
    }
}
