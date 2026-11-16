//! A confirmed paid owner can finish cooperatively at deadline, never spawn or renew generation.
use super::*;

// Reuse the catalog run transport's existing 20-second maximum. This is cleanup only, not a new
// request deadline; check/begin_attempt still reject expired requests while the guard is held.
pub(crate) const CLOSEOUT_TIMEOUT: Duration = Duration::from_secs(20);
pub(crate) struct DeadlineCloseoutGuard(RequestBudget);
impl RequestBudget {
    pub(crate) fn retain_deadline_closeout(&self) -> DeadlineCloseoutGuard {
        let mut state = self.0.lock();
        assert!(
            !state.deadline_closeout,
            "one request owns one paid closeout"
        );
        state.deadline_closeout = true;
        DeadlineCloseoutGuard(self.clone())
    }
    pub(super) async fn closeout_after_deadline<T>(
        &self,
        future: impl Future<Output = Result<T, GatewayError>>,
    ) -> Result<T, GatewayError> {
        let stopped = self.stop("deadline");
        let retained = self.0.lock().deadline_closeout;
        if retained {
            // This polls the ORIGINAL dispatch. Dropping/aborting the caller still drops it; no
            // detached compensation survives, and an unresponsive finalizer is bounded here too.
            if let Ok(Err(mut error)) = tokio::time::timeout(CLOSEOUT_TIMEOUT, future).await {
                error.retryable = false;
                error.fallback_hint = stopped.fallback_hint;
                return Err(error);
            }
        }
        Err(stopped)
    }
}
impl Drop for DeadlineCloseoutGuard {
    fn drop(&mut self) {
        self.0 .0.lock().deadline_closeout = false;
    }
}
#[cfg(test)]
#[path = "request_budget_closeout_tests.rs"]
mod tests;
