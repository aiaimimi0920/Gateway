//! Shared HTTP header construction, status classification, and media response reads.

use std::collections::HashMap;
use std::time::Duration;

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::headers::build_upstream_headers_with;
use rquest::header::HeaderMap;
use rquest::{Client, Method};
use serde_json::Value;

use super::super::execution::GEMINI_API_LEGACY_ADAPTER;

pub(super) fn build_official_headers(
    payload: &ProviderAccountPayload,
    extra_headers: Option<&HashMap<String, String>>,
) -> HeaderMap {
    let mut api_payload = payload.clone();
    api_payload.adapter = GEMINI_API_LEGACY_ADAPTER.to_string();
    build_upstream_headers_with(&api_payload, extra_headers)
}

pub(super) async fn send_official_json(
    http: &Client,
    payload: &ProviderAccountPayload,
    url: &str,
    body: &Value,
    timeout: Duration,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    let response = http
        .request(Method::POST, url)
        .headers(build_official_headers(payload, extra_headers))
        .timeout(timeout)
        .json(body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some(payload.adapter.as_str()),
        ));
    }
    serde_json::from_str::<Value>(&body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini official API returned invalid JSON: {error}"
        ))
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_invalid_json")
    })
}

pub(super) async fn send_official_get_json(
    http: &Client,
    payload: &ProviderAccountPayload,
    url: &str,
    timeout: Duration,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    let response = http
        .request(Method::GET, url)
        .headers(build_official_headers(payload, extra_headers))
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some(payload.adapter.as_str()),
        ));
    }
    serde_json::from_str::<Value>(&body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini official API returned invalid JSON: {error}"
        ))
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_invalid_json")
    })
}

pub(super) async fn send_official_get_bytes(
    http: &Client,
    payload: &ProviderAccountPayload,
    url: &str,
    timeout: Duration,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<(bytes::Bytes, Option<String>), GatewayError> {
    let response = http
        .request(Method::GET, url)
        .headers(build_official_headers(payload, extra_headers))
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = response
        .bytes()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &String::from_utf8_lossy(&bytes),
            Some(payload.adapter.as_str()),
        ));
    }
    Ok((bytes, content_type))
}
