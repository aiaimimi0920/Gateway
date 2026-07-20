use std::collections::HashMap;
use std::pin::Pin;

use futures::Stream;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;

pub async fn execute_legacy_text(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    super::execute(
        &client.http,
        client.timeout,
        payload,
        req,
        model,
        extra_headers,
    )
    .await
}

pub async fn execute_legacy_text_stream(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Pin<Box<dyn Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>, GatewayError> {
    super::execute_stream(
        &client.http,
        client.timeout,
        payload,
        req,
        model,
        extra_headers,
    )
    .await
}
