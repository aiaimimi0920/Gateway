//! Token inference owns a separate Key audience; shared Chat transport owns I/O and cancellation.
use crate::protocol::{
    call_flow::*,
    canonical::{CanonicalRelayRequest, EndpointKind},
};
use crate::{
    error::GatewayError, routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan,
};

pub const PRESET: &str = "lumalabs-inference-api";
pub const PROFILE: &str = "lumalabs_inference";
pub const AUTH_MODE: &str = "luma_inference_key";
pub const BASE: &str = "https://inference.lumalabs.ai/v1";
pub fn owns_base(base: &str) -> bool {
    url::Url::parse(base).is_ok_and(|url| url.host_str() == Some("inference.lumalabs.ai"))
}
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE) || owns_base(&payload.base_url)
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Luma LLM inference identity or unsupported request.")
        .with_code("invalid_luma_inference")
}
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 16_384
        && !key.starts_with("luma-api-")
        && key.bytes().all(|b| b.is_ascii_graphic())
}
pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    crate::implementation_lines::ensure_payload_compiled(payload)?;
    if payload.base_url.trim_end_matches('/') != BASE
        || adapter != "openai_compatible"
        || payload.adapter != adapter
        || payload.auth_mode.as_deref() != Some(AUTH_MODE)
        || (!payload.api_key.is_empty() && !valid_key(&payload.api_key))
        || payload.auth_token.is_some()
        || payload.auth_header_name.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload.runtime_state_object_key.is_some()
        || !payload.headers.is_empty()
        || payload.extra_body.as_ref().is_some_and(|m| !m.is_empty())
        || !payload.discovered_protocols.is_empty()
        || !matches!(
            payload.request_client_profile,
            None | Some(crate::protocol::request_profile::RequestClientProfile::Auto)
        )
        || payload
            .execution_mode
            .is_some_and(|v| v != crate::routing::candidate::ProviderExecutionMode::DirectHttp)
        || payload.endpoint_execution_modes.as_ref().is_some_and(|m| {
            m.values()
                .any(|v| *v != crate::routing::candidate::ProviderExecutionMode::DirectHttp)
        })
        || payload.call_flow.is_some_and(|f| {
            !matches!(f.call_adapter, CallAdapter::Auto | CallAdapter::Openai)
                || f.fingerprint_adapter != FingerprintAdapter::Auto
                || !matches!(
                    f.special_adapter,
                    SpecialAdapter::Auto | SpecialAdapter::None
                )
        })
        || payload.chat_completions_path.as_deref() != Some("/chat/completions")
        || [
            &payload.responses_path,
            &payload.messages_path,
            &payload.completions_path,
            &payload.embeddings_path,
            &payload.audio_transcriptions_path,
            &payload.audio_speech_path,
            &payload.search_path,
            &payload.fetch_path,
            &payload.research_path,
            &payload.balance_path,
        ]
        .iter()
        .any(|p| p.is_some())
    {
        return Err(invalid());
    }
    Ok(())
}
pub fn validate_request(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions | EndpointKind::Messages | EndpointKind::Responses
    ) || req.previous_response_id.is_some()
        || req
            .raw_body
            .get("conversation")
            .is_some_and(|v| !v.is_null())
        || ["store", "background"].iter().any(|k| {
            req.raw_body
                .get(k)
                .is_some_and(|v| v != false && !v.is_null())
        })
        || req.tools.iter().any(|t| t.tool_type != "function")
        || req
            .messages
            .iter()
            .flat_map(|m| &m.content)
            .any(|p| !matches!(p, crate::protocol::canonical::ContentPart::Text { .. }))
    {
        return Err(invalid());
    }
    Ok(())
}
pub fn validate_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    validate(payload, payload.canonical_adapter())?;
    let body = plan.body.as_ref().ok_or_else(invalid)?;
    if plan.url != format!("{BASE}/chat/completions")
        || plan.method != rquest::Method::POST
        || !plan.query.is_empty()
        || plan.response_kind != EndpointKind::ChatCompletions
        || !valid_key(&payload.api_key)
        || !body["model"].as_str().is_some_and(|m| !m.is_empty())
    {
        return Err(invalid());
    }
    Ok(())
}
pub fn finalize_headers(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    headers: &mut rquest::header::HeaderMap,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    validate_plan(payload, plan)?;
    // The shared client may contain ambient headers; this product sends only the exact account Key.
    headers.clear();
    headers.insert(
        "authorization",
        rquest::header::HeaderValue::from_str(&format!("Bearer {}", payload.api_key))
            .map_err(|_| invalid())?,
    );
    headers.insert(
        "content-type",
        rquest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "user-agent",
        rquest::header::HeaderValue::from_static("NeuroGateway/LumaInference"),
    );
    headers.insert(
        "accept",
        rquest::header::HeaderValue::from_static(
            if plan.body.as_ref().is_some_and(|b| b["stream"] == true) {
                "text/event-stream"
            } else {
                "application/json"
            },
        ),
    );
    Ok(())
}
pub(crate) async fn read_response(
    response: rquest::Response,
) -> Result<crate::protocol::canonical::CanonicalRelayResponse, GatewayError> {
    let body = read_json(response).await?;
    crate::protocol::openai::chat_admission::unpack(&body).map_err(|_| response_error())
}
pub(crate) async fn read_json(
    response: rquest::Response,
) -> Result<serde_json::Value, GatewayError> {
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_body(response, "Luma inference")
            .await
            .map_err(|_| response_error())?;
    let body = serde_json::from_slice(&bytes).map_err(|_| response_error())?;
    crate::protocol::openai::chat_admission::validate(&body).map_err(|_| response_error())?;
    Ok(body)
}
pub(crate) fn response_error() -> GatewayError {
    let mut error = GatewayError::server_error("Luma inference returned an invalid Chat response.")
        .with_code("luma_inference_response_failed");
    error.http_status = Some(502);
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "Luma inference response received; do not replay.".into(),
    };
    error
}
pub(crate) fn stream_error() -> crate::protocol::stream_error::ProtocolStreamError {
    crate::protocol::stream_error::ProtocolStreamError::classified(
        "invalid_luma_inference_stream: incomplete or invalid Chat stream",
        response_error(),
    )
}
pub(crate) mod config;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-lumalabs-inference-official-api"))]
mod wire_tests;
