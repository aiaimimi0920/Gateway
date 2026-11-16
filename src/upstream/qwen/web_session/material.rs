//! Fixed chat audience and account-owned Bearer/Cookie; no imported seed or caller auth is forwarded.
use super::failure;
use crate::{
    error::{ErrorKind, GatewayError},
    routing::candidate::ProviderAccountPayload,
};
use rquest::header::{HeaderMap, HeaderValue};

pub(super) fn headers(payload: &ProviderAccountPayload) -> Result<HeaderMap, GatewayError> {
    let invalid = || failure("qwen_web_current_material_invalid", ErrorKind::BadRequest);
    let url = url::Url::parse(&payload.base_url).map_err(|_| invalid())?;
    if !super::owns_payload(payload)
        || url.scheme() != "https"
        || url.host_str() != Some("chat.qwen.ai")
        || url.port().is_some_and(|p| p != 443)
        || !matches!(url.path().trim_end_matches('/'), "" | "/api/v1")
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(failure(
            "qwen_web_current_audience_invalid",
            ErrorKind::BadRequest,
        ));
    }
    if payload.api_key.is_empty()
        || payload.api_key.len() > 16 * 1024
        || !payload
            .api_key
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b';')
        || payload.keepalive.is_some()
        || payload.extra_body.as_ref().is_some_and(|b| !b.is_empty())
        || payload
            .chat_completions_path
            .as_deref()
            .is_some_and(|p| p != "/api/v2/chat/completions")
        || payload.session_auth.as_ref().is_some_and(|s| {
            s.transport != "bearer"
                || s.header_name
                    .as_deref()
                    .is_some_and(|h| !h.eq_ignore_ascii_case("authorization"))
        })
    {
        return Err(invalid());
    }
    for expiry in [
        payload.expires_at.as_deref(),
        payload
            .session_auth
            .as_ref()
            .and_then(|s| s.expires_at.as_deref()),
    ]
    .into_iter()
    .flatten()
    {
        let expiry =
            time::OffsetDateTime::parse(expiry, &time::format_description::well_known::Rfc3339)
                .map_err(|_| invalid())?;
        if expiry <= time::OffsetDateTime::now_utc() {
            return Err(failure(
                "qwen_web_current_session_expired",
                ErrorKind::Authentication,
            ));
        }
    }
    let mut headers = HeaderMap::new();
    for (key, value) in &payload.headers {
        let name = key.to_ascii_lowercase();
        if !matches!(name.as_str(), "cookie" | "user-agent" | "accept-language") {
            continue;
        }
        if value.is_empty() || value.len() > if name == "cookie" { 16 * 1024 } else { 2048 } {
            return Err(invalid());
        }
        let mut header = HeaderValue::from_str(value).map_err(|_| invalid())?;
        if name == "cookie" {
            let mut names = std::collections::HashSet::new();
            for pair in value.split(';') {
                let (key, val) = pair.trim().split_once('=').ok_or_else(invalid)?;
                if key.is_empty()
                    || !key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                    || !val.bytes().all(|b| b.is_ascii_graphic() && b != b';')
                    || !names.insert(key)
                {
                    return Err(invalid());
                }
            }
            header.set_sensitive(true);
        }
        if headers
            .insert(
                rquest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| invalid())?,
                header,
            )
            .is_some()
        {
            return Err(invalid());
        }
    }
    let mut auth =
        HeaderValue::from_str(&format!("Bearer {}", payload.api_key)).map_err(|_| invalid())?;
    auth.set_sensitive(true);
    headers.insert("authorization", auth);
    for (key, value) in [
        ("origin", "https://chat.qwen.ai"),
        ("referer", "https://chat.qwen.ai/"),
        ("accept", "application/json, text/event-stream"),
        ("content-type", "application/json"),
        ("source", "web"),
        ("version", "0.3.36"),
    ] {
        headers.insert(key, HeaderValue::from_static(value));
    }
    headers.insert(
        "x-request-id",
        HeaderValue::from_str(&uuid::Uuid::new_v4().to_string()).unwrap(),
    );
    headers.insert(
        "timezone",
        HeaderValue::from_str(
            &time::OffsetDateTime::now_utc()
                .format(&time::format_description::parse("[weekday repr:short] [month repr:short] [day] [year] [hour]:[minute]:[second] GMT+0000").unwrap())
                .unwrap(),
        )
        .unwrap(),
    );
    Ok(headers)
}
