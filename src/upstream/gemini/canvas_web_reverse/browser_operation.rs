use std::collections::HashMap;
use std::time::Duration;

use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::gemini::canvas_web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::browser_executor_helpers::{
    missing_browser_executor_field_error, read_json_u64,
};
use crate::upstream::browser_worker_types::GeminiCanvasHttpReplayWorkerInput;
use crate::upstream::upstream_payload_helpers::read_json_string;

use super::payload::relay_config_from_payload;

#[derive(Debug)]
pub(crate) struct PreparedGeminiCanvasBrowserExecutorServiceInput {
    pub(crate) base_url: String,
    pub(crate) share_id: String,
    pub(crate) runtime_state_object_key: String,
    pub(crate) browser_cdp_url: Option<String>,
    pub(crate) cookie_header: Option<String>,
    pub(crate) operation: String,
    pub(crate) prompt: String,
    pub(crate) locale: String,
    pub(crate) timeout: Duration,
}

pub fn build_browser_executor_invocation_input(
    payload: &ProviderAccountPayload,
    request_spec: &surface::GeminiCanvasBrowserRelayRequestSpec,
) -> Result<Value, GatewayError> {
    let relay = relay_config_from_payload(payload)?;
    Ok(surface::build_browser_executor_invocation_input(
        &relay,
        request_spec,
    ))
}

pub fn build_browser_operation_invocation_input(
    payload: &ProviderAccountPayload,
    operation: &str,
    prompt: &str,
    locale: &str,
    timeout: Duration,
) -> Result<Value, GatewayError> {
    let relay = relay_config_from_payload(payload)?;
    let browser_runtime_state_object_key =
        crate::protocol::gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
            payload, operation,
        )
        .unwrap_or_else(|| relay.runtime_state_object_key.clone());
    let browser_cdp_url = crate::protocol::gemini_canvas::browser_cdp_url(payload);
    let cookie_header = crate::protocol::gemini_canvas::browser_cookie_header(payload);
    Ok(build_browser_operation_invocation_input_from_values(
        payload.base_url.trim_end_matches('/'),
        &relay.share_id,
        &browser_runtime_state_object_key,
        browser_cdp_url.as_deref(),
        cookie_header.as_deref(),
        operation,
        prompt,
        locale,
        timeout,
    ))
}

pub fn build_browser_operation_invocation_input_from_values(
    base_url: &str,
    share_id: &str,
    runtime_state_object_key: &str,
    browser_cdp_url: Option<&str>,
    cookie_header: Option<&str>,
    operation: &str,
    prompt: &str,
    locale: &str,
    timeout: Duration,
) -> Value {
    json!({
        "baseUrl": base_url,
        "shareId": share_id,
        "runtimeStateObjectKey": runtime_state_object_key,
        "browserCdpUrl": browser_cdp_url,
        "cookieHeader": cookie_header,
        "requireAppPage": base_url.contains("gemini.google.com"),
        "operation": operation,
        "prompt": prompt,
        "locale": locale,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "browserExecutablePath": std::env::var("GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH").ok(),
    })
}

pub fn build_connected_fetch_invocation_input(
    base_url: &str,
    share_id: &str,
    runtime_state_object_key: &str,
    browser_cdp_url: Option<&str>,
    cookie_header: Option<&str>,
    request_url: &str,
    request_body: &Value,
    google_fetch_mode: &str,
    timeout: Duration,
) -> Value {
    json!({
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
            "method": "POST",
            "headers": {
                "Content-Type": "application/json",
            },
            "referrer": format!("{}/share/{}", base_url.trim_end_matches('/'), share_id),
            "referrerPolicy": "strict-origin-when-cross-origin",
            "jsonBody": request_body,
        }
    })
}

pub(crate) fn build_connected_fetch_form_invocation_input(
    base_url: &str,
    share_id: &str,
    runtime_state_object_key: &str,
    browser_cdp_url: Option<&str>,
    cookie_header: Option<&str>,
    request_url: &str,
    headers: &HashMap<String, String>,
    body_text: &str,
    referrer: &str,
    timeout: Duration,
) -> Value {
    json!({
        "baseUrl": base_url,
        "shareId": share_id,
        "runtimeStateObjectKey": runtime_state_object_key,
        "browserCdpUrl": browser_cdp_url,
        "cookieHeader": cookie_header,
        "requireAppPage": base_url.contains("gemini.google.com"),
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "fetchRequest": {
            "url": request_url,
            "method": "POST",
            "headers": headers,
            "bodyText": body_text,
            "referrer": referrer,
            "referrerPolicy": "strict-origin-when-cross-origin",
        }
    })
}

pub(crate) fn build_http_replay_worker_input<'a>(
    url: &'a str,
    query: &'a [(String, String)],
    headers: &'a HashMap<String, String>,
    raw_post_data: &'a str,
    cookie_header: &'a str,
    operation: Option<&'a str>,
    timeout: Duration,
) -> GeminiCanvasHttpReplayWorkerInput<'a> {
    GeminiCanvasHttpReplayWorkerInput {
        url,
        query,
        headers,
        raw_post_data,
        cookie_header,
        operation,
        timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
    }
}

pub(crate) fn prepare_gemini_canvas_browser_executor_service_input(
    input: &Value,
) -> Result<PreparedGeminiCanvasBrowserExecutorServiceInput, GatewayError> {
    let base_url = read_json_string(input, "baseUrl").ok_or_else(|| {
        missing_browser_executor_field_error(
            "Gemini Canvas",
            "baseUrl",
            "browser_executor_missing_base_url",
        )
    })?;
    let share_id = read_json_string(input, "shareId").ok_or_else(|| {
        missing_browser_executor_field_error(
            "Gemini Canvas",
            "shareId",
            "browser_executor_missing_share_id",
        )
    })?;
    let runtime_state_object_key =
        read_json_string(input, "runtimeStateObjectKey").ok_or_else(|| {
            missing_browser_executor_field_error(
                "Gemini Canvas",
                "runtimeStateObjectKey",
                "browser_executor_missing_runtime_state",
            )
        })?;
    let operation = read_json_string(input, "operation").ok_or_else(|| {
        missing_browser_executor_field_error(
            "Gemini Canvas",
            "operation",
            "browser_executor_missing_operation",
        )
    })?;
    let prompt = read_json_string(input, "prompt").ok_or_else(|| {
        missing_browser_executor_field_error(
            "Gemini Canvas",
            "prompt",
            "browser_executor_missing_prompt",
        )
    })?;

    Ok(PreparedGeminiCanvasBrowserExecutorServiceInput {
        base_url,
        share_id,
        runtime_state_object_key,
        browser_cdp_url: read_json_string(input, "browserCdpUrl"),
        cookie_header: read_json_string(input, "cookieHeader"),
        operation,
        prompt,
        locale: read_json_string(input, "locale").unwrap_or_else(|| "en-US".to_string()),
        timeout: Duration::from_millis(read_json_u64(input, "timeoutMs").unwrap_or(240_000)),
    })
}
