use rquest::Method;

use crate::error::GatewayError;
use crate::protocol::anthropic;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    match req.endpoint_kind {
        EndpointKind::ChatCompletions
        | EndpointKind::Messages
        | EndpointKind::Completions
        | EndpointKind::Responses => {
            let path = payload.messages_path.as_deref().unwrap_or("/v1/messages");
            Ok(RequestPlan {
                method: Method::POST,
                url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
                query: Vec::new(),
                body: Some(if payload.is_discovered_native(req) {
                    crate::upstream::common::merge_model_and_stream_into_body(req.raw_body.clone(), model, Some(stream))
                } else { anthropic::pack_anthropic(req, model, stream) }),
                response_kind: EndpointKind::Messages,
            })
        }
        _ => Err(GatewayError::bad_request(
            "Anthropic-compatible official API surfaces currently support only text chat endpoints.",
        )
        .with_code("unsupported_anthropic_official_endpoint")),
    }
}
