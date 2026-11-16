use bytes::Bytes;
use futures::{stream, Stream};
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};

// Feature gating disables network/packing, not pure historical material conversion.
#[path = "kiro/material_metadata.rs"]
mod material_metadata;
pub use material_metadata::{generate_machine_id, normalize_machine_id, read_payload_string};
#[path = "kiro/runtime_state.rs"]
mod runtime_state;
pub(crate) use runtime_state::valid as valid_runtime_state;

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

pub fn pack_kiro_api_key(
    _req: &CanonicalRelayRequest,
    _model: &str,
) -> Result<Value, GatewayError> {
    Err(compiled_out_error())
}

pub fn translate_kiro_api_key_stream<E: Send + 'static>(
    _inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    _model: String,
    _req: CanonicalRelayRequest,
    _anthropic: bool,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    stream::empty()
}

pub fn is_reserved_payload_extra_key(_key: &str) -> bool {
    false
}

pub async fn accumulate_kiro_stream(
    _response: rquest::Response,
    _model: &str,
    _req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(compiled_out_error())
}

pub async fn accumulate_kiro_api_key_stream(
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

pub fn translate_kiro_event_stream_to_openai_sse_with_error<E: Send + 'static>(
    _inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    _model: String,
    _req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    stream::empty()
}

pub fn translate_kiro_event_stream_to_anthropic_sse(
    _inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    _model: String,
    _req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    stream::empty()
}

pub fn translate_kiro_event_stream_to_anthropic_sse_with_error<E: Send + 'static>(
    _inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    _model: String,
    _req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    stream::empty()
}
