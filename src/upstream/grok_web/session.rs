//! Audience and exact-account Cookie boundary, shared by identity GET and WS handshake.
use super::failure;
use crate::{
    error::{ErrorKind, GatewayError},
    routing::candidate::ProviderAccountPayload,
};
use rquest::header::{HeaderMap, HeaderValue};
use serde_json::Value;

pub(super) fn headers(payload: &ProviderAccountPayload) -> Result<HeaderMap, GatewayError> {
    let invalid = || failure("grok_web_material_invalid", ErrorKind::BadRequest);
    if !super::owns_payload(payload) {
        return Err(invalid());
    }
    let url = url::Url::parse(&payload.base_url).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some("grok.com")
        || url.port().is_some_and(|p| p != 443)
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(failure("grok_web_audience_invalid", ErrorKind::BadRequest));
    }
    if payload.session_auth.as_ref().is_some_and(|auth| {
        auth.transport != "cookie"
            || auth
                .primary_cookie_name
                .as_deref()
                .is_some_and(|v| v != "sso")
            || auth
                .secondary_cookie_name
                .as_deref()
                .is_some_and(|v| v != "sso-rw")
    }) || !cookie_value(&payload.api_key)
    {
        return Err(invalid());
    }
    let mut headers = HeaderMap::new();
    let mut cookie = None;
    // Only these account-owned fields are relevant. Caller/other configured headers never enter the wire.
    for (key, value) in &payload.headers {
        if key.eq_ignore_ascii_case("cookie") {
            if cookie.replace(value.as_str()).is_some() {
                return Err(invalid());
            }
        } else if key.eq_ignore_ascii_case("user-agent")
            || key.eq_ignore_ascii_case("accept-language")
        {
            if value.is_empty() || value.len() > 1024 {
                return Err(invalid());
            }
            let name =
                rquest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|_| invalid())?;
            if headers
                .insert(name, HeaderValue::from_str(value).map_err(|_| invalid())?)
                .is_some()
            {
                return Err(invalid());
            }
        }
    }
    let default = format!("sso={}; sso-rw={}", payload.api_key, payload.api_key);
    let cookie = cookie.unwrap_or(&default);
    if cookie.len() > 16 * 1024 {
        return Err(invalid());
    }
    let mut names = std::collections::HashSet::new();
    let mut sso = None;
    for pair in cookie.split(';') {
        let (name, value) = pair.trim().split_once('=').ok_or_else(invalid)?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || !cookie_value(value)
            || !names.insert(name)
        {
            return Err(invalid());
        }
        if name == "sso" {
            sso = Some(value);
        }
    }
    if sso != Some(payload.api_key.as_str()) {
        return Err(invalid());
    }
    let mut cookie = HeaderValue::from_str(cookie).map_err(|_| invalid())?;
    cookie.set_sensitive(true);
    headers.insert("cookie", cookie);
    for (key, value) in [
        ("accept", "*/*"),
        ("origin", "https://grok.com"),
        ("referer", "https://grok.com/"),
        ("cache-control", "no-cache"),
        ("pragma", "no-cache"),
    ] {
        headers.insert(key, HeaderValue::from_static(value));
    }
    if !headers.contains_key("accept-language") {
        headers.insert(
            "accept-language",
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
    }
    Ok(headers)
}

fn cookie_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16 * 1024
        && value
            .bytes()
            .all(|b| matches!(b, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e))
}

pub(super) fn user_id(body: &[u8]) -> Result<String, GatewayError> {
    let invalid = || failure("grok_web_identity_invalid", ErrorKind::Authentication);
    let value: Value = serde_json::from_slice(body).map_err(|_| invalid())?;
    if value["status"].as_str().is_some_and(|s| {
        s.eq_ignore_ascii_case("blocked") || s.eq_ignore_ascii_case("unauthenticated")
    }) {
        return Err(invalid());
    }
    let id = [
        "/session/userId",
        "/user/id",
        "/user/userId",
        "/user/sub",
        "/id",
        "/userId",
        "/sub",
    ]
    .into_iter()
    .filter_map(|p| value.pointer(p).and_then(Value::as_str))
    .find(|v| !v.trim().is_empty())
    .ok_or_else(invalid)?;
    let uuid = uuid::Uuid::parse_str(id.trim()).map_err(|_| invalid())?;
    if uuid.is_nil() {
        return Err(invalid());
    }
    Ok(uuid.to_string())
}
