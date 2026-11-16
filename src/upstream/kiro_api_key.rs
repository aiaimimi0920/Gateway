//! CLI wire owner; same bounded EventStream decoder, no OAuth refresh, helper, fallback or retry.
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
        kiro, kiro_api_key as key,
    },
    routing::candidate::ProviderAccountPayload,
    upstream::common::RequestPlan,
};
use rquest::header::{HeaderMap, HeaderValue};

fn terminal_error(mut error: GatewayError) -> GatewayError {
    // No idempotency contract: a lost reply does not authorize another generation/account.
    // Provider text/codes may contain secrets; retain only typed classification and timing.
    error.message = "Kiro CLI Key request failed.".into();
    error.code = Some("kiro_api_key_upstream_failed".into());
    error.provider_name = Some("kiro-api-key".into());
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "Kiro CLI Key generations must not be replayed.".into(),
    };
    error
}

pub fn receive_response(
    payload: &ProviderAccountPayload,
    result: Result<rquest::Response, GatewayError>,
) -> Result<rquest::Response, GatewayError> {
    if !key::owns_payload(payload) {
        return result;
    }
    let response = result.map_err(terminal_error)?;
    if !response.status().is_success() {
        // Drop the untrusted body unread: diagnostics must not return or persist it.
        return Err(terminal_error(
            crate::error::classify_upstream_error(
                response.status().as_u16(),
                "",
                Some("kiro-api-key"),
            )
            .with_retry_after_ms(crate::error::retry_after_from_headers(
                response.headers(),
                std::time::SystemTime::now(),
            )),
        ));
    }
    validate_response(payload, &response)?;
    Ok(response)
}

const USER_AGENT: &str = "aws-sdk-rust/1.3.15 ua/2.1 api/codewhispererstreaming/0.1.17593 os/macos lang/rust/1.92.0 md/appVersion-2.10.0 app/AmazonQ-For-CLI";
const AMZ_USER_AGENT: &str = "aws-sdk-rust/1.3.15 ua/2.1 api/codewhispererstreaming/0.1.17593 os/macos lang/rust/1.92.0 m/F app/AmazonQ-For-CLI";

pub fn headers(payload: &ProviderAccountPayload) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in [
        ("content-type", "application/x-amz-json-1.0"),
        ("accept", "*/*"),
        ("tokentype", "API_KEY"),
        ("x-amz-target", key::TARGET),
        ("user-agent", USER_AGENT),
        ("x-amz-user-agent", AMZ_USER_AGENT),
        ("x-amzn-codewhisperer-optout", "true"),
        ("amz-sdk-request", "attempt=1; max=1"),
    ] {
        map.insert(name, HeaderValue::from_static(value));
    }
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", payload.api_key)) {
        map.insert("authorization", value);
    }
    map.insert(
        "amz-sdk-invocation-id",
        HeaderValue::from_str(&uuid::Uuid::new_v4().to_string()).expect("UUID is an ASCII header"),
    );
    map
}
pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    key::validate(payload, payload.canonical_adapter())?;
    validate_cli_request(payload, req, model)?;
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: format!("{}/", key::BASE_URL),
        query: Vec::new(),
        body: Some(kiro::pack_kiro_api_key(req, model)?),
        response_kind: req.endpoint_kind,
    };
    key::validate_plan(payload, &plan)?;
    Ok(plan)
}
// Both runtime authentication products share CLI content, not token or profile ownership.
pub(crate) fn validate_cli_request(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<(), GatewayError> {
    if !crate::upstream::kiro::supports_endpoint(req.endpoint_kind)
        || !crate::provider_model_id::valid_for_payload(model, payload)
        || req.reasoning.is_some()
        || req.tool_choice.as_ref().is_some_and(|choice| {
            choice.as_str() != Some("auto")
                && !(choice.as_object().is_some_and(|o| o.len() == 1) && choice["type"] == "auto")
        })
        || req.tools.iter().any(|tool| {
            tool.tool_type != "function"
                || tool.name.as_ref().is_none_or(|name| name.trim().is_empty())
        })
        || req.extra.keys().any(|k| {
            !matches!(
                k.as_str(),
                "model"
                    | "messages"
                    | "stream"
                    | "tools"
                    | "tool_choice"
                    | "user"
                    | "metadata"
                    | "max_tokens"
                    | "max_completion_tokens"
            )
        })
    {
        return Err(key::invalid());
    }
    Ok(())
}
pub fn finalize_headers(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    map: &mut HeaderMap,
) -> Result<(), GatewayError> {
    if !key::owns_payload(payload) {
        return Ok(());
    }
    key::validate_plan(payload, plan)?;
    // Fixed owner replaces the complete final map: no caller, generic profile or custom header wins.
    *map = headers(payload);
    Ok(())
}
pub fn validate_response(
    payload: &ProviderAccountPayload,
    response: &rquest::Response,
) -> Result<(), GatewayError> {
    if key::owns_payload(payload)
        && (response.status().as_u16() != 200
            || response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(';').next())
                .is_none_or(|v| {
                    !v.trim()
                        .eq_ignore_ascii_case("application/vnd.amazon.eventstream")
                }))
    {
        let mut error = terminal_error(GatewayError::server_error(
            "Kiro CLI Key returned a non-EventStream response.",
        ));
        error.code = Some("kiro_api_key_invalid_response".into());
        return Err(error);
    }
    Ok(())
}
pub async fn accumulate(
    response: rquest::Response,
    model: &str,
    req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut result = kiro::accumulate_kiro_api_key_stream(response, model, req)
        .await
        .map_err(terminal_error)?;
    result.model = model.to_owned();
    Ok(result)
}
pub fn translate<E: From<crate::protocol::stream_error::ProtocolStreamError> + Send + 'static>(
    inner: impl futures::Stream<Item = Result<bytes::Bytes, E>> + Send + 'static,
    model: String,
    req: CanonicalRelayRequest,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, E>> + Send>> {
    use crate::protocol::canonical::EndpointKind;
    let kind = req.endpoint_kind;
    let stream = kiro::translate_kiro_api_key_stream(
        inner,
        model.clone(),
        req,
        kind == EndpointKind::Messages,
    );
    if kind == EndpointKind::Responses {
        Box::pin(
            crate::protocol::responses::translate_openai_sse_to_responses_with_error(stream, model),
        )
    } else {
        Box::pin(stream)
    }
}
