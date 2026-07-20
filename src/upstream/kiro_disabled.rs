use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

fn compiled_out_error() -> GatewayError {
    crate::implementation_lines::compiled_out_error_for_line(
        crate::implementation_lines::RefactoredImplementationLine::KiroOfficialVendorApi,
    )
}

pub fn supports_endpoint(_endpoint_kind: EndpointKind) -> bool {
    false
}

pub fn build_request_plan(
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
) -> Result<RequestPlan, GatewayError> {
    Err(compiled_out_error())
}

pub async fn accumulate_stream(
    _response: rquest::Response,
    _model: &str,
    _req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(compiled_out_error())
}
