#[cfg(not(feature = "family-openai-compatible-official-api"))]
use crate::error::GatewayError;
#[cfg(not(feature = "family-openai-compatible-official-api"))]
use crate::protocol::canonical::CanonicalRelayRequest;
#[cfg(not(feature = "family-openai-compatible-official-api"))]
use crate::routing::candidate::ProviderAccountPayload;
#[cfg(not(feature = "family-openai-compatible-official-api"))]
use crate::upstream::common::RequestPlan;

#[cfg(feature = "family-openai-compatible-official-api")]
pub use crate::upstream::openai_compatible_official_api_common::build_request_plan;

#[cfg(not(feature = "family-openai-compatible-official-api"))]
pub fn build_request_plan(
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _stream: bool,
) -> Result<RequestPlan, GatewayError> {
    Err(
        GatewayError::conflict("OpenAI-compatible official API common family was compiled out.")
            .with_code("gateway_provider_line_compiled_out"),
    )
}
