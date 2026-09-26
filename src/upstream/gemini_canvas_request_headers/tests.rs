use super::cookies::merge_gemini_canvas_cookie_header;
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
        header_map_string(&headers, gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY).is_none()
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
        header_map_string(&headers, gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY).is_none()
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
