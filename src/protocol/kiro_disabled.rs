use std::collections::HashMap;

use bytes::Bytes;
use futures::{stream, Stream};
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};

pub const KIRO_DEFAULT_MODEL: &str = "claude-sonnet-4.6";
pub const KIRO_GENERATE_ASSISTANT_RESPONSE_PATH: &str = "/generateAssistantResponse";
pub const KIRO_DEFAULT_VERSION: &str = "0.11.107";
pub const KIRO_DEFAULT_SYSTEM_VERSION: &str = "win32#10.0.22631";
pub const KIRO_DEFAULT_NODE_VERSION: &str = "22.22.0";

fn compiled_out_error() -> GatewayError {
    crate::implementation_lines::compiled_out_error_for_line(
        crate::implementation_lines::RefactoredImplementationLine::KiroOfficialVendorApi,
    )
}

pub fn pack_kiro(_req: &CanonicalRelayRequest, _model: &str) -> Result<Value, GatewayError> {
    Err(compiled_out_error())
}

pub fn is_reserved_payload_extra_key(_key: &str) -> bool {
    false
}

pub fn read_payload_string(
    _extra: Option<&HashMap<String, Value>>,
    _aliases: &[&str],
) -> Option<String> {
    None
}

pub fn generate_machine_id(
    _explicit_machine_id: Option<&str>,
    _refresh_token: Option<&str>,
) -> Option<String> {
    None
}

pub async fn accumulate_kiro_stream(
    _response: rquest::Response,
    _model: &str,
    _req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(compiled_out_error())
}

pub fn translate_kiro_event_stream_to_openai_sse(
    _inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    _model: String,
    _req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    stream::empty()
}

pub fn translate_kiro_event_stream_to_anthropic_sse(
    _inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    _model: String,
    _req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    stream::empty()
}
