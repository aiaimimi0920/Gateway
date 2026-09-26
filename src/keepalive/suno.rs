//! Suno browser-cookie material selection and ordered runtime probes.

use super::auth_material::decode_jwt_expiry_iso;
use super::headers::{
    build_keepalive_probe_headers, read_header_case_insensitive, request_builder_with_headers,
    upsert_header_case_insensitive,
};
use super::metadata::truncate_error_summary;
use super::{GatewayKeepaliveEnsureRequest, GatewayKeepaliveEnsureResponse};
use crate::{credential_runtime::SessionAuthConfig, error::GatewayError};
use rquest::Client;
use serde_json::{json, Value};

pub(super) fn extract_cookie_value(cookie_header: &str, cookie_name: &str) -> Option<String> {
    cookie_header
        .split(';')
        .filter_map(|entry| {
            let (name, value) = entry.trim().split_once('=')?;
            Some((name.trim(), value.trim()))
        })
        .find(|(name, value)| *name == cookie_name && !value.is_empty())
        .map(|(_, value)| value.to_string())
}

pub(super) fn read_suno_cookie_header(input: &GatewayKeepaliveEnsureRequest) -> Option<String> {
    read_header_case_insensitive(&input.headers, "cookie").or_else(|| {
        input
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| value.contains('=') && !value.is_empty())
            .map(str::to_string)
    })
}

pub(super) fn suno_cookie_header_from_storage_state(state: &Value) -> Option<String> {
    let cookies = state.get("cookies")?.as_array()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0);
    let mut selected = std::collections::BTreeMap::<String, (usize, f64, String)>::new();
    for cookie in cookies {
        let Some(domain) = cookie.get("domain").and_then(Value::as_str) else {
            continue;
        };
        let normalized_domain = domain.trim_start_matches('.').to_ascii_lowercase();
        let is_suno_domain = normalized_domain == "suno.com"
            || normalized_domain.ends_with(".suno.com")
            || normalized_domain == "suno.ai"
            || normalized_domain.ends_with(".suno.ai");
        if !is_suno_domain {
            continue;
        }
        let Some(name) = cookie.get("name").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        let Some(value) = cookie.get("value").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if name.is_empty() || value.is_empty() {
            continue;
        }
        let expires = cookie
            .get("expires")
            .and_then(Value::as_f64)
            .unwrap_or(-1.0);
        if expires > 0.0 && expires <= now {
            continue;
        }
        let path_len = cookie
            .get("path")
            .and_then(Value::as_str)
            .map(str::len)
            .unwrap_or(0);
        let replace = selected
            .get(name)
            .map(|(current_path_len, current_expires, _)| {
                path_len > *current_path_len
                    || (path_len == *current_path_len && expires > *current_expires)
            })
            .unwrap_or(true);
        if replace {
            selected.insert(name.to_string(), (path_len, expires, value.to_string()));
        }
    }
    let cookie_header = selected
        .into_iter()
        .map(|(name, (_, _, value))| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ");
    (!cookie_header.is_empty()).then_some(cookie_header)
}

async fn resolve_suno_cookie_header(input: &GatewayKeepaliveEnsureRequest) -> Option<String> {
    if let Some(object_key) = input
        .runtime_state_object_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Ok(storage) = crate::object_storage::gateway_object_storage() {
            if let Ok(state) = storage.read_json(object_key).await {
                if let Some(cookie_header) = suno_cookie_header_from_storage_state(&state) {
                    if extract_cookie_value(&cookie_header, "__session").is_some() {
                        return Some(cookie_header);
                    }
                }
            }
        }
    }
    read_suno_cookie_header(input)
}

