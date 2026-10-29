//! One shared lease per concrete send; cloned gates do not duplicate feedback or lose cancellation cleanup.
use super::{CircuitKey, ModelCircuits, Outcome, Ticket};
use crate::error::{FallbackHint, GatewayError};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct CircuitAttempt {
    runtime: Arc<ModelCircuits>,
    keys: Vec<CircuitKey>,
    capacity_key: Option<CircuitKey>,
    active: Mutex<Option<Active>>,
}
#[derive(Debug)]
struct Active {
    hard: Vec<Ticket>,
    capacity: Option<super::capacity::CapacityTicket>,
}
impl CircuitAttempt {
    #[cfg(test)]
    pub(crate) fn new(runtime: Arc<ModelCircuits>, keys: Vec<CircuitKey>) -> Arc<Self> {
        Self::scoped(runtime, keys, None)
    }
    pub(crate) fn scoped(
        runtime: Arc<ModelCircuits>,
        mut keys: Vec<CircuitKey>,
        capacity_key: Option<CircuitKey>,
    ) -> Arc<Self> {
        keys.sort_by(|a, b| {
            (&a.provider, &a.group, &a.model).cmp(&(&b.provider, &b.group, &b.model))
        });
        keys.dedup();
        Arc::new(Self {
            runtime,
            keys,
            capacity_key,
            active: Mutex::new(None),
        })
    }
    pub(crate) fn begin(&self) -> Result<(), GatewayError> {
        if let Some(error) = self.runtime.network_error() {
            return Err(error);
        }
        let mut active = self.active.lock();
        if let Some(previous) = active.take() {
            self.complete(previous, Outcome::Neutral);
        }
        let capacity = self
            .capacity_key
            .as_ref()
            .map(|key| {
                self.runtime
                    .capacity
                    .lock()
                    .acquire(key, self.runtime.now())
            })
            .transpose()?;
        let tickets = self
            .runtime
            .acquire_at(&self.keys, self.runtime.now())
            .map_err(|_| {
                if let Some(error) = self.runtime.network_error() {
                    return error;
                }
                let mut error = GatewayError::service_unavailable(
                    "Model is temporarily disabled by failure circuit",
                )
                .with_code("model_circuit_open");
                error.fallback_hint = FallbackHint::FallbackProvider {
                    reason: "model_circuit_open".into(),
                };
                error
            });
        let tickets = match tickets {
            Ok(tickets) => tickets,
            Err(error) => {
                if let Some(ticket) = capacity {
                    self.runtime.capacity.lock().finish(ticket, false);
                }
                return Err(error);
            }
        };
        *active = Some(Active {
            hard: tickets,
            capacity,
        });
        Ok(())
    }
    pub(crate) fn finish(&self, outcome: Outcome) {
        if let Some(active) = self.active.lock().take() {
            self.complete(active, outcome);
        }
    }
    pub(crate) fn finish_error(&self, error: &GatewayError) -> GatewayError {
        let Some(active) = self.active.lock().take() else {
            return error.clone();
        };
        let outcome = super::failure_outcome(error);
        self.runtime
            .finish_at(active.hard, outcome, self.runtime.now());
        if let Some(ticket) = active.capacity {
            let mut capacity = self.runtime.capacity.lock();
            if error.kind == crate::error::ErrorKind::ModelCapacity
                && error.request_budget_stop_reason().is_none()
            {
                return capacity.failure(ticket, error, self.runtime.now());
            }
            capacity.finish(ticket, false);
        }
        error.clone()
    }
    fn complete(&self, active: Active, outcome: Outcome) {
        self.runtime
            .finish_at(active.hard, outcome, self.runtime.now());
        if let Some(ticket) = active.capacity {
            self.runtime
                .capacity
                .lock()
                .finish(ticket, matches!(outcome, Outcome::Success));
        }
    }
}
impl Drop for CircuitAttempt {
    fn drop(&mut self) {
        self.finish(Outcome::Neutral);
    }
}
