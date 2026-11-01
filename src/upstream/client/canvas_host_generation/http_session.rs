//! Selected-account material stays request-local; never consult a profile or browser CDP.
use super::*;
use crate::upstream::gemini::web_reverse::SessionCookies;

pub(super) fn origin(payload: &ProviderAccountPayload) -> Result<String, GatewayError> {
    let url = url::Url::parse(&payload.base_url)
        .map_err(|_| failure("gemini_canvas_http_origin_invalid"))?;
    let production = url.scheme() == "https"
        && url.host_str() == Some("gemini.google.com")
        && url.port_or_known_default() == Some(443);
    #[cfg(test)]
    let fixture = url.scheme() == "http" && url.host_str() == Some("127.0.0.1");
    #[cfg(not(test))]
    let fixture = false;
    if !(production || fixture)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(failure("gemini_canvas_http_origin_invalid"));
    }
    Ok(url.origin().ascii_serialization())
}

pub(super) fn auth_user(payload: &ProviderAccountPayload) -> Result<String, GatewayError> {
    let value = gemini_canvas::direct_http_auth_user(payload);
    if value.is_empty() || value.len() > 3 || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err(invalid(
            "Canvas HTTP account index is invalid.",
            "gemini_canvas_http_auth_user_invalid",
        ));
    }
    Ok(value)
}

pub(super) async fn load(
    payload: &ProviderAccountPayload,
    base: &str,
) -> Result<SessionCookies, GatewayError> {
    // Prefer the imported state over legacy header overrides so a re-import changes identity.
    if let Some(key) = payload
        .runtime_state_object_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
    {
        let storage = gateway_object_storage()
            .map_err(|_| failure("gemini_canvas_http_state_unavailable"))?;
        let bytes = storage
            .read_bytes_with_limit(key, 2 * 1024 * 1024)
            .await
            .map_err(|_| failure("gemini_canvas_http_state_unavailable"))?;
        let state: Value = serde_json::from_slice(&bytes)
            .map_err(|_| failure("gemini_canvas_http_state_invalid"))?;
        return SessionCookies::from_storage_state(base, &state)
            .map_err(|_| failure("gemini_canvas_http_state_invalid"));
    }
    let mut headers = HeaderMap::new();
    if let Some(cookie) = gemini_canvas::browser_cookie_header(payload) {
        let value = HeaderValue::from_str(&cookie)
            .map_err(|_| failure("gemini_canvas_http_state_invalid"))?;
        headers.insert(rquest::header::COOKIE, value);
    }
    SessionCookies::new(base, &headers).map_err(|_| failure("gemini_canvas_http_state_invalid"))
}

pub(super) fn authorize(
    headers: &mut HeaderMap,
    cookies: &SessionCookies,
    target: &str,
    origin: &str,
    auth_user: &str,
) -> Result<(), GatewayError> {
    cookies
        .apply(target, headers)
        .map_err(|_| failure("gemini_canvas_http_state_invalid"))?;
    let raw = headers
        .get(rquest::header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let pairs: Vec<_> = raw
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .collect();
    let sapisid = ["SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID"]
        .iter()
        .find_map(|name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| *value)
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| failure("gemini_canvas_http_signing_cookie_missing"))?;
    let stamp = current_unix_timestamp_i64();
    let digest =
        <sha1::Sha1 as sha1::Digest>::digest(format!("{stamp} {sapisid} {origin}").as_bytes());
    let signature = format!("SAPISIDHASH {stamp}_{}", hex::encode(digest));
    insert_header_map_value(headers, "authorization", &signature);
    insert_header_map_value(headers, "origin", origin);
    insert_header_map_value(headers, "x-origin", origin);
    insert_header_map_value(headers, "x-goog-authuser", auth_user);
    insert_header_map_value(headers, "x-same-domain", "1");
    Ok(())
}
