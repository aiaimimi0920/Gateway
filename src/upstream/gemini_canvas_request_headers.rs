//! Request-header profiles for Canvas replay and browser fetches.

use std::collections::HashMap;

use rquest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::protocol::{gemini_canvas, gemini_web};
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_runtime_helpers::origin_from_url;

mod cookies;

pub(crate) use cookies::{apply_gemini_canvas_cookie_header, apply_gemini_canvas_response_cookies};

pub(crate) fn apply_gemini_canvas_replay_template_headers(
    headers: &mut HeaderMap,
    template_headers: &HashMap<String, String>,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
) {
    for (name, value) in template_headers {
        let normalized = name.trim().to_ascii_lowercase();
        if normalized.is_empty()
            || normalized.starts_with(':')
            || matches!(normalized.as_str(), "content-length" | "host" | "cookie")
        {
            continue;
        }
        insert_header_map_value(headers, &normalized, value);
    }
    apply_gemini_canvas_cookie_header(headers, session);
    headers
        .entry(rquest::header::CONTENT_TYPE)
        .or_insert(HeaderValue::from_static(
            "application/x-www-form-urlencoded;charset=UTF-8",
        ));
}

pub(crate) fn apply_gemini_canvas_signed_headers(
    headers: &mut HeaderMap,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    origin: &str,
    referer: &str,
    authorization: &str,
    include_cookie_header: bool,
) {
    if include_cookie_header {
        insert_header_map_value(headers, "cookie", &session.cookie_header);
    }
    insert_header_map_value(headers, "authorization", authorization);
    insert_header_map_value(headers, "x-origin", origin);
    insert_header_map_value(headers, "x-goog-authuser", &session.auth_user);
    insert_header_map_value(headers, "referer", referer);
}

pub(crate) fn apply_gemini_canvas_browser_validation_headers(headers: &mut HeaderMap) {
    insert_header_map_value(
        headers,
        "x-browser-channel",
        gemini_canvas::GEMINI_CANVAS_BROWSER_CHANNEL,
    );
    insert_header_map_value(
        headers,
        "x-browser-copyright",
        gemini_canvas::GEMINI_CANVAS_BROWSER_COPYRIGHT,
    );
    insert_header_map_value(
        headers,
        "x-browser-validation",
        gemini_canvas::GEMINI_CANVAS_BROWSER_VALIDATION,
    );
    insert_header_map_value(
        headers,
        "x-browser-year",
        gemini_canvas::GEMINI_CANVAS_BROWSER_YEAR,
    );
}

pub(crate) fn apply_gemini_canvas_browserish_text_headers(headers: &mut HeaderMap) {
    for name in ["authorization", "x-api-key", "api-key", "x-goog-api-key"] {
        headers.remove(HeaderName::from_static(name));
    }
    insert_header_map_value(headers, "accept", "*/*");
    insert_header_map_value(headers, "priority", "u=1, i");
    insert_header_map_value(headers, "sec-fetch-dest", "empty");
    insert_header_map_value(headers, "sec-fetch-mode", "cors");
    insert_header_map_value(headers, "sec-fetch-site", "same-origin");
    insert_header_map_value(
        headers,
        "user-agent",
        gemini_canvas::GEMINI_CANVAS_BROWSER_USER_AGENT,
    );
    insert_header_map_value(
        headers,
        "sec-ch-ua",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA,
    );
    insert_header_map_value(
        headers,
        "sec-ch-ua-full-version",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION,
    );
    insert_header_map_value(
        headers,
        "sec-ch-ua-full-version-list",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION_LIST,
    );
    insert_header_map_value(headers, "sec-ch-ua-platform", "\"Windows\"");
    insert_header_map_value(
        headers,
        "sec-ch-ua-platform-version",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA_PLATFORM_VERSION,
    );
    insert_header_map_value(headers, "sec-ch-ua-mobile", "?0");
    insert_header_map_value(headers, "sec-ch-ua-arch", "\"x86\"");
    insert_header_map_value(headers, "sec-ch-ua-bitness", "\"64\"");
    insert_header_map_value(headers, "sec-ch-ua-form-factors", "\"Desktop\"");
    insert_header_map_value(headers, "sec-ch-ua-model", "\"\"");
    insert_header_map_value(headers, "sec-ch-ua-wow64", "?0");
    apply_gemini_canvas_browser_validation_headers(headers);
}

