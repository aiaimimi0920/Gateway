use super::direct_http_test_support::make_payload;
use super::*;
use rquest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
#[test]
fn gemini_canvas_direct_http_bootstrap_candidates_prefer_program_page_for_program_adapter() {
    let mut payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://gemini.google.com",
    );
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
    payload.extra_body = Some(HashMap::from([
        (
            "shareId".to_string(),
            Value::String("fe24c455a570".to_string()),
        ),
        (
            "canvasProgramUrl".to_string(),
            Value::String("https://gemini.google.com/app/4abc4e7577b6149f".to_string()),
        ),
    ]));

    let candidates = gemini_canvas_direct_http_bootstrap_candidates(
        &payload,
        "https://gemini.google.com",
        "fe24c455a570",
        false,
    );
    assert_eq!(
        candidates.first().map(String::as_str),
        Some("https://gemini.google.com/app/4abc4e7577b6149f")
    );
    assert!(candidates
        .iter()
        .any(|entry| entry == "https://gemini.google.com/share/fe24c455a570"));
    assert!(candidates
        .iter()
        .any(|entry| entry == "https://gemini.google.com/app"));
}

#[test]
fn gemini_canvas_direct_http_bootstrap_candidates_keep_legacy_order_without_program_page() {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let candidates = gemini_canvas_direct_http_bootstrap_candidates(
        &payload,
        "https://gemini.google.com",
        "fe24c455a570",
        false,
    );
    assert_eq!(
        candidates,
        vec![
            "https://gemini.google.com/share/fe24c455a570".to_string(),
            "https://gemini.google.com/app".to_string()
        ]
    );
}
#[test]
fn build_gemini_canvas_direct_http_bootstrap_response_meta_preserves_shape() {
    let meta = build_gemini_canvas_direct_http_bootstrap_response_meta(
        "https://gemini.google.com/app",
        Some("https://gemini.google.com/share/example"),
        Some("text/html; charset=utf-8"),
        "<html>bootstrap</html>",
    );

    assert_eq!(
        meta,
        "final_url=https://gemini.google.com/app, location=https://gemini.google.com/share/example, content_type=text/html; charset=utf-8, body_preview=<html>bootstrap</html>"
    );
}

#[test]
fn build_gemini_canvas_direct_http_bootstrap_response_meta_preserves_empty_preview() {
    let meta = build_gemini_canvas_direct_http_bootstrap_response_meta(
        "https://gemini.google.com/app",
        None,
        None,
        "",
    );

    assert_eq!(
        meta,
        "final_url=https://gemini.google.com/app, location=<none>, content_type=<none>, body_preview=<empty>"
    );
}

#[test]
fn build_gemini_canvas_direct_http_text_bootstrap_request_contract_preserves_shape() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", HeaderValue::from_static("SID=abc"));
    headers.insert(
        "origin",
        HeaderValue::from_static("https://gemini.google.com"),
    );
    headers.insert(
        "referer",
        HeaderValue::from_static("https://gemini.google.com/share/example"),
    );

    let contract = build_gemini_canvas_direct_http_text_bootstrap_request_contract(
        "https://gemini.google.com/share/example",
        &headers,
    );

    assert_eq!(
        contract,
        "bootstrap_url=https://gemini.google.com/share/example, is_text_mode=true, cookie=<present>, origin=https://gemini.google.com, referer=https://gemini.google.com/share/example"
    );
}

#[test]
fn build_gemini_canvas_direct_http_text_bootstrap_request_contract_marks_missing_headers() {
    let headers = HeaderMap::new();

    let contract = build_gemini_canvas_direct_http_text_bootstrap_request_contract(
        "https://gemini.google.com/app",
        &headers,
    );

    assert_eq!(
        contract,
        "bootstrap_url=https://gemini.google.com/app, is_text_mode=true, cookie=<none>, origin=<none>, referer=<none>"
    );
}

#[test]
fn build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract_preserves_shape() {
    let attempted_urls = vec![
        "https://gemini.google.com/share/example".to_string(),
        "https://gemini.google.com/app".to_string(),
    ];

    let contract = build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract(
        "https://gemini.google.com/share/example",
        &attempted_urls,
        "https://gemini.google.com/share/example",
        128,
    );

    assert_eq!(
        contract,
        "bootstrap_source=page_harvest_helper, selected_url=https://gemini.google.com/share/example, attempted_urls=https://gemini.google.com/share/example,https://gemini.google.com/app, is_text_mode=false, session_target_url=https://gemini.google.com/share/example, cookie_header_len=128"
    );
}

#[test]
fn build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract_preserves_empty_attempts(
) {
    let attempted_urls = Vec::<String>::new();

    let contract = build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract(
        "https://gemini.google.com/app",
        &attempted_urls,
        "https://gemini.google.com/app",
        0,
    );

    assert_eq!(
        contract,
        "bootstrap_source=page_harvest_helper, selected_url=https://gemini.google.com/app, attempted_urls=, is_text_mode=false, session_target_url=https://gemini.google.com/app, cookie_header_len=0"
    );
}

#[test]
fn build_gemini_canvas_direct_http_page_harvest_failure_request_contract_preserves_shape() {
    let attempted_urls = vec![
        "https://gemini.google.com/share/example".to_string(),
        "https://gemini.google.com/app".to_string(),
    ];
    let failures = vec![
        "https://gemini.google.com/share/example: 503 challenge".to_string(),
        "https://gemini.google.com/app: 401 session_invalid".to_string(),
    ];

    let contract = build_gemini_canvas_direct_http_page_harvest_failure_request_contract(
        &attempted_urls,
        "https://gemini.google.com/share/example",
        &failures,
    );

    assert_eq!(
        contract,
        "bootstrap_source=page_harvest_helper, attempted_urls=https://gemini.google.com/share/example,https://gemini.google.com/app, is_text_mode=false, session_target_url=https://gemini.google.com/share/example, failures=https://gemini.google.com/share/example: 503 challenge | https://gemini.google.com/app: 401 session_invalid"
    );
}

#[test]
fn build_gemini_canvas_direct_http_page_harvest_failure_request_contract_preserves_empty_lists() {
    let attempted_urls = Vec::<String>::new();
    let failures = Vec::<String>::new();

    let contract = build_gemini_canvas_direct_http_page_harvest_failure_request_contract(
        &attempted_urls,
        "https://gemini.google.com/app",
        &failures,
    );

    assert_eq!(
        contract,
        "bootstrap_source=page_harvest_helper, attempted_urls=, is_text_mode=false, session_target_url=https://gemini.google.com/app, failures="
    );
}
