//! A failed connector sent no origin HTTP request; it is not an execution replay.
use super::*;

impl RequestBudget {
    pub(crate) fn allow_connect_readmission(&self, error: &GatewayError) -> bool {
        if !error.native_connect_retry_allowed() {
            return false;
        }
        let mut state = self.0.lock();
        if state.stop.is_some()
            || state.handed_off
            || state.attempts <= state.connect_readmission_at
            || state.connect_readmissions >= 4
            || state.deadline.is_some_and(|at| Instant::now() >= at)
        {
            return false;
        }
        state.connect_readmission_at = state.attempts;
        state.connect_readmissions += 1;
        // Never reset actual attempts or widen the normal execution replay policy.
        state.max_attempts = state.max_attempts.max(state.attempts.saturating_add(1));
        true
    }
}
