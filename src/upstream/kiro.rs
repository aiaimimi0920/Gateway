use rquest::Method;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind};
use crate::protocol::kiro;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub fn supports_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions | EndpointKind::Messages | EndpointKind::Responses
    )
}

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    if !supports_endpoint(req.endpoint_kind) {
        return Err(GatewayError::bad_request(
            "Kiro adapters currently support only chat/messages/responses style endpoints",
        )
        .with_code("unsupported_kiro_endpoint"));
    }

    let path = payload
        .responses_path
        .as_deref()
        .unwrap_or(kiro::KIRO_GENERATE_ASSISTANT_RESPONSE_PATH);
    Ok(RequestPlan {
        method: Method::POST,
        url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
        query: Vec::new(),
        body: Some(kiro::pack_kiro(req, model)?),
        response_kind: req.endpoint_kind,
    })
}

pub async fn accumulate_stream(
    response: rquest::Response,
    model: &str,
    req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayResponse, GatewayError> {
    kiro::accumulate_kiro_stream(response, model, req).await
}
