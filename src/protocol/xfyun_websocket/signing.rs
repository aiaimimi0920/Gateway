use super::payload_fields::{read_required_api_secret, websocket_path};
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use sha2::Sha256;
use time::format_description::well_known::Rfc2822;
use time::OffsetDateTime;

type HmacSha256 = Hmac<Sha256>;

pub fn build_signed_websocket_url(
    payload: &ProviderAccountPayload,
) -> Result<String, GatewayError> {
    let api_secret = read_required_api_secret(payload)?;
    let api_key = payload.api_key.trim();
    if api_key.is_empty() {
        return Err(
            GatewayError::unauthorized("XFYun native WebSocket provider missing APIKey")
                .with_code("xfyun_websocket_missing_api_key")
                .with_provider("xfyun_websocket_compatible"),
        );
    }

    let path = websocket_path(payload);
    let base_url = normalize_websocket_base_url(&payload.base_url)?;
    let (scheme, host, base_path) = split_websocket_base(&base_url)?;
    let full_path = join_paths(&base_path, &path);
    let date = OffsetDateTime::now_utc()
        .format(&Rfc2822)
        .map_err(|error| {
            GatewayError::server_error(format!("failed to format RFC2822 date: {error}"))
                .with_code("xfyun_websocket_date_format_failed")
                .with_provider("xfyun_websocket_compatible")
        })?;
    let signature_origin = format!("host: {host}\ndate: {date}\nGET {full_path} HTTP/1.1");
    let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes()).map_err(|error| {
        GatewayError::server_error(format!("failed to initialize HMAC: {error}"))
            .with_code("xfyun_websocket_hmac_init_failed")
            .with_provider("xfyun_websocket_compatible")
    })?;
    mac.update(signature_origin.as_bytes());
    let signature = BASE64_STANDARD.encode(mac.finalize().into_bytes());
    let authorization_origin = format!(
        "api_key=\"{api_key}\", algorithm=\"hmac-sha256\", headers=\"host date request-line\", signature=\"{signature}\""
    );
    let authorization = BASE64_STANDARD.encode(authorization_origin.as_bytes());

    Ok(format!(
        "{scheme}://{host}{full_path}?authorization={authorization_query}&date={date_query}&host={host_query}",
        authorization_query = encode_query_component(&authorization),
        date_query = encode_query_component(&date),
        host_query = encode_query_component(&host),
    ))
}

fn normalize_websocket_base_url(base_url: &str) -> Result<String, GatewayError> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(
            GatewayError::bad_request("XFYun native WebSocket provider missing base URL")
                .with_code("xfyun_websocket_missing_base_url")
                .with_provider("xfyun_websocket_compatible"),
        );
    }

    let normalized = if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if trimmed.starts_with("wss://") || trimmed.starts_with("ws://") {
        trimmed.to_string()
    } else {
        format!("wss://{trimmed}")
    };

    Ok(normalized)
}

fn split_websocket_base(base_url: &str) -> Result<(String, String, String), GatewayError> {
    let (scheme, rest) = base_url.split_once("://").ok_or_else(|| {
        GatewayError::bad_request("XFYun native WebSocket base URL must include ws:// or wss://")
            .with_code("xfyun_websocket_invalid_base_url")
            .with_provider("xfyun_websocket_compatible")
    })?;
    let mut parts = rest.splitn(2, '/');
    let host = parts.next().unwrap_or_default().trim().to_string();
    if host.is_empty() {
        return Err(
            GatewayError::bad_request("XFYun native WebSocket base URL missing host")
                .with_code("xfyun_websocket_invalid_host")
                .with_provider("xfyun_websocket_compatible"),
        );
    }
    let path = parts
        .next()
        .map(|value| format!("/{}", value.trim_start_matches('/')))
        .unwrap_or_else(|| "/".to_string());
    Ok((scheme.to_string(), host, path))
}

fn join_paths(base_path: &str, path: &str) -> String {
    let normalized_base = if base_path.is_empty() { "/" } else { base_path };
    let normalized_path = if path.is_empty() {
        "/"
    } else if path.starts_with('/') {
        path
    } else {
        return format!("{}/{}", normalized_base.trim_end_matches('/'), path);
    };
    if normalized_base == "/" {
        normalized_path.to_string()
    } else {
        format!(
            "{}/{}",
            normalized_base.trim_end_matches('/'),
            normalized_path.trim_start_matches('/')
        )
    }
}

fn encode_query_component(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}
