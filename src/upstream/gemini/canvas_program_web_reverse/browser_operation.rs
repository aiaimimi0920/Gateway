use std::collections::HashMap;

use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::gemini::canvas_program_web_reverse as surface;
use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;

use super::payload::relay_config_from_payload;

pub fn build_browser_operation_invocation_input(
    payload: &ProviderAccountPayload,
    operation: &str,
    prompt: &str,
    locale: &str,
    timeout: std::time::Duration,
) -> Result<Value, GatewayError> {
    let config = relay_config_from_payload(payload)?;
    let runtime_state_object_key =
        gemini_canvas::browser_runtime_state_object_key_for_browser_operation(payload, operation)
            .unwrap_or_else(|| config.bootstrap.runtime_state_object_key.clone());
    let mut input = build_browser_operation_invocation_input_from_config(
        payload.base_url.trim_end_matches('/'),
        &config,
        &runtime_state_object_key,
        operation,
        prompt,
        locale,
        timeout,
    );
    input["authUser"] = Value::String(crate::protocol::gemini_canvas::direct_http_auth_user(
        payload,
    ));
    Ok(input)
}

pub fn build_browser_operation_invocation_input_from_config(
    base_url: &str,
    config: &surface::GeminiCanvasProgramRelayConfig,
    runtime_state_object_key: &str,
    operation: &str,
    prompt: &str,
    locale: &str,
    timeout: std::time::Duration,
) -> Value {
    json!({
        "baseUrl": base_url.trim_end_matches('/'),
        "shareId": config.bootstrap.share_id,
        "runtimeStateObjectKey": runtime_state_object_key,
        "enforceProgramOwner": true,
        "operation": operation,
        "prompt": prompt,
        "locale": locale,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "canvasProgramHint": config.bootstrap.canvas_program_hint,
        "canvasProgramUrl": config.app_endpoint.canvas_program_url,
        "pageUrl": config.app_endpoint.page_url,
        "appPath": config.app_endpoint.app_path,
        "conversationId": config.app_endpoint.conversation_id,
        "responseId": config.app_endpoint.response_id,
        "browserExecutablePath": std::env::var("GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH").ok(),
    })
}

pub fn build_connected_fetch_invocation_input(
    base_url: &str,
    config: &surface::GeminiCanvasProgramRelayConfig,
    request_url: &str,
    request_body: &Value,
    google_fetch_mode: &str,
    timeout: std::time::Duration,
) -> Value {
    build_connected_fetch_invocation_input_with_method(
        base_url,
        config,
        request_url,
        "POST",
        Some(request_body),
        google_fetch_mode,
        timeout,
    )
}

pub fn build_connected_fetch_invocation_input_with_method(
    base_url: &str,
    config: &surface::GeminiCanvasProgramRelayConfig,
    request_url: &str,
    method: &str,
    request_body: Option<&Value>,
    google_fetch_mode: &str,
    timeout: std::time::Duration,
) -> Value {
    let trimmed_base_url = base_url.trim_end_matches('/');
    let require_app_page = trimmed_base_url.contains("gemini.google.com");
    let referrer = config
        .app_endpoint
        .canvas_program_url
        .clone()
        .unwrap_or_else(|| format!("{trimmed_base_url}/share/{}", config.bootstrap.share_id));
    json!({
        "baseUrl": trimmed_base_url,
        "shareId": config.bootstrap.share_id,
        "runtimeStateObjectKey": config.bootstrap.runtime_state_object_key,
        "requireAppPage": require_app_page,
        "enforceProgramOwner": true,
        "googleFetchMode": google_fetch_mode,
        "canvasProgramHint": config.bootstrap.canvas_program_hint,
        "canvasProgramUrl": config.app_endpoint.canvas_program_url,
        "pageUrl": config.app_endpoint.page_url,
        "appPath": config.app_endpoint.app_path,
        "conversationId": config.app_endpoint.conversation_id,
        "responseId": config.app_endpoint.response_id,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "fetchRequest": {
            "url": request_url,
            "method": method,
            "headers": {
                "Content-Type": "application/json",
            },
            "referrer": referrer,
            "referrerPolicy": "strict-origin-when-cross-origin",
            "jsonBody": request_body,
        }
    })
}

