use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;

pub(crate) const IMPLEMENTATION_LINE: &str = "udio_web_reverse_api";
pub(crate) const PROVIDER_ADAPTER: &str = "udio_compatible";

fn compiled_out_error() -> GatewayError {
    crate::implementation_lines::compiled_out_error_for_line(
        crate::implementation_lines::RefactoredImplementationLine::UdioWebReverseApi,
    )
}

impl UpstreamClient {
    pub(crate) async fn execute_udio_browser_executor_service_invocation(
        &self,
        _input: &Value,
    ) -> Result<Value, GatewayError> {
        Err(compiled_out_error())
    }

    pub(crate) async fn execute_udio_media(
        &self,
        _provider_account_id: &str,
        _payload: &ProviderAccountPayload,
        _req: &CanonicalRelayRequest,
        _model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        Err(compiled_out_error())
    }
}
