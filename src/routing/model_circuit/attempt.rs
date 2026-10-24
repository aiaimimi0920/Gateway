//! One shared lease per concrete send; cloned gates do not duplicate feedback or lose cancellation cleanup.
use super::{CircuitKey, ModelCircuits, Outcome, Ticket};
use crate::error::{FallbackHint, GatewayError};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct CircuitAttempt {
    runtime: Arc<ModelCircuits>,
    keys: Vec<CircuitKey>,
    active: Mutex<Option<Vec<Ticket>>>,
}
impl CircuitAttempt {
    pub(crate) fn new(runtime: Arc<ModelCircuits>, mut keys: Vec<CircuitKey>) -> Arc<Self> {
        keys.sort_by(|a, b| {
            (&a.provider, &a.group, &a.model).cmp(&(&b.provider, &b.group, &b.model))
        });
        keys.dedup();
        Arc::new(Self {
            runtime,
            keys,
            active: Mutex::new(None),
        })
    }
    pub(crate) fn begin(&self) -> Result<(), GatewayError> {
        let mut active = self.active.lock();
        if let Some(tickets) = active.take() {
            self.runtime
                .finish_at(tickets, Outcome::Neutral, self.runtime.now());
        }
        let tickets = self
            .runtime
            .acquire_at(&self.keys, self.runtime.now())
            .map_err(|_| {
                let mut error = GatewayError::service_unavailable(
                    "Model is temporarily disabled by failure circuit",
                )
                .with_code("model_circuit_open");
                error.fallback_hint = FallbackHint::FallbackProvider {
                    reason: "model_circuit_open".into(),
                };
                error
            })?;
        *active = Some(tickets);
        Ok(())
    }
    pub(crate) fn finish(&self, outcome: Outcome) {
        if let Some(tickets) = self.active.lock().take() {
            self.runtime.finish_at(tickets, outcome, self.runtime.now());
        }
    }
}
impl Drop for CircuitAttempt {
    fn drop(&mut self) {
        self.finish(Outcome::Neutral);
    }
}
