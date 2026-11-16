//! Feature exclusion removes HTTP execution, never material/destination recognition.
use super::Session;
use crate::{
    error::GatewayError,
    implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine},
    protocol::{
        canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
        perplexity_web::Answer,
    },
    routing::candidate::ProviderAccountPayload,
};
use std::time::Duration;

fn unavailable() -> GatewayError {
    compiled_out_error_for_line(RefactoredImplementationLine::PerplexityWebReverseApi)
}
pub async fn execute(
    _: &rquest::Client,
    _: Duration,
    _: &ProviderAccountPayload,
    _: &CanonicalRelayRequest,
    _: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(unavailable())
}
pub async fn execute_single_turn(
    _: &rquest::Client,
    _: &Session,
    _: &CanonicalRelayRequest,
    _: Duration,
) -> Result<Answer, GatewayError> {
    Err(unavailable())
}

pub async fn execute_stream(
    _: &rquest::Client,
    _: Duration,
    _: &ProviderAccountPayload,
    _: &CanonicalRelayRequest,
    _: &str,
) -> Result<crate::upstream::response_types::UpstreamStreamingResponse, GatewayError> {
    Err(unavailable())
}
