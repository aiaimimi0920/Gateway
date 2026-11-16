//! Build uses the shared Responses codec, with its own audience, headers and final send checks.
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, EndpointKind},
        grok_oauth::business,
    },
    routing::candidate::ProviderAccountPayload,
    upstream::common::RequestPlan,
};
use rquest::header::{HeaderMap, HeaderValue};

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    business::validate(payload, payload.canonical_adapter())?;
    business::validate_material(payload)?;
    if !matches!(
        req.endpoint_kind,
        EndpointKind::Responses
            | EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Completions
    ) {
        return Err(business::invalid());
    }
    #[cfg(feature = "family-openai-compatible-official-api")]
    {
        let mut plan = super::openai_compatible_official_api_common::build_request_plan(
            payload, req, model, stream,
        )?;
        plan.response_kind = EndpointKind::Responses;
        business::validate_plan(payload, &plan)?;
        Ok(plan)
    }
    #[cfg(not(feature = "family-openai-compatible-official-api"))]
    {
        let _ = (model, stream);
        Err(business::invalid())
    }
}

pub(crate) fn finalize_headers(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    headers: &mut HeaderMap,
) -> Result<(), GatewayError> {
    if !business::owns_payload(payload) {
        return Ok(());
    }
    business::validate_plan(payload, plan)?;
    let values = headers.get_all("authorization");
    if values.iter().count() != 1
        || !values
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .is_some_and(|v| v == payload.api_key)
        || [
            "cookie",
            "host",
            "proxy-authorization",
            "x-api-key",
            "api-key",
            "x-goog-api-key",
        ]
        .iter()
        .any(|h| headers.contains_key(*h))
    {
        return Err(business::invalid());
    }
    // Caller software identity never makes this Gateway an OpenAI/Anthropic first-party client.
    let foreign: Vec<_> = headers
        .keys()
        .filter(|h| business::foreign_identity(h.as_str()))
        .cloned()
        .collect();
    for name in foreign {
        headers.remove(name);
    }
    headers.insert(
        "user-agent",
        HeaderValue::from_static("NeuroGateway/GrokBuild"),
    );
    // This is the observed Build wire selector, not a borrowed OAuth client ID or proof of entitlement.
    headers.insert("x-xai-token-auth", HeaderValue::from_static("xai-grok-cli"));
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers.insert(
        "accept",
        HeaderValue::from_static(
            if plan.body.as_ref().and_then(|b| b["stream"].as_bool()) == Some(true) {
                "text/event-stream"
            } else {
                "application/json"
            },
        ),
    );
    Ok(())
}
