//! DashScope generation planner. Streaming is negotiated by a header, not `stream` JSON.
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily},
        dashscope,
    },
    routing::candidate::ProviderAccountPayload,
    upstream::common::RequestPlan,
};

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
    ) {
        return Err(GatewayError::bad_request(
            "DashScope generation only supports conversational endpoints",
        ));
    }
    let multimodal = payload.adapter == "dashscope_multimodal_compatible";
    if dashscope::is_dashscope(req)
        && multimodal != (req.protocol_family == ProtocolFamily::DashScopeMultimodal)
    {
        dashscope::validate_bridge(req)?;
        return Err(GatewayError::bad_request(
            "DashScope text and multimodal surfaces are not interchangeable",
        )
        .with_code("unsupported_dashscope_surface"));
    }
    let path = payload
        .chat_completions_path
        .as_deref()
        .unwrap_or(if multimodal {
            dashscope::MULTIMODAL_PATH
        } else {
            dashscope::TEXT_PATH
        });
    let base = payload.base_url.trim_end_matches('/');
    let base = if payload.chat_completions_path.is_some() || base.ends_with("/api/v1") {
        base.to_string()
    } else {
        format!("{base}/api/v1")
    };
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: format!("{base}{path}"),
        query: Vec::new(),
        body: Some(dashscope::pack(req, model, stream, multimodal)?),
        response_kind: req.endpoint_kind,
    })
}
