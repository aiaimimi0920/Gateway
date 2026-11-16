//! Cancellation is a signal inside the paid owner, not an outer select that discards its ledger.
use super::*;
pub(super) struct Control {
    pub budget: crate::pipeline::request_budget::RequestBudget,
    caller: Option<tokio::sync::watch::Receiver<bool>>,
    state: Arc<AppState>,
    admitted: Option<crate::pipeline::request_budget::AdmittedStream>,
}
impl Control {
    pub fn new(ctx: &PipelineContext, state: &Arc<AppState>) -> Self {
        Self {
            budget: ctx.request_budget.clone(),
            caller: ctx.caller_cancel.clone(),
            state: state.clone(),
            admitted: None,
        }
    }
    pub fn admitted(mut self) -> Result<Self, GatewayError> {
        self.admitted = Some(self.budget.admitted_stream()?);
        Ok(self)
    }
    pub fn check(&self) -> Result<(), GatewayError> {
        if self.state.shutdown.is_requested() {
            return Err(self.budget.cancel_shutdown());
        }
        if self
            .caller
            .as_ref()
            .is_some_and(|cancel| *cancel.borrow() || cancel.has_changed().is_err())
        {
            return Err(self.budget.cancel_caller());
        }
        match &self.admitted {
            Some(admitted) => admitted.check(),
            None => self.budget.check_inflight(),
        }
    }
    pub fn remaining(&self) -> Result<Duration, GatewayError> {
        self.check()?;
        // Attempt slots control new paid sends, not later stages of the one admitted operation.
        // A tool-bearing request has a single slot and must still reach its original START/FINISH.
        Ok(self
            .budget
            .admission_deadline()
            .map(|at| at.saturating_duration_since(tokio::time::Instant::now()))
            .unwrap_or(Duration::from_secs(300))
            .min(Duration::from_secs(300)))
    }
    pub async fn cancelled(&self) {
        let mut caller = self.caller.clone();
        tokio::select! {
            biased;
            _ = self.state.shutdown.wait() => { self.budget.cancel_shutdown(); },
            _ = async {
                match caller.as_mut() {
                    Some(caller) => while !*caller.borrow_and_update() {
                        if caller.changed().await.is_err() { break; }
                    },
                    None => std::future::pending().await,
                }
            } => { self.budget.cancel_caller(); },
            _ = async { match self.budget.admission_deadline() {
                Some(at) => tokio::time::sleep_until(at).await,
                None => std::future::pending().await,
            }} => { let _ = self.check(); },
        }
    }
    pub fn business(&self, error: BusinessError) -> GatewayError {
        let mut error = business(error);
        if let Err(stopped) = self.check() {
            // Retain the safe accounting/run failure code AND the first request stop cause, so a
            // caller cancellation with unknown cost does not become a credential-health failure.
            error.fallback_hint = stopped.fallback_hint;
            error.retryable = false;
        }
        error
    }
}
