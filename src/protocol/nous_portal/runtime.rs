//! Portal inference owns fixed Chat/embedding destinations, never a borrowed CLI or browser wire.
use super::business;
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, EndpointKind},
    routing::candidate::ProviderAccountPayload,
    upstream::common::RequestPlan,
};
use rquest::header::{HeaderMap, HeaderValue};

pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !business::owns_payload(payload) {
        return Ok(());
    }
    crate::implementation_lines::ensure_payload_compiled(payload)?;
    business::validate_shape(payload, adapter)
}

pub fn validate_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Result<(), GatewayError> {
    if !business::owns_payload(payload) {
        return Ok(());
    }
    validate(payload, payload.canonical_adapter())?;
    let suffix = match plan.response_kind {
        EndpointKind::ChatCompletions => "/chat/completions",
        EndpointKind::Embeddings => "/embeddings",
        _ => return Err(business::invalid()),
    };
    let body = plan
        .body
        .as_ref()
        .and_then(|b| b.as_object())
        .ok_or_else(business::invalid)?;
    if plan.method != rquest::Method::POST
        || !super::atom(&payload.api_key, 32_768)
        || !plan.query.is_empty()
        || plan.url != format!("{}{suffix}", payload.base_url.trim_end_matches('/'))
        || !body
            .get("model")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
        || body.keys().any(|k| {
            k.starts_with("__gateway_")
                || matches!(
                    k.as_str(),
                    "provider"
                        | "session_id"
                        | "cache_control"
                        | "instructions"
                        | "previous_response_id"
                )
                || k == "input" && plan.response_kind != EndpointKind::Embeddings
        })
        || plan.response_kind == EndpointKind::ChatCompletions
            && (!body.get("messages").is_some_and(|v| v.is_array())
                || !body.get("stream").is_some_and(|v| v.is_boolean()))
    {
        return Err(business::invalid());
    }
    Ok(())
}

/// Chat bridges carry explicit history, not remote Responses state or unimplemented hosted tools.
pub fn validate_request(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
) -> Result<(), GatewayError> {
    if business::owns_payload(payload) && req.endpoint_kind == EndpointKind::Completions {
        validate_legacy_bridge(&req.raw_body)?;
    }
    if business::owns_payload(payload)
        && (req.previous_response_id.is_some()
            || req
                .raw_body
                .get("conversation")
                .is_some_and(|v| !v.is_null())
            || req.raw_body.get("store").is_some_and(|v| v == true)
            || req.raw_body.get("background").is_some_and(|v| v == true)
            || req.tools.iter().any(|tool| tool.tool_type != "function"))
    {
        return Err(business::invalid());
    }
    Ok(())
}

/// A single Chat turn cannot implement independent prompt batches or legacy-only sampling output.
fn validate_legacy_bridge(body: &serde_json::Value) -> Result<(), GatewayError> {
    if !body.get("prompt").is_some_and(|v| v.is_string())
        || ["n", "best_of"].iter().any(|key| {
            body.get(key)
                .is_some_and(|v| !v.is_null() && v.as_u64() != Some(1))
        })
        || body.get("echo").is_some_and(|v| !v.is_null() && v != false)
        || ["logprobs", "suffix"]
            .iter()
            .any(|key| body.get(key).is_some_and(|v| !v.is_null()))
        || body
            .get("logit_bias")
            .is_some_and(|v| !v.is_null() && !v.as_object().is_some_and(|map| map.is_empty()))
    {
        return Err(GatewayError::bad_request(
            "Nous legacy Completions supports one string prompt without echo, logprobs, suffix, logit_bias or multiple candidates.",
        )
        .with_code("nous_legacy_completions_unsupported"));
    }
    Ok(())
}

pub fn headers(payload: &ProviderAccountPayload) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers.insert(
        "user-agent",
        HeaderValue::from_static("NeuroGateway/NousPortal"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", payload.api_key)) {
        headers.insert("authorization", value);
    }
    headers
}

pub(crate) fn finalize_headers(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    actual: &mut HeaderMap,
) -> Result<(), GatewayError> {
    if !business::owns_payload(payload) {
        return Ok(());
    }
    validate_plan(payload, plan)?;
    let values = actual.get_all("authorization");
    if values.iter().count() != 1
        || !values
            .iter()
            .next()
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .is_some_and(|s| s == payload.api_key)
        || [
            "cookie",
            "host",
            "proxy-authorization",
            "x-api-key",
            "api-key",
            "x-goog-api-key",
        ]
        .iter()
        .any(|k| actual.contains_key(*k))
    {
        return Err(business::invalid());
    }
    // Selected credentials are authoritative; late foreign identity/trace headers are not a Portal contract.
    *actual = headers(payload);
    actual.insert(
        "accept",
        HeaderValue::from_static(if plan.body.as_ref().is_some_and(|b| b["stream"] == true) {
            "text/event-stream"
        } else {
            "application/json"
        }),
    );
    Ok(())
}