pub(crate) fn apply_gemini_canvas_signaler_headers(headers: &mut HeaderMap) {
    apply_gemini_canvas_browserish_text_headers(headers);
    for name in [
        "x-browser-channel",
        "x-browser-copyright",
        "x-browser-validation",
        "x-browser-year",
    ] {
        headers.remove(HeaderName::from_static(name));
    }
    insert_header_map_value(headers, "sec-fetch-site", "same-site");
}

pub(crate) fn apply_gemini_canvas_navigation_headers(headers: &mut HeaderMap) {
    insert_header_map_value(
        headers,
        "user-agent",
        gemini_canvas::GEMINI_CANVAS_BROWSER_USER_AGENT,
    );
    insert_header_map_value(
        headers,
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
    );
    insert_header_map_value(headers, "upgrade-insecure-requests", "1");
    insert_header_map_value(headers, "sec-fetch-site", "none");
    insert_header_map_value(headers, "sec-fetch-mode", "navigate");
    insert_header_map_value(headers, "sec-fetch-user", "?1");
    insert_header_map_value(headers, "sec-fetch-dest", "document");
    insert_header_map_value(
        headers,
        "sec-ch-ua",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA,
    );
    insert_header_map_value(
        headers,
        "sec-ch-ua-full-version",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION,
    );
    insert_header_map_value(
        headers,
        "sec-ch-ua-full-version-list",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION_LIST,
    );
    insert_header_map_value(headers, "sec-ch-ua-platform", "\"Windows\"");
    insert_header_map_value(
        headers,
        "sec-ch-ua-platform-version",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA_PLATFORM_VERSION,
    );
    insert_header_map_value(headers, "sec-ch-ua-mobile", "?0");
    insert_header_map_value(headers, "sec-ch-ua-arch", "\"x86\"");
    insert_header_map_value(headers, "sec-ch-ua-bitness", "\"64\"");
    insert_header_map_value(headers, "sec-ch-ua-form-factors", "\"Desktop\"");
    insert_header_map_value(headers, "sec-ch-ua-model", "\"\"");
    insert_header_map_value(headers, "sec-ch-ua-wow64", "?0");
    apply_gemini_canvas_browser_validation_headers(headers);
}

pub(crate) fn apply_gemini_canvas_lightweight_navigation_headers(headers: &mut HeaderMap) {
    insert_header_map_value(
        headers,
        "user-agent",
        gemini_canvas::GEMINI_CANVAS_BROWSER_USER_AGENT,
    );
    insert_header_map_value(
        headers,
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    );
    insert_header_map_value(headers, "cache-control", "no-cache");
    insert_header_map_value(headers, "pragma", "no-cache");
}

pub(crate) fn apply_gemini_canvas_text_session_headers(
    headers: &mut HeaderMap,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
) {
    apply_gemini_canvas_cookie_header(headers, session);
    if !session.auth_user.trim().is_empty() {
        insert_header_map_value(headers, "x-goog-authuser", &session.auth_user);
    }
}

pub(crate) fn apply_gemini_canvas_capture_aligned_batchexecute_headers(
    headers: &mut HeaderMap,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    accept_language: &str,
    model_header: &str,
    model_header_2: &str,
) {
    for name in [
        "authorization",
        "x-api-key",
        "api-key",
        "x-goog-api-key",
        "accept",
        "priority",
        "origin",
        "sec-fetch-dest",
        "sec-fetch-mode",
        "sec-fetch-site",
        "x-goog-authuser",
        "x-browser-channel",
        "x-browser-copyright",
        "x-browser-validation",
        "x-browser-year",
    ] {
        headers.remove(HeaderName::from_static(name));
    }
    insert_header_map_value(
        headers,
        "user-agent",
        gemini_canvas::GEMINI_CANVAS_BROWSER_USER_AGENT,
    );
    insert_header_map_value(
        headers,
        "sec-ch-ua",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA,
    );
    insert_header_map_value(
        headers,
        "sec-ch-ua-full-version",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION,
    );
    insert_header_map_value(
        headers,
        "sec-ch-ua-full-version-list",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION_LIST,
    );
    insert_header_map_value(headers, "sec-ch-ua-platform", "\"Windows\"");
    insert_header_map_value(
        headers,
        "sec-ch-ua-platform-version",
        gemini_canvas::GEMINI_CANVAS_BROWSER_SEC_CH_UA_PLATFORM_VERSION,
    );
    insert_header_map_value(headers, "sec-ch-ua-mobile", "?0");
    insert_header_map_value(headers, "sec-ch-ua-arch", "\"x86\"");
    insert_header_map_value(headers, "sec-ch-ua-bitness", "\"64\"");
    insert_header_map_value(headers, "sec-ch-ua-form-factors", "\"Desktop\"");
    insert_header_map_value(headers, "sec-ch-ua-model", "\"\"");
    insert_header_map_value(headers, "sec-ch-ua-wow64", "?0");
    apply_gemini_canvas_cookie_header(headers, session);
    insert_header_map_value(headers, "accept-language", accept_language);
    headers.remove(HeaderName::from_static(
        gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY,
    ));
    headers.remove(HeaderName::from_static(
        gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY,
    ));
    insert_header_map_value(
        headers,
        gemini_web::GEMINI_WEB_MODEL_HEADER_KEY,
        model_header,
    );
    insert_header_map_value(
        headers,
        gemini_web::GEMINI_WEB_MODEL_HEADER_2_KEY,
        model_header_2,
    );
}

