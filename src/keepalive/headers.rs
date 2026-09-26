//! External header projection and session-auth probe request construction.

use crate::{
    credential_runtime::SessionAuthConfig, http::request_headers::is_internal_gateway_header,
};
use rquest::{
    header::{HeaderName, HeaderValue},
    Client,
};
use std::collections::HashMap;

pub(super) fn build_keepalive_probe_headers(
    headers: Option<&HashMap<String, String>>,
    session_auth: Option<&SessionAuthConfig>,
    api_key: &str,
) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (key, value) in headers.into_iter().flat_map(|items| items.iter()) {
        let normalized = key.trim().to_ascii_lowercase();
        if key.trim().is_empty()
            || value.trim().is_empty()
            || normalized == "content-type"
            || normalized == "authorization"
            || normalized == "cookie"
            || is_internal_gateway_header(&normalized)
        {
            continue;
        }
        result.insert(key.trim().to_string(), value.trim().to_string());
    }

    match session_auth
        .map(|config| config.transport.as_str())
        .unwrap_or("cookie")
    {
        "bearer" => {
            let header_name = session_auth
                .and_then(SessionAuthConfig::header_name)
                .unwrap_or("authorization")
                .to_string();
            if !is_internal_gateway_header(&header_name) {
                result.insert(header_name, format!("Bearer {api_key}"));
            }
        }
        "header" => {
            let header_name = session_auth
                .and_then(SessionAuthConfig::header_name)
                .unwrap_or("x-session-token")
                .to_string();
            if !is_internal_gateway_header(&header_name) {
                result.insert(header_name, api_key.to_string());
            }
        }
        _ => {
            let primary = session_auth
                .and_then(|config| config.primary_cookie_name.clone())
                .unwrap_or_else(|| "token".to_string());
            let secondary = session_auth.and_then(|config| config.secondary_cookie_name.clone());
            let mut cookies = vec![format!("{primary}={api_key}")];
            if let Some(secondary) = secondary.filter(|value| !value.trim().is_empty()) {
                cookies.push(format!("{}={}", secondary.trim(), api_key));
            }
            result.insert("cookie".to_string(), cookies.join("; "));
        }
    }

    result
}

pub(super) fn collect_set_cookie_header(headers: &rquest::header::HeaderMap) -> Option<String> {
    let mut cookies = Vec::new();
    for value in headers.get_all(rquest::header::SET_COOKIE) {
        let Ok(value) = value.to_str() else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if let Some(cookie) = value
            .split(';')
            .next()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            if !cookies.iter().any(|existing: &String| existing == cookie) {
                cookies.push(cookie.to_string());
            }
        }
    }
    if cookies.is_empty() {
        None
    } else {
        Some(cookies.join("; "))
    }
}

pub(super) fn read_header_case_insensitive(
    headers: &HashMap<String, String>,
    name: &str,
) -> Option<String> {
    headers
        .iter()
        .find(|(key, value)| key.trim().eq_ignore_ascii_case(name) && !value.trim().is_empty())
        .map(|(_, value)| value.trim().to_string())
}

pub(super) fn upsert_header_case_insensitive(
    headers: &mut HashMap<String, String>,
    name: &str,
    value: impl Into<String>,
) {
    let existing = headers
        .keys()
        .filter(|key| key.trim().eq_ignore_ascii_case(name))
        .cloned()
        .collect::<Vec<_>>();
    for key in existing {
        headers.remove(&key);
    }
    headers.insert(name.to_string(), value.into());
}

pub(super) fn request_builder_with_headers(
    client: &Client,
    method: rquest::Method,
    url: &str,
    headers: &HashMap<String, String>,
) -> rquest::RequestBuilder {
    let mut builder = client.request(method, url);
    for (key, value) in headers {
        if is_internal_gateway_header(key) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::try_from(key.as_str()),
            HeaderValue::from_str(value.as_str()),
        ) {
            builder = builder.header(name, value);
        }
    }
    builder
}

pub(super) fn external_gateway_headers(
    headers: &HashMap<String, String>,
) -> HashMap<String, String> {
    headers
        .iter()
        .filter(|(name, _)| !is_internal_gateway_header(name))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}
