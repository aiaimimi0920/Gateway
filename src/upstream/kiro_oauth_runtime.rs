//! Registered OAuth runtime sender. Fixed-reference compatibility, not an official general model API.
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
        kiro,
        kiro_oauth::{runtime, RuntimeTarget},
    },
    routing::candidate::ProviderAccountPayload,
    upstream::common::RequestPlan,
};
use rquest::header::HeaderMap;

fn terminal(mut error: GatewayError) -> GatewayError {
    error.message = "Kiro OAuth runtime request failed.".into();
    error.code = Some("kiro_oauth_runtime_upstream_failed".into());
    error.provider_name = Some("kiro-oauth".into());
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "Kiro OAuth generations must not be replayed.".into(),
    };
    error
}
pub fn headers(payload: &ProviderAccountPayload) -> HeaderMap {
    // The fixed reference uses identical CLI/AWS headers except the static-Key-only TokenType.
    let mut map = super::kiro_api_key::headers(payload);
    map.remove("tokentype");
    map
}
pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    runtime::validate(payload)?;
    super::kiro_api_key::validate_cli_request(payload, req, model)
        .map_err(|_| runtime::invalid())?;
    let target = RuntimeTarget::from_payload(payload).map_err(|_| runtime::invalid())?;
    let mut body = kiro::pack_kiro_api_key(req, model).map_err(|_| runtime::invalid())?;
    body["profileArn"] = serde_json::json!(target.profile_arn());
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: format!("{}/", target.base_url()),
        query: vec![],
        body: Some(body),
        response_kind: req.endpoint_kind,
    };
    runtime::validate_plan(payload, &plan)?;
    Ok(plan)
}
pub fn finalize_headers(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    map: &mut HeaderMap,
) -> Result<(), GatewayError> {
    if runtime::owns_payload(payload) {
        runtime::validate_plan(payload, plan)?;
        *map = headers(payload);
    }
    Ok(())
}
pub fn receive_response(
    payload: &ProviderAccountPayload,
    result: Result<rquest::Response, GatewayError>,
) -> Result<rquest::Response, GatewayError> {
    if !runtime::owns_payload(payload) {
        return super::kiro_api_key::receive_response(payload, result);
    }
    let response = result.map_err(terminal)?;
    if !response.status().is_success() {
        return Err(terminal(
            crate::error::classify_upstream_error(
                response.status().as_u16(),
                "",
                Some("kiro-oauth"),
            )
            .with_retry_after_ms(crate::error::retry_after_from_headers(
                response.headers(),
                std::time::SystemTime::now(),
            )),
        ));
    }
    if response.status().as_u16() != 200
        || response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .is_none_or(|v| {
                !v.trim()
                    .eq_ignore_ascii_case("application/vnd.amazon.eventstream")
            })
    {
        let mut error = terminal(GatewayError::server_error(
            "Invalid Kiro OAuth runtime response.",
        ));
        error.code = Some("kiro_oauth_runtime_invalid_response".into());
        return Err(error);
    }
    Ok(response)
}
pub async fn accumulate(
    response: rquest::Response,
    model: &str,
    req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut result = kiro::accumulate_kiro_api_key_stream(response, model, req)
        .await
        .map_err(terminal)?;
    result.model = model.into();
    Ok(result)
}
pub use super::kiro_api_key::translate;
