//! Match the protocol surface's feature gate; disabled entry points fail before any I/O.
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
    routing::candidate::ProviderAccountPayload,
    upstream::response_types::BinaryUpstreamResponse,
};
use std::{collections::HashMap, pin::Pin, time::Duration};
type GeminiWebStream =
    Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>;

fn unavailable() -> GatewayError {
    crate::implementation_lines::compiled_out_error_for_line(
        crate::implementation_lines::RefactoredImplementationLine::GeminiWebReverse,
    )
    .with_provider("gemini_web_compatible")
}

pub async fn execute(
    _http: &rquest::Client,
    _timeout: Duration,
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(unavailable())
}

pub async fn execute_stream(
    _http: &rquest::Client,
    _timeout: Duration,
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _extra_headers: Option<&HashMap<String, String>>,
) -> Result<GeminiWebStream, GatewayError> {
    Err(unavailable())
}

pub async fn execute_speech(
    _http: &rquest::Client,
    _timeout: Duration,
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _extra_headers: Option<&HashMap<String, String>>,
) -> Result<BinaryUpstreamResponse, GatewayError> {
    Err(unavailable())
}

pub async fn execute_transcription(
    _http: &rquest::Client,
    _timeout: Duration,
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _extra_headers: Option<&HashMap<String, String>>,
) -> Result<String, GatewayError> {
    Err(unavailable())
}