pub fn build_connected_fetch_invocation_input_with_method_for_payload(
    payload: &ProviderAccountPayload,
    base_url: &str,
    config: &surface::GeminiCanvasProgramRelayConfig,
    request_url: &str,
    method: &str,
    request_body: Option<&Value>,
    google_fetch_mode: &str,
    timeout: std::time::Duration,
) -> Value {
    let mut input = build_connected_fetch_invocation_input_with_method(
        base_url,
        config,
        request_url,
        method,
        request_body,
        google_fetch_mode,
        timeout,
    );
    let runtime_state_object_key =
        gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
            payload,
            "bootstrap_program",
        )
        .unwrap_or_else(|| config.bootstrap.runtime_state_object_key.clone());
    input["runtimeStateObjectKey"] = Value::String(runtime_state_object_key);
    input
}

pub fn program_prefers_preview_no_key_generate_content_contract(
    config: &surface::GeminiCanvasProgramRelayConfig,
) -> bool {
    let Some(contract) = config.app_endpoint.canvas_program_invoke_contract.as_ref() else {
        return false;
    };
    matches!(
        contract.request_envelope_kind.as_deref(),
        Some("canvas_proxy_request")
    ) || matches!(
        contract.transport_kind.as_deref(),
        Some("canvas_program_ws_candidate")
    ) || contract.ws_url.is_some()
}

pub fn program_prefers_canvas_proxy_contract(
    config: &surface::GeminiCanvasProgramRelayConfig,
) -> bool {
    program_prefers_preview_no_key_generate_content_contract(config)
}

pub fn connected_fetch_mode_is_canvas_proxy(mode: &str) -> bool {
    mode.trim().eq_ignore_ascii_case("canvas_proxy")
}

pub fn connected_fetch_mode_is_canvas_preview_no_key(mode: &str) -> bool {
    mode.trim().eq_ignore_ascii_case("canvas_preview_no_key")
}

pub fn connected_fetch_mode_is_canvas_preview_music_no_key(mode: &str) -> bool {
    mode.trim()
        .eq_ignore_ascii_case("canvas_preview_music_no_key")
}

pub fn connected_fetch_mode_is_canvas_page_no_key(mode: &str) -> bool {
    mode.trim().eq_ignore_ascii_case("canvas_page_no_key")
}

pub fn connected_fetch_mode_is_canvas_page_music_no_key(mode: &str) -> bool {
    mode.trim().eq_ignore_ascii_case("canvas_page_music_no_key")
}

pub fn append_api_key_query_if_missing(
    request_url: &str,
    api_key: &str,
) -> Result<String, GatewayError> {
    let trimmed_key = api_key.trim();
    if trimmed_key.is_empty() {
        return Ok(request_url.to_string());
    }
    let Ok(mut parsed) = url::Url::parse(request_url) else {
        return Ok(request_url.to_string());
    };
    if parsed
        .query_pairs()
        .any(|(key, value)| key == "key" && !value.trim().is_empty())
    {
        return Ok(parsed.to_string());
    }
    parsed.query_pairs_mut().append_pair("key", trimmed_key);
    Ok(parsed.to_string())
}

pub fn apply_program_connected_fetch_identity_contract(
    input: &mut Value,
    auth_user: Option<&str>,
    api_key: Option<&str>,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<(), GatewayError> {
    let Some(fetch_request) = input.get_mut("fetchRequest").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    let current_request_url = fetch_request
        .get("url")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut effective_request_url = None::<String>;

    {
        let headers = fetch_request
            .entry("headers")
            .or_insert_with(|| Value::Object(serde_json::Map::new()))
            .as_object_mut()
            .ok_or_else(|| {
                GatewayError::server_error(
                    "Gemini Canvas connected fetch input had a non-object headers field.",
                )
                .with_provider("gemini_canvas_program_web_reverse_compatible")
                .with_code("gemini_canvas_connected_fetch_invalid_headers")
            })?;

        for (key, value) in extra_headers.into_iter().flat_map(|entry| entry.iter()) {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                continue;
            }
            headers.insert(key.clone(), Value::String(trimmed.to_string()));
        }

        if let Some(auth_user_value) = auth_user.map(str::trim).filter(|value| !value.is_empty()) {
            headers
                .entry("X-Goog-AuthUser".to_string())
                .or_insert_with(|| Value::String(auth_user_value.to_string()));
        }

        if let Some(api_key_value) = api_key.map(str::trim).filter(|value| !value.is_empty()) {
            headers.insert(
                "x-goog-api-key".to_string(),
                Value::String(api_key_value.to_string()),
            );
            if let Some(url_value) = current_request_url.as_deref() {
                effective_request_url =
                    Some(append_api_key_query_if_missing(url_value, api_key_value)?);
            }
        }

        headers
            .entry("Accept".to_string())
            .or_insert_with(|| Value::String("application/json".to_string()));
    }

    if let Some(url) = effective_request_url {
        fetch_request.insert("url".to_string(), Value::String(url));
    }

    Ok(())
}
