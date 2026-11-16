//! Explicit text probes reuse the production identity/transport boundary without replay.
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use crate::{
    protocol::{canonical::CanonicalRelayResponse, openai},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};

pub(crate) async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    model: &str,
    prompt: &str,
) -> Result<(u16, CanonicalRelayResponse), GatewayError> {
    let request = openai::normalize_chat_completions(json!({
        "model":model,"stream":false,"max_tokens":256,"n":1,
        "messages":[{"role":"user","content":prompt}]
    }))?;
    let plan = crate::upstream::openai_compatible_request_plan::build_request_plan(
        payload, &request, model, false,
    )?;
    // Production finalizer validates the fixed target and clears ambient credentials/redirects.
    let response = client
        .send_profiled_plan(payload, &plan, super::runtime::headers(payload))?
        .send()
        .await?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err(response_error());
    }
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(),
            "Nous Portal model test",
            65_536,
            None,
        )
        .await
        .map_err(|_| response_error())?;
    let body: Value = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    let mut result = openai::chat_admission::unpack(&body).map_err(|_| response_error())?;
    if body["choices"].as_array().is_none_or(|v| v.len() != 1)
        || body["choices"][0]["message"]
            .get("refusal")
            .is_some_and(|v| !v.is_null())
        || result.finish_reason.as_deref() != Some("stop")
        || !result.tool_calls.is_empty()
        || result.text.trim().is_empty()
        || result.text.chars().count() > 8192
    {
        return Err(response_error());
    }
    // The selected access token is authoritative; refresh remains with the durable grant owner.
    result.text = result.text.replace(&payload.api_key, "[redacted]");
    result.text = crate::error::sanitize_provider_error_message(&result.text);
    Ok((status, result))
}

// A probe failure never authorizes replay or exposing an upstream error body.
fn response_error() -> GatewayError {
    let mut error =
        GatewayError::server_error("Nous Portal returned a failed or invalid Chat test response.")
            .with_code("nous_portal_probe_response_failed");
    error.http_status = Some(502);
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "Nous Portal probe response received; do not replay inference.".into(),
    };
    error
}
