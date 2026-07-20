use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;

fn compiled_out_error() -> GatewayError {
    crate::implementation_lines::compiled_out_error_for_line(
        crate::implementation_lines::RefactoredImplementationLine::ChataibotWebReverse,
    )
}

pub fn unsupported_request_plan_error() -> GatewayError {
    compiled_out_error()
}

impl UpstreamClient {
    pub(crate) async fn execute_chataibot_images(
        &self,
        _payload: &ProviderAccountPayload,
        _req: &CanonicalRelayRequest,
        _model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        Err(compiled_out_error())
    }
}
