//! Upstream facts are captured before, and independently of, local receipt writes.
use crate::error::GatewayError;

#[derive(Clone)]
pub(crate) enum Evidence {
    Completed(u16),
    Accepted(u16),
    Neutral(u16),
    Failed {
        error: GatewayError,
        status: Option<u16>,
    },
}

impl Evidence {
    pub(crate) fn status(&self) -> Option<u16> {
        match self {
            Self::Completed(status) | Self::Accepted(status) | Self::Neutral(status) => {
                Some(*status)
            }
            Self::Failed { status, .. } => *status,
        }
    }

    pub(super) fn failed(error: GatewayError, status: Option<u16>) -> Self {
        Self::Failed { error, status }
    }

    pub(super) fn terminal(body: &serde_json::Value, status: u16) -> Self {
        match body["status"].as_str() {
            Some("completed" | "incomplete") => Self::Completed(status),
            Some("queued" | "in_progress") => Self::Accepted(status),
            // Neither a caller cancellation nor an unspecified task error proves
            // credential failure. Never classify arbitrary upstream error text.
            Some("failed") => match body["error"]["code"].as_str() {
                Some("server_error") => Self::failed(
                    super::super::failure("sap_aicore_response_failed"),
                    Some(status),
                ),
                Some("rate_limit_exceeded") => Self::failed(
                    super::super::errors::rejected(429, None, false),
                    Some(status),
                ),
                _ => Self::Neutral(status),
            },
            _ => Self::Neutral(status),
        }
    }

    pub(super) fn finish(&self, circuit: &crate::routing::model_circuit::CircuitAttempt) {
        use crate::routing::model_circuit::Outcome;
        match self {
            Self::Completed(_) | Self::Accepted(_) => circuit.finish(Outcome::Success),
            Self::Neutral(_) => circuit.finish(Outcome::Neutral),
            Self::Failed { error, .. } => {
                circuit.finish_error(error);
            }
        }
    }
}
