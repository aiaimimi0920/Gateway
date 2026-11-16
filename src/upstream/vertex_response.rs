//! Vertex products share verified REST algorithms, never credentials, quota or ordered-turn identity.
use super::vertex_product::Contract;
use crate::{
    error::{FallbackHint, GatewayError},
    protocol::{canonical::TokenUsage, upstream_body::collect_bounded_upstream_body_with_provider},
};
use serde_json::Value;
mod function_stream;
mod projection;
mod sse;
mod turn;
pub(crate) struct VerifiedResponse {
    pub body: Value,
    pub usage: Option<TokenUsage>,
}
impl Contract {
    pub fn project(
        self,
        verified: VerifiedResponse,
        model: &str,
    ) -> Result<crate::protocol::canonical::CanonicalRelayResponse, GatewayError> {
        projection::project(self, &verified.body, model, verified.usage)
    }
    pub async fn read(
        self,
        request: impl std::future::Future<Output = Result<rquest::Response, GatewayError>>,
    ) -> Result<VerifiedResponse, GatewayError> {
        let response = request.await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let mut error = crate::upstream::response_error::classify_response_error(
                response,
                self.profile,
                "Vertex HTTP error body",
            )
            .await;
            // Retain classified kind/timing/recovery, not arbitrary upstream diagnostics or codes.
            if !matches!(
                error.code.as_deref(),
                Some("upstream_body_too_large" | "upstream_body_buffer_allocation_failed")
            ) {
                error.message = format!("{} upstream returned HTTP {status}.", self.label);
                error.code = Some(format!("{}_http_error", self.code_prefix));
            }
            return Err(error);
        }
        let sse = response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.split(';').next())
            .is_some_and(|h| h.trim().eq_ignore_ascii_case("text/event-stream"));
        if !sse {
            return Err(self.invalid("Expected an SSE response."));
        }
        let body =
            collect_bounded_upstream_body_with_provider(response, "Vertex SSE body", self.profile)
                .await
                .map_err(|error| self.body_error(error))?;
        self.parse(&body)
    }
    pub fn parse(self, body: &[u8]) -> Result<VerifiedResponse, GatewayError> {
        sse::parse(self, body)
    }
    pub fn invalid(self, reason: &'static str) -> GatewayError {
        let mut error =
            GatewayError::server_error(format!("Invalid {} response: {reason}", self.label))
                .with_code(format!("{}_invalid_response", self.code_prefix))
                .with_provider(self.profile);
        error.http_status = Some(502);
        self.abort(&mut error);
        error
    }
    fn abort(self, error: &mut GatewayError) {
        // After receiving a response, damage/EOF/timeout cannot prove generation never started.
        error.retryable = false;
        error.fallback_hint = FallbackHint::Abort {
            reason: format!("{} completion was not verified.", self.label),
        };
    }
    fn body_error(self, error: GatewayError) -> GatewayError {
        let mut safe = self.invalid("The SSE body could not be verified.");
        if matches!(
            error.code.as_deref(),
            Some("upstream_body_too_large" | "upstream_body_buffer_allocation_failed")
        ) {
            safe.code = error.code;
        }
        safe
    }
    fn prompt_blocked(self) -> GatewayError {
        GatewayError::content_filtered(format!("{} blocked the prompt.", self.label))
            .with_code(format!("{}_prompt_blocked", self.code_prefix))
            .with_provider(self.profile)
    }
}
