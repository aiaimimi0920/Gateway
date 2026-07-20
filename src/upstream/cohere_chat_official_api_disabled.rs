use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

fn compiled_out_error() -> GatewayError {
    crate::implementation_lines::compiled_out_error_for_line(
        crate::implementation_lines::RefactoredImplementationLine::CohereChatOfficialModelApi,
    )
}

pub fn build_request_plan(
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _stream: bool,
) -> Result<RequestPlan, GatewayError> {
    Err(compiled_out_error())
}

pub async fn accumulate_stream(
    _response: rquest::Response,
    _model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(compiled_out_error())
}

pub fn unpack_response(_body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(compiled_out_error())
}
