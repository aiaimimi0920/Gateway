use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

fn compiled_out_error() -> GatewayError {
    crate::implementation_lines::compiled_out_error_for_line(
        crate::implementation_lines::RefactoredImplementationLine::AzureOpenAIOfficialVendorApi,
    )
}

pub fn owns_payload(_payload: &ProviderAccountPayload) -> bool {
    false
}

pub fn build_request_plan(
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _stream: bool,
) -> Result<RequestPlan, GatewayError> {
    Err(compiled_out_error())
}
