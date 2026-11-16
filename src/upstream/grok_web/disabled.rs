//! The typed selection remains recognized when the optional wire is not compiled.
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
    routing::candidate::ProviderAccountPayload,
    upstream::response_types::UpstreamStreamingResponse,
};
use std::time::Duration;
pub(crate) async fn execute(
    _: &rquest::Client,
    _: Duration,
    _: &ProviderAccountPayload,
    _: &CanonicalRelayRequest,
    _: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(super::failure(
        "grok_web_line_disabled",
        ErrorKind::ServiceUnavailable,
    ))
}
pub(crate) async fn execute_stream(
    _: &rquest::Client,
    _: Duration,
    _: &ProviderAccountPayload,
    _: &CanonicalRelayRequest,
    _: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    Err(super::failure(
        "grok_web_line_disabled",
        ErrorKind::ServiceUnavailable,
    ))
}
