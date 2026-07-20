use std::collections::HashMap;

use rquest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::protocol::{gemini_canvas, gemini_web};
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_runtime_helpers::origin_from_url;

pub(crate) fn apply_gemini_canvas_cookie_header(
    headers: &mut HeaderMap,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
) {
    insert_header_map_value(headers, "cookie", &session.cookie_header);
}

pub(crate) fn merge_gemini_canvas_cookie_header(
    existing: &str,
    set_cookie_values: &[String],
) -> String {
    let mut ordered_pairs: Vec<(String, String)> = existing
        .split(';')
        .filter_map(|segment| {
            let (name, value) = segment.trim().split_once('=')?;
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some((name.to_string(), value.to_string()))
        })
        .collect();
    let mut positions: HashMap<String, usize> = ordered_pairs
        .iter()
        .enumerate()
        .map(|(index, (name, _))| (name.clone(), index))
        .collect();

    for set_cookie in set_cookie_values {
        for logical_cookie in set_cookie
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let Some((name, value)) = logical_cookie
                .split(';')
                .next()
                .and_then(|segment| segment.trim().split_once('='))
            else {
                continue;
            };
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                continue;
            }
            if let Some(index) = positions.get(name).copied() {
                ordered_pairs[index].1 = value.to_string();
            } else {
                positions.insert(name.to_string(), ordered_pairs.len());
                ordered_pairs.push((name.to_string(), value.to_string()));
            }
        }
    }

    ordered_pairs
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn apply_gemini_canvas_response_cookies(
    headers: &HeaderMap,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
) {
    let set_cookie_values = headers
        .get_all(rquest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok().map(str::to_string))
        .collect::<Vec<_>>();
    if set_cookie_values.is_empty() {
        return;
    }

    let merged_cookie_header =
        merge_gemini_canvas_cookie_header(&session.cookie_header, &set_cookie_values);
    if !merged_cookie_header.is_empty() {
        session.cookie_header = merged_cookie_header;
    }

    for set_cookie in &set_cookie_values {
        for logical_cookie in set_cookie
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let Some((name, value)) = logical_cookie
                .split(';')
                .next()
                .and_then(|segment| segment.trim().split_once('='))
            else {
                continue;
            };
            match name.trim() {
                "__Secure-1PAPISID" | "__Secure-3PAPISID" | "SAPISID"
                    if !value.trim().is_empty() =>
                {
                    session.sapisid = value.trim().to_string();
                    return;
                }
                _ => {}
            }
        }
    }
}

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
mod tests {
    use super::*;

    use rquest::header::HeaderValue;

    use crate::upstream::common::insert_header_map_value;
    use crate::upstream::header_map_helpers::header_map_string;

