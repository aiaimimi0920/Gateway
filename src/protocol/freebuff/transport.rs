use super::session_response::classify_auth_error;
use super::FreeBuffRuntimeConfig;
use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::upstream_body::collect_bounded_upstream_text_with_provider;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::headers::build_upstream_headers_with;
use rquest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use rquest::{Client, Method};
use serde_json::{json, Value};
use std::collections::HashMap;

pub(super) async fn start_run(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
) -> Result<String, GatewayError> {
    let response = send_run_action(
        client,
        config,
        &config.start_run_path,
        json!({
            "action": "START",
            "agentId": config.agent_id,
        }),
    )
    .await?;
    let status = response.status().as_u16();
    let body_text = collect_bounded_upstream_text_with_provider(
        response,
        "FreeBuff START response",
        "freebuff_compatible",
    )
    .await?;
    if status < 200 || status >= 300 {
        if matches!(status, 401 | 403) {
            return Err(classify_auth_error(&body_text));
        }
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some("freebuff_compatible"),
        ));
    }

    let body: Value = serde_json::from_str(&body_text).map_err(|error| {
        GatewayError::server_error(format!("decode FreeBuff START run response: {error}"))
            .with_code("freebuff_invalid_start_run_response")
            .with_provider("freebuff_compatible")
    })?;
    let run_id = body
        .get("runId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error("FreeBuff START run response missing runId")
                .with_code("freebuff_missing_run_id")
                .with_provider("freebuff_compatible")
        })?;
    Ok(run_id.to_string())
}

pub(super) async fn finish_run(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
    run_id: &str,
    request_count: u64,
) -> Result<(), GatewayError> {
    let response = send_run_action(
        client,
        config,
        &config.finish_run_path,
        json!({
            "action": "FINISH",
            "runId": run_id,
            "status": "completed",
            "totalSteps": request_count,
            "directCredits": 0,
            "totalCredits": 0,
        }),
    )
    .await?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        let body_text = collect_bounded_upstream_text_with_provider(
            response,
            "FreeBuff FINISH error response",
            "freebuff_compatible",
        )
        .await?;
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some("freebuff_compatible"),
        ));
    }
    Ok(())
}

async fn send_run_action(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
    path: &str,
    body: Value,
) -> Result<rquest::Response, GatewayError> {
    let headers = build_runtime_headers(config);
    client
        .request(Method::POST, build_absolute_url(&config.base_url, path))
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some("freebuff_compatible")))
}

pub(super) async fn send_chat_request(
    client: &Client,
    base_url: &str,
    payload: &ProviderAccountPayload,
    path: &str,
    body: &Value,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<rquest::Response, GatewayError> {
    let headers = build_upstream_headers_with(payload, extra_headers);
    client
        .request(Method::POST, build_absolute_url(base_url, path))
        .headers(headers)
        .json(body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some("freebuff_compatible")))
}

pub(super) fn build_runtime_headers(config: &FreeBuffRuntimeConfig) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", config.auth_token)) {
        headers.insert(AUTHORIZATION, value);
    }
    if let Ok(value) = HeaderValue::from_str(config.user_agent.as_str()) {
        headers.insert(USER_AGENT, value);
    }
    headers
}

pub(super) fn build_absolute_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    let normalized_path = if path.trim().starts_with('/') {
        path.trim().to_string()
    } else {
        format!("/{}", path.trim())
    };
    format!("{base}{normalized_path}")
}
