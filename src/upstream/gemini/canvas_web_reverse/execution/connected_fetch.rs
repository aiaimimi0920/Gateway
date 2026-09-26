//! Execute connected fetches and preserve the signed-mode to proxy fallback.

use std::time::Duration;

use base64::Engine;
use rquest::{Client, Method};
use serde_json::{json, Value};
use tracing::debug;

use crate::error::{classify_network_error, GatewayError};

use super::super::browser_operation::build_connected_fetch_invocation_input;
use super::super::result::{
    parse_connected_fetch_invocation_response, parse_connected_fetch_json_body,
};

pub async fn execute_connected_fetch_json_with_mode(
    http: &Client,
    timeout: Duration,
    provider: &str,
    browser_pool_base_url: &str,
    base_url: &str,
    share_id: &str,
    runtime_state_object_key: &str,
    browser_cdp_url: Option<&str>,
    cookie_header: Option<&str>,
    request_url: &str,
    request_body: &Value,
    google_fetch_mode: &str,
) -> Result<Value, GatewayError> {
    let input = build_connected_fetch_invocation_input(
        base_url,
        share_id,
        runtime_state_object_key,
        browser_cdp_url,
        cookie_header,
        request_url,
        request_body,
        google_fetch_mode,
        timeout,
    );
    let response = http
        .request(Method::POST, format!("{}/fetch", browser_pool_base_url))
        .header(rquest::header::CONTENT_TYPE, "application/json")
        .timeout(timeout.max(Duration::from_secs(30)))
        .json(&input)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;

    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let invocation = parse_connected_fetch_invocation_response(provider, status, &body_text)?;
    parse_connected_fetch_json_body(provider, invocation.body_text.as_deref())
}

pub async fn execute_connected_fetch_get_bytes_with_mode(
    http: &Client,
    timeout: Duration,
    provider: &str,
    browser_pool_base_url: &str,
    base_url: &str,
    share_id: &str,
    runtime_state_object_key: &str,
    browser_cdp_url: Option<&str>,
    cookie_header: Option<&str>,
    request_url: &str,
    google_fetch_mode: &str,
) -> Result<(bytes::Bytes, Option<String>), GatewayError> {
    let input = json!({
        "baseUrl": base_url,
        "shareId": share_id,
        "runtimeStateObjectKey": runtime_state_object_key,
        "browserCdpUrl": browser_cdp_url,
        "cookieHeader": cookie_header,
        "requireAppPage": base_url.contains("gemini.google.com"),
        "googleFetchMode": google_fetch_mode,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "fetchRequest": {
            "url": request_url,
            "method": "GET",
            "headers": {},
            "referrer": format!("{}/share/{}", base_url.trim_end_matches('/'), share_id),
            "referrerPolicy": "strict-origin-when-cross-origin"
        }
    });
    let response = http
        .request(Method::POST, format!("{}/fetch", browser_pool_base_url))
        .header(rquest::header::CONTENT_TYPE, "application/json")
        .timeout(timeout.max(Duration::from_secs(30)))
        .json(&input)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;

    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let invocation = parse_connected_fetch_invocation_response(provider, status, &body_text)?;
    let bytes = if let Some(body_base64) = invocation.body_base64.as_deref() {
        bytes::Bytes::from(
            base64::engine::general_purpose::STANDARD
                .decode(body_base64)
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "Gemini Canvas connected fetch returned invalid base64 bytes: {error}"
                    ))
                    .with_provider(provider)
                    .with_code("gemini_canvas_connected_fetch_invalid_base64")
                })?,
        )
    } else if let Some(body_text) = invocation.body_text.as_ref() {
        bytes::Bytes::from(body_text.clone().into_bytes())
    } else {
        return Err(GatewayError::server_error(
            "Gemini Canvas connected fetch GET completed without a body payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_missing_body"));
    };
    Ok((bytes, invocation.content_type))
}

pub async fn execute_connected_fetch_get_bytes(
    http: &Client,
    timeout: Duration,
    provider: &str,
    browser_pool_base_url: &str,
    base_url: &str,
    share_id: &str,
    runtime_state_object_key: &str,
    browser_cdp_url: Option<&str>,
    cookie_header: Option<&str>,
    request_url: &str,
) -> Result<(bytes::Bytes, Option<String>), GatewayError> {
    match execute_connected_fetch_get_bytes_with_mode(
        http,
        timeout,
        provider,
        browser_pool_base_url,
        base_url,
        share_id,
        runtime_state_object_key,
        browser_cdp_url,
        cookie_header,
        request_url,
        "google_signed",
    )
    .await
    {
        Ok(body) => Ok(body),
        Err(primary_error) => {
            debug!(
                provider,
                message = %primary_error.message,
                code = ?primary_error.code,
                status = ?primary_error.http_status,
                "gemini canvas connected fetch GET failed in primary google-signed mode; retrying canvas proxy mode"
            );
            execute_connected_fetch_get_bytes_with_mode(
                http,
                timeout,
                provider,
                browser_pool_base_url,
                base_url,
                share_id,
                runtime_state_object_key,
                browser_cdp_url,
                cookie_header,
                request_url,
                "canvas_proxy",
            )
            .await
        }
    }
}

pub async fn execute_connected_fetch_json(
    http: &Client,
    timeout: Duration,
    provider: &str,
    browser_pool_base_url: &str,
    base_url: &str,
    share_id: &str,
    runtime_state_object_key: &str,
    browser_cdp_url: Option<&str>,
    cookie_header: Option<&str>,
    request_url: &str,
    request_body: &Value,
) -> Result<Value, GatewayError> {
    match execute_connected_fetch_json_with_mode(
        http,
        timeout,
        provider,
        browser_pool_base_url,
        base_url,
        share_id,
        runtime_state_object_key,
        browser_cdp_url,
        cookie_header,
        request_url,
        request_body,
        "google_signed",
    )
    .await
    {
        Ok(body) => Ok(body),
        Err(primary_error) => {
            debug!(
                provider,
                message = %primary_error.message,
                code = ?primary_error.code,
                status = ?primary_error.http_status,
                "gemini canvas connected fetch failed in primary google-signed mode; retrying canvas proxy mode"
            );
            execute_connected_fetch_json_with_mode(
                http,
                timeout,
                provider,
                browser_pool_base_url,
                base_url,
                share_id,
                runtime_state_object_key,
                browser_cdp_url,
                cookie_header,
                request_url,
                request_body,
                "canvas_proxy",
            )
            .await
        }
    }
}