    fn make_session() -> gemini_canvas::GeminiCanvasPureHttpSession {
        gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "__Secure-1PSID=psid; __Secure-1PAPISID=sapisid".to_string(),
            sapisid: "sapisid".to_string(),
            auth_user: "0".to_string(),
        }
    }

    #[test]
    fn apply_gemini_canvas_navigation_headers_sets_document_navigation_contract() {
        let mut headers = HeaderMap::new();
        apply_gemini_canvas_navigation_headers(&mut headers);

        assert_eq!(
            header_map_string(&headers, "sec-fetch-site").as_deref(),
            Some("none")
        );
        assert_eq!(
            header_map_string(&headers, "sec-fetch-mode").as_deref(),
            Some("navigate")
        );
        assert_eq!(
            header_map_string(&headers, "sec-fetch-user").as_deref(),
            Some("?1")
        );
        assert_eq!(
            header_map_string(&headers, "sec-fetch-dest").as_deref(),
            Some("document")
        );
        assert_eq!(
            header_map_string(&headers, "x-browser-validation").as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_BROWSER_VALIDATION)
        );
    }

    #[test]
    fn apply_gemini_canvas_signaler_headers_drops_browser_validation_fields_and_marks_same_site() {
        let mut headers = HeaderMap::new();
        apply_gemini_canvas_signaler_headers(&mut headers);

        assert_eq!(
            header_map_string(&headers, "sec-fetch-site").as_deref(),
            Some("same-site")
        );
        assert!(header_map_string(&headers, "x-browser-channel").is_none());
        assert!(header_map_string(&headers, "x-browser-copyright").is_none());
        assert!(header_map_string(&headers, "x-browser-validation").is_none());
        assert!(header_map_string(&headers, "x-browser-year").is_none());
    }

    #[test]
    fn merge_gemini_canvas_cookie_header_replaces_existing_values_and_preserves_order() {
        let merged = merge_gemini_canvas_cookie_header(
            "a=1; b=2",
            &[
                "b=22; Path=/; Secure".to_string(),
                "__Secure-1PAPISID=new-sapisid; Path=/".to_string(),
            ],
        );

        assert_eq!(merged, "a=1; b=22; __Secure-1PAPISID=new-sapisid");
    }

    #[test]
    fn apply_gemini_canvas_response_cookies_updates_cookie_header_and_sapisid() {
        let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "__Secure-1PSID=psid; __Secure-1PAPISID=old".to_string(),
            sapisid: "old".to_string(),
            auth_user: "0".to_string(),
        };
        let mut headers = HeaderMap::new();
        headers.append(
            rquest::header::SET_COOKIE,
            HeaderValue::from_static("__Secure-1PAPISID=new-sapisid; Path=/; Secure"),
        );
        headers.append(
            rquest::header::SET_COOKIE,
            HeaderValue::from_static("NID=example; Path=/; Secure"),
        );

        apply_gemini_canvas_response_cookies(&headers, &mut session);

        assert_eq!(session.sapisid, "new-sapisid");
        assert_eq!(
            session.cookie_header,
            "__Secure-1PSID=psid; __Secure-1PAPISID=new-sapisid; NID=example"
        );
    }

    #[test]
    fn apply_gemini_canvas_replay_template_headers_skips_cookie_and_sets_default_content_type() {
        let mut headers = HeaderMap::new();
        let template = HashMap::from([
            ("Authorization".to_string(), "Bearer replay".to_string()),
            ("Cookie".to_string(), "ignored=1".to_string()),
            ("Host".to_string(), "ignored.example".to_string()),
        ]);
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "__Secure-1PSID=psid".to_string(),
            sapisid: "psid".to_string(),
            auth_user: "0".to_string(),
        };

        apply_gemini_canvas_replay_template_headers(&mut headers, &template, &session);

        assert_eq!(
            header_map_string(&headers, "authorization").as_deref(),
            Some("Bearer replay")
        );
        assert_eq!(
            header_map_string(&headers, "cookie").as_deref(),
            Some("__Secure-1PSID=psid")
        );
        assert_eq!(
            header_map_string(&headers, "content-type").as_deref(),
            Some("application/x-www-form-urlencoded;charset=UTF-8")
        );
        assert!(header_map_string(&headers, "host").is_none());
    }

    #[test]
    fn apply_gemini_canvas_signed_headers_sets_google_session_contract() {
        let session = make_session();
        let mut headers = HeaderMap::new();

        apply_gemini_canvas_signed_headers(
            &mut headers,
            &session,
            "https://gemini.google.com",
            "https://gemini.google.com/share/abc",
            "SAPISIDHASH 1_hash SAPISID1PHASH 1_hash SAPISID3PHASH 1_hash",
            true,
        );

        assert_eq!(
            header_map_string(&headers, "cookie").as_deref(),
            Some("__Secure-1PSID=psid; __Secure-1PAPISID=sapisid")
        );
        assert_eq!(
            header_map_string(&headers, "authorization").as_deref(),
            Some("SAPISIDHASH 1_hash SAPISID1PHASH 1_hash SAPISID3PHASH 1_hash")
        );
        assert_eq!(
            header_map_string(&headers, "x-origin").as_deref(),
            Some("https://gemini.google.com")
        );
        assert_eq!(
            header_map_string(&headers, "x-goog-authuser").as_deref(),
            Some("0")
        );
        assert_eq!(
            header_map_string(&headers, "referer").as_deref(),
            Some("https://gemini.google.com/share/abc")
        );

        let mut auth_only_headers = HeaderMap::new();
        apply_gemini_canvas_signed_headers(
            &mut auth_only_headers,
            &session,
            "https://gemini.google.com",
            "https://gemini.google.com/share/abc",
            "SAPISIDHASH 1_hash SAPISID1PHASH 1_hash SAPISID3PHASH 1_hash",
            false,
        );
        assert!(header_map_string(&auth_only_headers, "cookie").is_none());
    }

    #[test]
    fn apply_gemini_canvas_page_context_headers_omits_cross_origin_context() {
        let mut same_origin_headers = HeaderMap::new();
        let same_origin = apply_gemini_canvas_page_context_headers(
            &mut same_origin_headers,
            "https://gemini.google.com",
            "https://gemini.google.com/share/abc",
            "https://gemini.google.com",
            false,
        );
        assert!(same_origin);
        assert_eq!(
            header_map_string(&same_origin_headers, "origin").as_deref(),
            Some("https://gemini.google.com")
        );
        assert_eq!(
            header_map_string(&same_origin_headers, "referer").as_deref(),
            Some("https://gemini.google.com/share/abc")
        );

        let mut cross_origin_headers = HeaderMap::new();
        let same_origin = apply_gemini_canvas_page_context_headers(
            &mut cross_origin_headers,
            "https://gemini.google.com",
            "https://gemini.google.com/share/abc",
            "https://geminiweb-pa.clients6.google.com",
            false,
        );
        assert!(!same_origin);
        assert!(header_map_string(&cross_origin_headers, "origin").is_none());
        assert!(header_map_string(&cross_origin_headers, "referer").is_none());

        let mut preserved_cross_origin_headers = HeaderMap::new();
        let preserved = apply_gemini_canvas_page_context_headers(
            &mut preserved_cross_origin_headers,
            "https://gemini.google.com",
            "https://gemini.google.com/share/abc",
            "https://geminiweb-pa.clients6.google.com",
            true,
        );
        assert!(preserved);
        assert_eq!(
            header_map_string(&preserved_cross_origin_headers, "origin").as_deref(),
            Some("https://gemini.google.com")
        );
        assert_eq!(
            header_map_string(&preserved_cross_origin_headers, "referer").as_deref(),
            Some("https://gemini.google.com/share/abc")
        );
    }

    #[test]
    fn apply_gemini_canvas_same_origin_batchexecute_headers_preserve_edit_broad_surface() {
        let mut headers = HeaderMap::new();
        insert_header_map_value(&mut headers, "origin", "https://gemini.google.com");
        insert_header_map_value(
            &mut headers,
            gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY,
            "[\"stale\",1]",
        );
        insert_header_map_value(
            &mut headers,
            gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY,
            "[\"stale\",1]",
        );
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "__Secure-1PSID=test".to_string(),
            sapisid: "test".to_string(),
            auth_user: "0".to_string(),
        };

        apply_gemini_canvas_same_origin_batchexecute_headers(
            &mut headers,
            &session,
            "zh-CN",
            "[1,null,null,null,null,null,null,null,[4],null,null,null,null,null,null,null,\"capture-uuid\"]",
            "[]",
        );

        assert_eq!(
            header_map_string(&headers, "origin").as_deref(),
            Some("https://gemini.google.com")
        );
        assert_eq!(
            header_map_string(&headers, "x-browser-channel").as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_BROWSER_CHANNEL)
        );
        assert_eq!(
            header_map_string(&headers, "x-browser-copyright").as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_BROWSER_COPYRIGHT)
        );
        assert_eq!(
            header_map_string(&headers, "x-browser-validation").as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_BROWSER_VALIDATION)
        );
        assert_eq!(
            header_map_string(&headers, "x-browser-year").as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_BROWSER_YEAR)
        );
        assert!(header_map_string(&headers, "x-goog-authuser").is_none());
        assert!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY)
                .is_none()
        );
        assert!(header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY).is_none());
    }

    #[test]
    fn apply_gemini_canvas_capture_aligned_batchexecute_headers_match_successful_maziqc_shape() {
        let mut headers = HeaderMap::new();
        insert_header_map_value(&mut headers, "origin", "https://gemini.google.com");
        insert_header_map_value(&mut headers, "accept", "*/*");
        insert_header_map_value(&mut headers, "priority", "u=1, i");
        insert_header_map_value(&mut headers, "sec-fetch-mode", "cors");
        insert_header_map_value(
            &mut headers,
            gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY,
            "[\"stale\",1]",
        );
        insert_header_map_value(
            &mut headers,
            gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY,
            "[\"stale\",1]",
        );
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "__Secure-1PSID=test".to_string(),
            sapisid: "test".to_string(),
            auth_user: "0".to_string(),
        };

        apply_gemini_canvas_capture_aligned_batchexecute_headers(
            &mut headers,
            &session,
            "zh-CN",
            "[1,null,null,null,null,null,null,null,[4],null,null,null,null,null,null,null,\"capture-uuid\"]",
            "[0]",
        );

        assert!(header_map_string(&headers, "origin").is_none());
        assert!(header_map_string(&headers, "accept").is_none());
        assert!(header_map_string(&headers, "priority").is_none());
        assert!(header_map_string(&headers, "sec-fetch-mode").is_none());
        assert!(header_map_string(&headers, "x-goog-authuser").is_none());
        assert!(header_map_string(&headers, "x-browser-channel").is_none());
        assert!(header_map_string(&headers, "x-browser-copyright").is_none());
        assert!(header_map_string(&headers, "x-browser-validation").is_none());
        assert!(header_map_string(&headers, "x-browser-year").is_none());
        assert_eq!(
            header_map_string(&headers, "user-agent").as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_BROWSER_USER_AGENT)
        );
        assert_eq!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_KEY).as_deref(),
            Some(
                "[1,null,null,null,null,null,null,null,[4],null,null,null,null,null,null,null,\"capture-uuid\"]"
            )
        );
        assert_eq!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_2_KEY).as_deref(),
            Some("[0]")
        );
        assert!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY)
                .is_none()
        );
        assert!(header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY).is_none());
    }

    #[test]
    fn apply_browser_fetch_client_hints_sets_expected_defaults() {
        let mut same_origin_headers = HeaderMap::new();
        apply_browser_fetch_client_hints(
            &mut same_origin_headers,
            "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate",
            "https://gemini.google.com",
        );

        assert_eq!(
            header_map_string(&same_origin_headers, "sec-ch-ua").as_deref(),
            Some(gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA)
        );
        assert_eq!(
            header_map_string(&same_origin_headers, "sec-ch-ua-mobile").as_deref(),
            Some(gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_MOBILE)
        );
        assert_eq!(
            header_map_string(&same_origin_headers, "sec-ch-ua-arch").as_deref(),
            Some(gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_ARCH)
        );
        assert_eq!(
            header_map_string(&same_origin_headers, "sec-ch-ua-full-version-list").as_deref(),
            Some(gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION_LIST)
        );
        assert_eq!(
            header_map_string(&same_origin_headers, "sec-ch-ua-platform").as_deref(),
            Some(gemini_web::GEMINI_WEB_DEFAULT_SEC_CH_UA_PLATFORM)
        );
        assert_eq!(
            header_map_string(&same_origin_headers, "sec-fetch-mode").as_deref(),
            Some("cors")
        );
        assert_eq!(
            header_map_string(&same_origin_headers, "sec-fetch-dest").as_deref(),
            Some("empty")
        );
        assert_eq!(
            header_map_string(&same_origin_headers, "sec-fetch-site").as_deref(),
            Some("same-origin")
        );

        let mut cross_site_headers = HeaderMap::new();
        apply_browser_fetch_client_hints(
            &mut cross_site_headers,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent",
            "https://gemini.google.com",
        );
        assert_eq!(
            header_map_string(&cross_site_headers, "sec-fetch-site").as_deref(),
            Some("cross-site")
        );
    }
}
