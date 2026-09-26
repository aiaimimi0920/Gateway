use std::collections::HashMap;
use std::time::Duration;

use rquest::{Client, Method};
use serde_json::Value;
use tracing::debug;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::chatgpt::web_reverse as surface;
use crate::protocol::upstream_body::collect_bounded_upstream_charset_text_with_provider;
use crate::routing::candidate::ProviderAccountPayload;

use super::super::common::{classify_chatgpt_web_text_response, insert_header_map_value};
use super::super::headers::build_request_headers;
use super::super::web_reverse::{build_target_url, ChatGptWebRequestContext};
use super::super::PROVIDER;

pub(super) async fn post_json(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request_context: &ChatGptWebRequestContext,
    extra_headers: Option<&HashMap<String, String>>,
    path: &str,
    body: &Value,
    requirements: Option<&surface::ChatRequirements>,
    stream: bool,
    turn_trace_id: Option<&str>,
) -> Result<rquest::Response, GatewayError> {
    let url = build_target_url(request_context, path);
    let accept = if stream {
        "text/event-stream"
    } else {
        "application/json"
    };
    let mut headers = build_request_headers(
        request_context,
        payload,
        extra_headers,
        path,
        accept,
        Some("application/json"),
    );
    strip_request_headers_for_path(&mut headers, path);
    if let Some(requirements) = requirements {
        insert_header_map_value(
            &mut headers,
            "OpenAI-Sentinel-Chat-Requirements-Token",
            &requirements.token,
        );
        if let Some(proof_token) = requirements.proof_token.as_deref() {
            insert_header_map_value(&mut headers, "OpenAI-Sentinel-Proof-Token", proof_token);
        }
        if let Some(turnstile_token) = requirements.turnstile_token.as_deref() {
            insert_header_map_value(
                &mut headers,
                "OpenAI-Sentinel-Turnstile-Token",
                turnstile_token,
            );
        }
        if let Some(so_token) = requirements.so_token.as_deref() {
            insert_header_map_value(&mut headers, "OpenAI-Sentinel-SO-Token", so_token);
        }
    }
    if let Some(turn_trace_id) = turn_trace_id {
        insert_header_map_value(&mut headers, "X-OAI-Turn-Trace-Id", turn_trace_id);
    }

    let response = http
        .request(Method::POST, &url)
        .headers(headers)
        .timeout(timeout.max(Duration::from_secs(if stream { 180 } else { 60 })))
        .json(body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    debug!(
        path = %path,
        status = response.status().as_u16(),
        content_type = ?response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        "chatgpt web post_json response"
    );
    Ok(response)
}

pub(super) async fn post_prepare(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request_context: &ChatGptWebRequestContext,
    extra_headers: Option<&HashMap<String, String>>,
    path: &str,
    turn_trace_id: &str,
) -> Result<(), GatewayError> {
    let url = build_target_url(request_context, path);
    let mut headers = build_request_headers(
        request_context,
        payload,
        extra_headers,
        path,
        "application/json",
        Some("application/json"),
    );
    strip_request_headers_for_path(&mut headers, path);
    insert_header_map_value(&mut headers, "X-OAI-Turn-Trace-Id", turn_trace_id);
    let response = http
        .request(Method::POST, &url)
        .headers(headers)
        .timeout(timeout.max(Duration::from_secs(60)))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = collect_bounded_upstream_charset_text_with_provider(
        response,
        "ChatGPT Web reverse prepare body",
        PROVIDER,
    )
    .await?;
    debug!(
        path = %path,
        status,
        content_type = ?content_type,
        "chatgpt web prepare response"
    );
    classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)
}

fn strip_request_headers_for_path(headers: &mut rquest::header::HeaderMap, path: &str) {
    if matches!(
        path,
        surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH
            | surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PREPARE_PATH
    ) {
        for name in [
            "cookie",
            "origin",
            "cache-control",
            "pragma",
            "priority",
            "sec-fetch-dest",
            "sec-fetch-mode",
            "sec-fetch-site",
            "sec-ch-ua-arch",
            "sec-ch-ua-bitness",
            "sec-ch-ua-full-version",
            "sec-ch-ua-full-version-list",
            "sec-ch-ua-model",
            "sec-ch-ua-platform-version",
        ] {
            headers.remove(name);
        }
    }
    if !matches!(
        path,
        surface::CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH
            | surface::CHATGPT_WEB_DEFAULT_CONVERSATION_PATH
    ) {
        return;
    }
    for name in [
        "x-oai-is",
        "x-conduit-token",
        "oai-telemetry",
        "oai-echo-logs",
    ] {
        headers.remove(name);
    }
}
