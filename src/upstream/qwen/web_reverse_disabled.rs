use std::collections::HashMap;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::conflict("Qwen Web reverse implementation line was compiled out.")
        .with_code("gateway_provider_line_compiled_out")
}

pub async fn execute(
    _client: &UpstreamClient,
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(unsupported_request_plan_error())
}

pub async fn execute_stream(
    _client: &UpstreamClient,
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _extra_headers: Option<&HashMap<String, String>>,
) -> Result<
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
    GatewayError,
> {
    Err(unsupported_request_plan_error())
}