pub(super) async fn ensure_runtime(
    http: &Client,
    input: &GatewayKeepaliveEnsureRequest,
    mut effective_session_auth: Option<SessionAuthConfig>,
    effective_expires_at: Option<String>,
    requested_runtime_state_object_key: Option<String>,
) -> Result<GatewayKeepaliveEnsureResponse, GatewayError> {
    let Some(cookie_header) = resolve_suno_cookie_header(&input).await else {
        return Ok(GatewayKeepaliveEnsureResponse {
            ready: false,
            message: Some(
                "Suno credentials require the raw browser Cookie header as the long-lived credential material."
                    .to_string(),
            ),
            api_key: input.api_key.clone(),
            headers: Some(input.headers.clone()),
            extra_body: input.extra_body.clone(),
            session_auth: effective_session_auth,
            keepalive: None,
            expires_at: effective_expires_at,
            runtime_state_object_key: requested_runtime_state_object_key,
            upstream_session_id: None,
        });
    };
    let Some(session_token) = extract_cookie_value(&cookie_header, "__session") else {
        return Ok(GatewayKeepaliveEnsureResponse {
            ready: false,
            message: Some(
                "Suno Cookie material is missing the __session access token and must be refreshed from a signed-in browser."
                    .to_string(),
            ),
            api_key: input.api_key.clone(),
            headers: Some(input.headers.clone()),
            extra_body: input.extra_body.clone(),
            session_auth: effective_session_auth,
            keepalive: None,
            expires_at: effective_expires_at,
            runtime_state_object_key: requested_runtime_state_object_key,
            upstream_session_id: None,
        });
    };

    let runtime_expires_at = decode_jwt_expiry_iso(Some(session_token.as_str()))
        .or_else(|| effective_expires_at.clone());

    if let Some(config) = effective_session_auth.as_mut() {
        if config.expires_at.is_none() {
            config.expires_at = runtime_expires_at.clone();
        }
    }

    let mut runtime_headers = input.headers.clone();
    upsert_header_case_insensitive(&mut runtime_headers, "Cookie", cookie_header.clone());
    if read_header_case_insensitive(&runtime_headers, "Device-Id").is_none() {
        if let Some(device_id) = extract_cookie_value(&cookie_header, "ajs_anonymous_id") {
            upsert_header_case_insensitive(&mut runtime_headers, "Device-Id", device_id);
        }
    }
    upsert_header_case_insensitive(&mut runtime_headers, "Referring-Pathname", "/");
    upsert_header_case_insensitive(&mut runtime_headers, "Referring-Origin", "https://suno.com");

    let mut probe_headers = build_keepalive_probe_headers(
        Some(&runtime_headers),
        effective_session_auth.as_ref(),
        &session_token,
    );
    upsert_header_case_insensitive(&mut probe_headers, "Cookie", cookie_header.clone());
    if let Some(device_id) = read_header_case_insensitive(&runtime_headers, "Device-Id") {
        upsert_header_case_insensitive(&mut probe_headers, "Device-Id", device_id);
    }
    upsert_header_case_insensitive(&mut probe_headers, "Referring-Pathname", "/");
    upsert_header_case_insensitive(&mut probe_headers, "Referring-Origin", "https://suno.com");

    let user_config_response = request_builder_with_headers(
        http,
        rquest::Method::POST,
        &format!(
            "{}/api/user/user_config/",
            input.base_url.trim_end_matches('/')
        ),
        &probe_headers,
    )
    .header("Content-Type", "application/json")
    .body("{}")
    .send()
    .await;

    match user_config_response {
        Ok(response)
            if response.status().is_success()
                || (response.status().is_client_error()
                    && response.status().as_u16() != 401
                    && response.status().as_u16() != 403) => {}
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Ok(GatewayKeepaliveEnsureResponse {
                ready: false,
                message: Some(if body.trim().is_empty() {
                    format!(
                        "Suno user_config probe failed with HTTP {}; session must be refreshed externally.",
                        status
                    )
                } else {
                    truncate_error_summary(&body, 1_000)
                }),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: runtime_expires_at,
                runtime_state_object_key: requested_runtime_state_object_key,
                upstream_session_id: None,
            });
        }
        Err(error) => {
            return Ok(GatewayKeepaliveEnsureResponse {
                ready: false,
                message: Some(format!("Suno user_config probe failed: {error}")),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: runtime_expires_at,
                runtime_state_object_key: requested_runtime_state_object_key,
                upstream_session_id: None,
            });
        }
    }

    let challenge_response = request_builder_with_headers(
        http,
        rquest::Method::POST,
        &format!("{}/api/c/check", input.base_url.trim_end_matches('/')),
        &probe_headers,
    )
    .header("Content-Type", "application/json")
    .json(&json!({ "ctype": "generation" }))
    .send()
    .await;

    match challenge_response {
        Ok(response) if response.status().is_success() => {
            let body = response.json::<Value>().await.map_err(|error| {
                GatewayError::server_error(format!("Suno challenge probe JSON 解析失败: {error}"))
            })?;
            if body.get("required").and_then(Value::as_bool).is_none() {
                return Ok(GatewayKeepaliveEnsureResponse {
                    ready: false,
                    message: Some(
                        "Suno challenge probe response did not include the required flag."
                            .to_string(),
                    ),
                    api_key: input.api_key.clone(),
                    headers: Some(input.headers.clone()),
                    extra_body: input.extra_body.clone(),
                    session_auth: effective_session_auth,
                    keepalive: None,
                    expires_at: runtime_expires_at,
                    runtime_state_object_key: requested_runtime_state_object_key,
                    upstream_session_id: None,
                });
            }
        }
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Ok(GatewayKeepaliveEnsureResponse {
                ready: false,
                message: Some(if body.trim().is_empty() {
                    format!(
                        "Suno challenge probe failed with HTTP {}; session must be refreshed externally.",
                        status
                    )
                } else {
                    truncate_error_summary(&body, 1_000)
                }),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: runtime_expires_at,
                runtime_state_object_key: requested_runtime_state_object_key,
                upstream_session_id: None,
            });
        }
        Err(error) => {
            return Ok(GatewayKeepaliveEnsureResponse {
                ready: false,
                message: Some(format!("Suno challenge probe failed: {error}")),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: runtime_expires_at,
                runtime_state_object_key: requested_runtime_state_object_key,
                upstream_session_id: None,
            });
        }
    }

    return Ok(GatewayKeepaliveEnsureResponse {
        ready: true,
        message: Some("Suno runtime material is ready.".to_string()),
        api_key: Some(session_token),
        headers: Some(runtime_headers),
        extra_body: input.extra_body.clone(),
        session_auth: effective_session_auth,
        keepalive: None,
        expires_at: runtime_expires_at,
        runtime_state_object_key: requested_runtime_state_object_key,
        upstream_session_id: None,
    });
}
