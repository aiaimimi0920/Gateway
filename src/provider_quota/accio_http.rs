use super::http_contract::{build_absolute_url, provider_quota_http_error};
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use rquest::Method;
use serde_json::Value;
use std::time::Duration;
pub(super) async fn fetch_accio_quota_endpoint(
    timeout_secs: u64,
    payload: &ProviderAccountPayload,
    path: &str,
    source: &str,
    probe: bool,
) -> Result<(String, Value), GatewayError> {
    let access_token = payload.api_key.trim();
    if access_token.is_empty() {
        return Err(
            GatewayError::bad_request("Accio provider quota probe 缺少 access token")
                .with_code("accio_quota_missing_access_token"),
        );
    }

    let utdid = payload
        .headers
        .get("utdid")
        .or_else(|| payload.headers.get("x-utdid"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::bad_request("Accio provider quota probe 缺少 utdid")
                .with_code("accio_quota_missing_utdid")
        })?;
    let version = payload
        .headers
        .get("version")
        .or_else(|| payload.headers.get("x-app-version"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("0.5.6");

    let client = crate::http_client::builder()
        .timeout(Duration::from_secs(timeout_secs.max(1)))
        .build()
        .map_err(|error| {
            GatewayError::server_error(format!("build accio quota client: {error}"))
        })?;
    let response = client
        .request(
            Method::GET,
            build_absolute_url(payload.base_url.trim_end_matches('/'), path),
        )
        .query(&[
            ("accessToken", access_token),
            ("utdid", utdid),
            ("version", version),
        ])
        .headers(build_accio_control_plane_headers(payload, true))
        .send()
        .await
        .map_err(|error| {
            let error = error.without_uri();
            GatewayError::service_unavailable(format!("Accio quota request failed: {error}"))
                .with_code("provider_quota_request_failed")
        })?;
    let status = response.status();
    let body = response.bytes().await.map_err(|error| {
        let error = error.without_uri();
        GatewayError::service_unavailable(format!("read Accio quota response: {error}"))
    })?;
    if !status.is_success() {
        return Err(provider_quota_http_error(
            if probe {
                "Accio quota probe"
            } else {
                "Accio currentSubscription"
            },
            status,
            &body,
        ));
    }
    let raw_data: Value = serde_json::from_slice(&body).map_err(|error| {
        GatewayError::server_error(format!("parse Accio quota body: {error}"))
            .with_code("provider_quota_parse_failed")
    })?;
    Ok((source.to_string(), raw_data))
}

pub(super) fn build_accio_control_plane_headers(
    payload: &ProviderAccountPayload,
    quota_request: bool,
) -> rquest::header::HeaderMap {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "accept",
        rquest::header::HeaderValue::from_static(if quota_request {
            "*/*"
        } else {
            "application/json"
        }),
    );
    headers.insert(
        "user-agent",
        rquest::header::HeaderValue::from_static("node"),
    );
    headers.insert("x-language", rquest::header::HeaderValue::from_static("zh"));
    headers.insert("x-os", rquest::header::HeaderValue::from_static("win32"));
    headers.insert(
        "x-app-version",
        rquest::header::HeaderValue::from_str(
            payload
                .headers
                .get("x-app-version")
                .or_else(|| payload.headers.get("version"))
                .map(String::as_str)
                .unwrap_or("0.5.6"),
        )
        .unwrap_or_else(|_| rquest::header::HeaderValue::from_static("0.5.6")),
    );
    if let Some(utdid) = payload
        .headers
        .get("x-utdid")
        .or_else(|| payload.headers.get("utdid"))
        .map(String::as_str)
    {
        if let Ok(value) = rquest::header::HeaderValue::from_str(utdid) {
            headers.insert("x-utdid", value);
        }
    }
    if let Some(cna) = payload
        .headers
        .get("x-cna")
        .map(String::as_str)
        .or_else(|| {
            payload
                .headers
                .get("Cookie")
                .or_else(|| payload.headers.get("cookie"))
                .and_then(|value| extract_cookie_value(value, "cna"))
        })
    {
        if let Ok(value) = rquest::header::HeaderValue::from_str(cna) {
            headers.insert("x-cna", value);
        }
    }
    if quota_request {
        headers.insert(
            "accept-language",
            rquest::header::HeaderValue::from_static("*"),
        );
        headers.insert(
            "sec-fetch-mode",
            rquest::header::HeaderValue::from_static("cors"),
        );
    }
    headers
}

fn extract_cookie_value<'a>(cookie_header: &'a str, cookie_name: &str) -> Option<&'a str> {
    cookie_header
        .split(';')
        .filter_map(|segment| segment.split_once('='))
        .find_map(|(name, value)| {
            if name.trim().eq_ignore_ascii_case(cookie_name) {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            } else {
                None
            }
        })
}
