use bytes::Bytes;
use futures::Stream;

use crate::error::GatewayError;
use crate::implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

fn compiled_out_error() -> GatewayError {
    compiled_out_error_for_line(RefactoredImplementationLine::GrokWebReverseApi)
}

pub fn build_request_plan(
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
) -> Result<RequestPlan, GatewayError> {
    Err(compiled_out_error())
}

pub async fn accumulate_grok_stream(
    _response: rquest::Response,
    _model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(compiled_out_error())
}

pub fn translate_grok_ndjson_to_openai_sse(
    _line: &[u8],
    _model: &str,
    _response_id: &str,
    _created: i64,
) -> Option<Vec<u8>> {
    None
}

pub fn translate_grok_stream(
    _inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    _model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::empty()
}
