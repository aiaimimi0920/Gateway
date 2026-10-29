//! Waiting changes policy once from the original clock. Explicit no-work refusals retain audit counts.
use super::*;

impl RequestBudget {
    pub(crate) fn configure_wait(
        &self,
        req: &CanonicalRelayRequest,
        seconds: Option<i32>,
        long: bool,
        enabled: bool,
    ) -> Result<(), GatewayError> {
        let initial = !self.0.lock().policy_configured;
        self.configure(req, seconds)?;
        let mut state = self.0.lock();
        if initial && long && seconds.is_none() {
            state.deadline = None;
        }
        state.wait_managed = enabled;
        Ok(())
    }

    /// One next slot only for a completed, typed capacity refusal, including tool declarations.
    /// No decrement/reset of actual sends; unknown execution and post-handoff errors stay closed.
    pub(crate) fn allow_capacity_readmission(&self, error: &GatewayError) {
        if error.kind != crate::error::ErrorKind::ModelCapacity
            || error.request_budget_stop_reason().is_some()
        {
            return;
        }
        let mut state = self.0.lock();
        if state.wait_managed
            && state.stop.is_none()
            && !state.handed_off
            && state.attempts > state.capacity_readmission_at
        {
            state.capacity_readmission_at = state.attempts;
            state.max_attempts = state.max_attempts.saturating_add(1);
        }
    }

    pub(crate) fn cancel(&self) -> GatewayError {
        self.stop("cancelled")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::responses::normalize_responses;
    #[test]
    fn capacity_readmission_is_exactly_once_per_completed_refusal_and_never_for_unknown_delivery() {
        let req = normalize_responses(serde_json::json!({"input":"hello", "tools":[{"type":"function","name":"fixture","parameters":{"type":"object"}}]})).unwrap();
        let budget = RequestBudget::for_request(&req);
        budget.configure_wait(&req, None, true, true).unwrap();
        let refused = crate::error::classify_upstream_error(
            429,
            r#"{"error":{"code":"turn_rate_limit_exceeded","message":"Codex is at capacity, please try again later"}}"#,
            None,
        );
        for _ in 0..8 {
            budget.begin_attempt().unwrap();
            budget.allow_capacity_readmission(&refused);
            budget.allow_capacity_readmission(&refused);
            assert_eq!(budget.remaining_attempts(), 1);
        }
        budget.begin_attempt().unwrap();
        budget.allow_capacity_readmission(&GatewayError::service_unavailable("unknown execution"));
        assert_eq!(budget.remaining_attempts(), 0);
        assert_eq!(budget.attempts(), 9);
    }
    #[tokio::test(start_paused = true)]
    async fn long_wait_outlives_fifteen_minutes_and_never_revives_cancelled_owner() {
        let req =
            normalize_responses(serde_json::json!({"model":"fixture", "input":"hello"})).unwrap();
        let budget = RequestBudget::for_request(&req);
        budget.configure_wait(&req, None, true, true).unwrap();
        tokio::time::advance(Duration::from_secs(1800)).await;
        assert!(budget.check().is_ok());
        let error = budget.cancel();
        assert_eq!(error.request_budget_stop_reason(), Some("cancelled"));
        budget.configure_wait(&req, None, true, true).unwrap();
        assert!(budget.check().is_err());
    }
    #[tokio::test(start_paused = true)]
    async fn explicit_trusted_deadline_still_bounds_long_wait_and_stream() {
        let req =
            normalize_responses(serde_json::json!({"model":"fixture", "input":"hello"})).unwrap();
        let budget = RequestBudget::for_request(&req);
        budget.configure_wait(&req, Some(600), true, true).unwrap();
        tokio::time::advance(Duration::from_secs(601)).await;
        assert!(budget.check().is_err());
    }
}
