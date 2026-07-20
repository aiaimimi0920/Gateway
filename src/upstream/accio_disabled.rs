use crate::error::GatewayError;
use crate::implementation_lines;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
use rquest::header::HeaderMap;

pub fn unsupported_request_plan_error() -> GatewayError {
    implementation_lines::accio_web_reverse_api_compiled_out_error(
        "request planning attempted for adapter accio_compatible",
    )
}

pub fn build_request_plan(
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _stream: bool,
) -> Result<RequestPlan, GatewayError> {
    Err(unsupported_request_plan_error())
}

pub fn apply_runtime_headers(_payload: &ProviderAccountPayload, _map: &mut HeaderMap) {}
