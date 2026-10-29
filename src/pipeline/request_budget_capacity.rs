//! A single capacity retry permit belongs to the logical request, not each candidate/recovery loop.
use super::*;
use crate::error::ErrorKind;

impl RequestBudget {
    pub(crate) fn claim_capacity_retry(&self, error: &GatewayError) -> bool {
        if error.kind != ErrorKind::ModelCapacity || !error.retryable {
            return false;
        }
        let FallbackHint::Retry { delay_ms, .. } = error.fallback_hint else {
            return false;
        };
        let mut state = self.0.lock();
        if state.wait_managed
            || !state.capacity_retry_available
            || state.stop.is_some()
            || state.handed_off
            || state.attempts >= state.max_attempts
            || !Instant::now()
                .checked_add(Duration::from_millis(delay_ms))
                .is_some_and(|after_wait| {
                    state.deadline.is_none_or(|deadline| after_wait < deadline)
                })
        {
            return false;
        }
        state.capacity_retry_available = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn rejected_tool_fallback_does_not_grant_same_account_capacity_replay() {
        let req: CanonicalRelayRequest = serde_json::from_value(serde_json::json!({
            "protocol_family":"open_ai", "endpoint_kind":"responses", "stream":true,
            "messages":[], "tools":[{"type":"function","name":"fixture","raw":{}}], "raw_body":{}
        }))
        .unwrap();
        let budget = RequestBudget::for_request(&req);
        budget.begin_attempt().unwrap();
        let rejection = crate::error::classify_upstream_error(401, "Unauthorized", None);
        budget.allow_rejected_candidate_fallback(&rejection);
        assert_eq!(budget.remaining_attempts(), 1);
        let capacity = crate::error::classify_upstream_error(
            429,
            r#"{"error":{"code":"turn_rate_limit_exceeded","message":"Codex is at capacity, please try again later"}}"#,
            None,
        );
        assert!(!budget.claim_capacity_retry(&capacity));
    }
}
