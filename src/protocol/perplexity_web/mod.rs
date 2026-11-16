//! 独立消费端 Web 协议；不复用 Sonar/Search，也不推断订阅或额度。
use crate::error::{FallbackHint, GatewayError};
mod decode;
pub mod material;
pub(crate) mod public_snapshot;
mod request;
mod schematized;
mod snapshot;
mod workflow;
pub(crate) use decode::{Decoder, Update, BODY_LIMIT};
pub use request::pack_single_turn;
pub(crate) use request::pack_single_turn_for_model;

pub use super::response_sources::ResponseSource as Source;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Answer {
    pub text: String,
    pub sources: Vec<Source>,
}

impl Answer {
    /// `turbo` names the requested policy, not an observed backend model.
    pub fn into_canonical(self) -> super::canonical::CanonicalRelayResponse {
        super::canonical::CanonicalRelayResponse {
            bedrock_turn: None,
            vertex_turn: None,
            model_declaration: None,
            model: "unknown".into(),
            text: self.text,
            sources: self.sources,
            usage: None,
            tool_calls: Vec::new(),
            upstream_status: Some(200),
            finish_reason: Some("stop".into()),
        }
    }
}

pub(crate) fn failure(code: &'static str, message: &'static str) -> GatewayError {
    let mut error = stop(GatewayError::server_error(message).with_code(code));
    error.http_status = Some(502);
    error
}

pub(crate) fn input_failure(code: &'static str, message: &'static str) -> GatewayError {
    stop(GatewayError::bad_request(message).with_code(code))
}

fn stop(mut error: GatewayError) -> GatewayError {
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Perplexity Web requests are never resubmitted automatically.".into(),
    };
    error
}

#[cfg(test)]
mod negative_tests;
#[cfg(test)]
mod schematized_tests;
#[cfg(test)]
mod snapshot_stream_tests;
#[cfg(test)]
mod terminal_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod workflow_boundary_tests;
#[cfg(test)]
mod workflow_tests;