pub(crate) fn apply_gemini_canvas_same_origin_batchexecute_headers(
    headers: &mut HeaderMap,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    accept_language: &str,
    model_header: &str,
    model_header_2: &str,
) {
    apply_gemini_canvas_browserish_text_headers(headers);
    apply_gemini_canvas_text_session_headers(headers, session);
    insert_header_map_value(headers, "accept-language", accept_language);
    headers.remove("x-goog-authuser");
    headers.remove(HeaderName::from_static(
        gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY,
    ));
    headers.remove(HeaderName::from_static(
        gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY,
    ));
    insert_header_map_value(
        headers,
        gemini_web::GEMINI_WEB_MODEL_HEADER_KEY,
        model_header,
    );
    insert_header_map_value(
        headers,
        gemini_web::GEMINI_WEB_MODEL_HEADER_2_KEY,
        model_header_2,
    );
}

pub(crate) fn apply_gemini_canvas_page_context_headers(
    headers: &mut HeaderMap,
    page_origin: &str,
    page_referer: &str,
    target_origin: &str,
    preserve_cross_origin_origin: bool,
) -> bool {
    if target_origin.eq_ignore_ascii_case(page_origin.trim()) || preserve_cross_origin_origin {
        insert_header_map_value(headers, "origin", page_origin);
        insert_header_map_value(headers, "referer", page_referer);
        true
    } else {
        headers.remove("origin");
        headers.remove("referer");
        false
    }
}

pub(crate) fn apply_browser_fetch_client_hints(
    headers: &mut HeaderMap,
    request_url: &str,
    origin: &str,
) {
    if !headers.contains_key("sec-ch-ua") {
        insert_header_map_value(
            headers,
            "sec-ch-ua",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA,
        );
    }
    if !headers.contains_key("sec-ch-ua-mobile") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-mobile",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_MOBILE,
        );
    }
    if !headers.contains_key("sec-ch-ua-arch") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-arch",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_ARCH,
        );
    }
    if !headers.contains_key("sec-ch-ua-bitness") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-bitness",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_BITNESS,
        );
    }
    if !headers.contains_key("sec-ch-ua-form-factors") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-form-factors",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_FORM_FACTORS,
        );
    }
    if !headers.contains_key("sec-ch-ua-full-version") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-full-version",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION,
        );
    }
    if !headers.contains_key("sec-ch-ua-full-version-list") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-full-version-list",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION_LIST,
        );
    }
    if !headers.contains_key("sec-ch-ua-model") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-model",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_MODEL,
        );
    }
    if !headers.contains_key("sec-ch-ua-platform") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-platform",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_PLATFORM,
        );
    }
    if !headers.contains_key("sec-ch-ua-platform-version") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-platform-version",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_PLATFORM_VERSION,
        );
    }
    if !headers.contains_key("sec-ch-ua-wow64") {
        insert_header_map_value(
            headers,
            "sec-ch-ua-wow64",
            gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_WOW64,
        );
    }
    if !headers.contains_key("sec-fetch-mode") {
        insert_header_map_value(headers, "sec-fetch-mode", "cors");
    }
    if !headers.contains_key("sec-fetch-dest") {
        insert_header_map_value(headers, "sec-fetch-dest", "empty");
    }
    if !headers.contains_key("sec-fetch-site") {
        let request_origin = origin_from_url(request_url);
        let site = if request_origin
            .as_deref()
            .is_some_and(|request_origin| request_origin.eq_ignore_ascii_case(origin.trim()))
        {
            "same-origin"
        } else {
            "cross-site"
        };
        insert_header_map_value(headers, "sec-fetch-site", site);
    }
}

#[cfg(test)]
mod tests;
