//! Resolve explicit session material and Canvas origins with the existing precedence.

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini::canvas_program_web_reverse as gemini_canvas_program_web_reverse_modular;

fn read_json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

pub(crate) fn gemini_canvas_explicit_cookie_header_from_payload(
    payload: &ProviderAccountPayload,
    storage_state: Option<&Value>,
) -> Option<String> {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| {
            extra
                .get("canvasProgramInvokeContract")
                .and_then(Value::as_object)
                .and_then(|contract| contract.get("cookieHeader"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("cookieHeader"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| {
                    extra
                        .get("canvasProgramInvokeContract")
                        .and_then(Value::as_object)
                        .and_then(|contract| contract.get("cookie_header"))
                        .and_then(Value::as_str)
                })
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("cookie_header"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("cookie"))
                .map(|(_, value)| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| storage_state.and_then(|value| read_json_string(value, "cookieHeader")))
}

pub(crate) fn gemini_canvas_pure_http_session_from_payload_or_storage(
    payload: &ProviderAccountPayload,
    storage_state: &Value,
    target_url: &str,
    base_url: &str,
    auth_user: &str,
) -> Result<gemini_canvas::GeminiCanvasPureHttpSession, GatewayError> {
    if let Some(cookie_header) =
        gemini_canvas_explicit_cookie_header_from_payload(payload, Some(storage_state))
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        gemini_canvas::pure_http_session_from_cookie_header(cookie_header, auth_user)
    } else {
        gemini_canvas::storage_state_to_pure_http_session(
            storage_state,
            target_url,
            base_url,
            auth_user,
        )
    }
}

pub(crate) fn gemini_canvas_http_origin(payload: &ProviderAccountPayload) -> String {
    origin_from_url(payload.base_url.trim_end_matches('/'))
        .unwrap_or_else(|| "https://gemini.google.com".to_string())
}

pub(crate) fn gemini_canvas_page_base_url(payload: &ProviderAccountPayload) -> String {
    if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
        if let Some(page_url) =
            gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_page_url(
                payload,
                "https://gemini.google.com",
            )
        {
            if let Some(origin) = origin_from_url(&page_url) {
                return origin;
            }
        }
    }
    let base_origin = gemini_canvas_http_origin(payload);
    if base_origin.contains("gemini.google.com") {
        base_origin
    } else {
        "https://gemini.google.com".to_string()
    }
}

pub(crate) fn origin_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (scheme, rest) = if let Some(rest) = trimmed.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        ("http", rest)
    } else {
        ("https", trimmed)
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .split('@')
        .last()
        .unwrap_or_default()
        .trim();
    if host.is_empty() {
        None
    } else {
        Some(format!("{scheme}://{host}"))
    }
}
