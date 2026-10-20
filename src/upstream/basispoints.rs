//! Alternate Codex transport: fixed origin, no redirects, bounded complete response validation.
use crate::error::{classify_network_error, GatewayError};
use crate::protocol::{
    canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
    chatgpt::basispoints as wire,
};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::response_types::UpstreamStreamingResponse;
use futures::StreamExt;
use serde_json::Value;
use std::time::Duration;

pub(crate) async fn execute(
    http: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let body = exchange(http, timeout, payload, req, model).await?;
    crate::protocol::responses::unpack_responses_response(&body)
}

pub(crate) async fn execute_stream(
    http: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let body = exchange(http, timeout, payload, req, model).await?;
    let mut response = axum::http::Response::new(wire::response::replay(&body));
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/event-stream"),
    );
    Ok(UpstreamStreamingResponse::Http(rquest::Response::from(
        response,
    )))
}

pub(crate) async fn exchange(
    http: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    exchange_at(http, timeout, payload, req, model, wire::URL).await
}

// Private transport seam for loopback contracts; production callers only use the fixed product URL.
async fn exchange_at(
    http: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    url: &str,
) -> Result<Value, GatewayError> {
    if !cfg!(feature = "line-chatgpt-codex-oauth-official") {
        return Err(GatewayError::bad_request(
            "Basispoints requires the Codex OAuth protocol feature",
        ));
    }
    let source = wire::request::source(req, model)?;
    let body = wire::request::prepare(&source, model, true)?;
    let account = wire::account_id(payload).ok_or_else(|| {
        GatewayError::bad_request("Basispoints requires a ChatGPT account ID")
            .with_code("basispoints_account_missing")
    })?;
    if payload.api_key.trim().is_empty() {
        return Err(GatewayError::unauthorized(
            "Basispoints requires a Codex access token",
        ));
    }
    let mut request = http
        .post(url)
        .redirect(rquest::redirect::Policy::none())
        .timeout(timeout)
        .bearer_auth(&payload.api_key)
        .json(&body)
        .header("ChatGPT-Account-ID", account)
        .header("X-OpenAI-Account-ID", account)
        .header("X-Basispoints-Auth-Mode", "chatgpt")
        .header("Origin", "https://bps.openai.com")
        .header("Accept", "text/event-stream")
        .header("Accept-Encoding", "identity")
        .header("User-Agent", "neuro-gateway-basispoints/1");
    for (name, value) in [
        ("Client-Agent-Profile", "excel"),
        ("Client-Editor", "excel"),
        ("Client-Host", "office"),
        ("Client-Platform", "excel"),
        ("Client-Platform-Class", "PC"),
        ("Client-Product", "basispoints-excel-plugin"),
        ("Client-Runtime", "desktop"),
        ("Office-Host", "Excel"),
        ("Office-Platform", "PC"),
    ] {
        request = request.header(format!("X-OpenAI-Internal-Basispoints-{name}"), value);
    }
    // Wire compatibility with the known product profile, not a change to Gateway's identity.
    for (name, value) in [
        ("Arch", "unknown"),
        ("Lang", "js"),
        ("OS", "Unknown"),
        ("Package-Version", "6.31.0"),
        ("Retry-Count", "0"),
        ("Runtime", "browser:chrome"),
    ] {
        request = request.header(format!("X-Stainless-{name}"), value);
    }
    let response = request
        .send()
        .await
        .map_err(|e| classify_network_error(&e, Some(wire::ADAPTER)))?;
    if !response.status().is_success() {
        return Err(crate::upstream::response_error::classify_response_error(
            response,
            wire::ADAPTER,
            "Basispoints request failed",
        )
        .await);
    }
    let sse = response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.starts_with("text/event-stream"));
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| classify_network_error(&e, Some(wire::ADAPTER)))?;
        if chunk.len() > 16 * 1024 * 1024 - bytes.len() {
            return Err(
                GatewayError::server_error("Basispoints response exceeded 16 MiB")
                    .with_code("basispoints_response_too_large"),
            );
        }
        bytes.extend_from_slice(&chunk);
    }
    let prefix = bytes
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(&bytes)
        .trim_ascii_start();
    wire::response::decode(
        &bytes,
        sse || prefix.starts_with(b"event:") || prefix.starts_with(b"data:"),
        &source,
    )
}

#[cfg(test)]
mod tests;
