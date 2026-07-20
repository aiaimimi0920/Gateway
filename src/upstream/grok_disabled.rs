use rquest::header::HeaderMap;

use crate::error::GatewayError;
use crate::implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

fn compiled_out_error() -> GatewayError {
    compiled_out_error_for_line(RefactoredImplementationLine::GrokWebReverseApi)
}

pub fn apply_runtime_headers(_payload: &ProviderAccountPayload, _map: &mut HeaderMap) {}

pub fn build_request_plan(
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
) -> Result<RequestPlan, GatewayError> {
    Err(compiled_out_error())
}
