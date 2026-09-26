use super::direct_http_test_support::make_payload;
use super::*;
use crate::protocol::{gemini_canvas, gemini_web};
use crate::upstream::gemini_canvas_client_types::GeminiCanvasDirectHttpApiKeyTransport;
use crate::upstream::header_map_helpers::header_map_string;
use rquest::header::{HeaderMap, HeaderValue};
use serde_json::{json, Value};
use std::collections::HashMap;
#[test]
fn resolve_gemini_canvas_direct_http_bootstrap_page_path_prefers_url_contract() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: None,
        session_id: None,
        language: "en-US".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: Some("/app/from-bootstrap".to_string()),
    };

    let page_path = resolve_gemini_canvas_direct_http_bootstrap_page_path(
        "https://gemini.google.com/app/4abc4e7577b6149f",
        &bootstrap,
    );

    assert_eq!(page_path, "/app/4abc4e7577b6149f");
}

#[test]
fn resolve_gemini_canvas_direct_http_bootstrap_page_path_falls_back_to_bootstrap_contract() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: None,
        session_id: None,
        language: "en-US".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: Some("/app/from-bootstrap".to_string()),
    };

    let page_path = resolve_gemini_canvas_direct_http_bootstrap_page_path(
        "https://gemini.google.com/share/example",
        &bootstrap,
    );

    assert_eq!(page_path, "/app/from-bootstrap");
}

#[test]
fn resolve_gemini_canvas_direct_http_bootstrap_page_path_defaults_to_app_contract() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: None,
        session_id: None,
        language: "en-US".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let page_path = resolve_gemini_canvas_direct_http_bootstrap_page_path(
        "https://gemini.google.com/share/example",
        &bootstrap,
    );

    assert_eq!(page_path, gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
}

#[test]
fn resolve_gemini_canvas_direct_http_referer_prefers_concrete_text_source_contract() {
    let referer = resolve_gemini_canvas_direct_http_referer(
        "https://gemini.google.com",
        "/app/from-bootstrap",
        "/app/from-program",
        true,
        false,
    );

    assert_eq!(referer, "https://gemini.google.com/app/from-program");
}

#[test]
fn resolve_gemini_canvas_direct_http_referer_falls_back_to_root_for_image_contract() {
    let referer = resolve_gemini_canvas_direct_http_referer(
        "https://gemini.google.com",
        "/app/from-bootstrap",
        gemini_web::GEMINI_WEB_DEFAULT_APP_PATH,
        false,
        true,
    );

    assert_eq!(referer, "https://gemini.google.com/");
}

#[test]
fn resolve_gemini_canvas_direct_http_referer_uses_bootstrap_page_for_media_contract() {
    let referer = resolve_gemini_canvas_direct_http_referer(
        "https://gemini.google.com",
        "/app/from-bootstrap",
        gemini_web::GEMINI_WEB_DEFAULT_APP_PATH,
        false,
        false,
    );

    assert_eq!(referer, "https://gemini.google.com/app/from-bootstrap");
}

#[test]
fn resolve_gemini_canvas_direct_http_preflight_source_path_prefers_program_app_path_contract() {
    let mut payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://gemini.google.com",
    );
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
    payload.extra_body = Some(HashMap::from([
        (
            "shareId".to_string(),
            Value::String("canvas-share-789".to_string()),
        ),
        (
            "appPath".to_string(),
            Value::String("/app/4abc4e7577b6149f".to_string()),
        ),
        (
            "conversationId".to_string(),
            Value::String("c_ignored_because_app_path_wins".to_string()),
        ),
    ]));

    let source_path = resolve_gemini_canvas_direct_http_preflight_source_path(&payload);

    assert_eq!(source_path, "/app/4abc4e7577b6149f");
}

#[test]
fn resolve_gemini_canvas_direct_http_preflight_source_path_falls_back_to_conversation_contract() {
    let mut payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://gemini.google.com",
    );
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
    payload.extra_body = Some(HashMap::from([
        (
            "shareId".to_string(),
            Value::String("canvas-share-789".to_string()),
        ),
        (
            "conversationId".to_string(),
            Value::String("c_4abc4e7577b6149f".to_string()),
        ),
    ]));

    let source_path = resolve_gemini_canvas_direct_http_preflight_source_path(&payload);

    assert_eq!(source_path, "/app/4abc4e7577b6149f");
}

#[test]
fn resolve_gemini_canvas_direct_http_preflight_source_path_defaults_to_app_contract() {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");

    let source_path = resolve_gemini_canvas_direct_http_preflight_source_path(&payload);

    assert_eq!(source_path, gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
}

#[test]
fn resolve_gemini_canvas_direct_http_stream_generate_model_header_prefers_harvested_contract() {
    let header = resolve_gemini_canvas_direct_http_stream_generate_model_header(
        Some("[1,\"HARVESTED\"]"),
        false,
        true,
    );

    assert_eq!(header, "[1,\"HARVESTED\"]");
}

#[test]
fn resolve_gemini_canvas_direct_http_stream_generate_model_header_uses_text_default_contract() {
    let header = resolve_gemini_canvas_direct_http_stream_generate_model_header(None, true, false);

    assert_eq!(
        header,
        gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER
    );
}

#[test]
fn resolve_gemini_canvas_direct_http_stream_generate_model_header_uses_media_default_contract() {
    let header = resolve_gemini_canvas_direct_http_stream_generate_model_header(None, false, false);

    assert_eq!(
        header,
        gemini_canvas::GEMINI_CANVAS_MEDIA_STREAM_GENERATE_MODEL_HEADER
    );
}

#[test]
fn gemini_canvas_direct_http_text_state_variant_preflight_specs_preserve_marker_order() {
    let specs = gemini_canvas_direct_http_text_state_variant_preflight_specs();

    assert_eq!(
        specs
            .iter()
            .map(|(_, _, _, marker)| *marker)
            .collect::<Vec<_>>(),
        vec![
            "side_nav_open_by_default",
            "popup_zs_visits_cooldown",
            "popup_zs_visits_cooldown",
            "current_popup_id",
            "current_popup_id",
        ]
    );
}

#[test]
fn gemini_canvas_direct_http_text_state_variant_preflight_specs_preserve_tail_values() {
    let specs = gemini_canvas_direct_http_text_state_variant_preflight_specs();

    assert_eq!(
        specs[0],
        (41usize, 40usize, Value::from(0), "side_nav_open_by_default")
    );
    assert_eq!(
        specs[1],
        (87usize, 86usize, Value::from(1), "popup_zs_visits_cooldown",)
    );
    assert_eq!(
        specs[2],
        (87usize, 86usize, Value::from(2), "popup_zs_visits_cooldown",)
    );
    assert_eq!(
        specs[3],
        (
            94usize,
            93usize,
            Value::String("NULL".to_string()),
            "current_popup_id",
        )
    );
    assert_eq!(
        specs[4],
        (
            94usize,
            93usize,
            Value::String("HUMAN_REVIEWER_DISCLOSURE".to_string()),
            "current_popup_id",
        )
    );
}

#[test]
fn gemini_canvas_direct_http_text_fast_version_preflight_spec_preserves_marker_contract() {
    let (state_len, tail_index, tail_value, marker) =
        gemini_canvas_direct_http_text_fast_version_preflight_spec();

    assert_eq!(state_len, 179usize);
    assert_eq!(tail_index, 178usize);
    assert_eq!(tail_value, Value::String("2025-12-16".to_string()));
    assert_eq!(marker, "enforce_default_to_fast_version");
}

#[test]
fn build_gemini_canvas_direct_http_text_generic_preflight_specs_preserve_rpcid_order() {
    let specs = build_gemini_canvas_direct_http_text_generic_preflight_specs("en-US");

    assert_eq!(specs.len(), 12);
    assert_eq!(specs[0].0, "otAQ7b");
    assert_eq!(specs[1].0, "sJBwce");
    assert_eq!(specs[2].0, "DYBcR");
    assert_eq!(specs[11].0, "CNgdBe");
    assert_eq!(
        specs[0].2,
        gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT
    );
    assert_eq!(
        specs[11].2,
        gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER
    );
}

#[test]
fn build_gemini_canvas_direct_http_text_generic_preflight_specs_embed_language_contract() {
    let specs = build_gemini_canvas_direct_http_text_generic_preflight_specs("zh-CN");

    assert_eq!(specs[2].1, json!(["zh-CN"]));
    assert_eq!(specs[6].1, json!([["zh-CN"], [1]]));
    assert_eq!(specs[8].1, json!([[0], ["zh-CN"]]));
    assert_eq!(specs[11].1, json!([1, ["zh-CN"], 0]));
}

#[test]
fn apply_gemini_canvas_direct_http_stream_generate_headers_preserves_text_contract() {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };
    let mut headers = HeaderMap::new();

    apply_gemini_canvas_direct_http_stream_generate_headers(
        &mut headers,
        &payload,
        "request-123",
        gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER,
        Some(&session),
        true,
    );

    assert_eq!(
        header_map_string(&headers, "accept").as_deref(),
        Some("*/*")
    );
    assert_eq!(
        header_map_string(&headers, "accept-language").as_deref(),
        Some("en-US")
    );
    assert_eq!(
        header_map_string(&headers, "cookie").as_deref(),
        Some("SID=abc; SAPISID=def")
    );
    assert_eq!(
        header_map_string(&headers, "x-goog-authuser").as_deref(),
        Some("1")
    );
    assert_eq!(
        header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_KEY).as_deref(),
        Some(gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER)
    );
    assert_eq!(
        header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_2_KEY).as_deref(),
        Some(gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER_2)
    );
    assert_eq!(
        header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY).as_deref(),
        Some(gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER_3)
    );
    assert_eq!(
        header_map_string(&headers, gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY).as_deref(),
        Some("[\"request-123\",1]")
    );
}
#[test]
fn gemini_canvas_direct_http_api_key_transports_prefer_query_for_clients6() {
    let transports = gemini_canvas_direct_http_api_key_transports(
        "https://geminiweb-pa.clients6.google.com/v1beta/models/gemini-2.5-flash-image-preview:generateContent",
    );

    assert_eq!(
        transports,
        &[
            GeminiCanvasDirectHttpApiKeyTransport::QueryOnly,
            GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly,
            GeminiCanvasDirectHttpApiKeyTransport::HeaderAndQuery,
        ]
    );
}

#[test]
fn gemini_canvas_direct_http_api_key_transports_prefer_header_for_googleapis() {
    let transports = gemini_canvas_direct_http_api_key_transports(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
    );

    assert_eq!(
        transports,
        &[
            GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly,
            GeminiCanvasDirectHttpApiKeyTransport::QueryOnly,
            GeminiCanvasDirectHttpApiKeyTransport::HeaderAndQuery,
        ]
    );
}

#[test]
fn gemini_canvas_public_page_api_key_fallbacks_keep_candidate_contract() {
    assert_eq!(GEMINI_CANVAS_PUBLIC_PAGE_API_KEY_FALLBACKS.len(), 6);
    assert!(GEMINI_CANVAS_PUBLIC_PAGE_API_KEY_FALLBACKS
        .iter()
        .all(|candidate| candidate.starts_with("AIza")));
}

#[test]
fn redact_gemini_canvas_api_key_for_logs_omits_secret_body() {
    assert_eq!(
        redact_gemini_canvas_api_key_for_logs("AIzaSyCqyCcs2R2e7AegGjvFAwG98wlamtbHvZY"),
        "<redacted>"
    );
}

#[test]
fn redact_gemini_canvas_url_for_logs_removes_credentials_query_and_fragment() {
    assert_eq!(
        redact_gemini_canvas_url_for_logs(
            "https://user:password@gemini.google.com/app?token=secret&authuser=1#state"
        ),
        "https://gemini.google.com/app"
    );
    assert_eq!(
        redact_gemini_canvas_url_for_logs("/app/next?token=secret#state"),
        "/app/next"
    );
    assert_eq!(
        redact_gemini_canvas_url_for_logs("data:text/plain,secret"),
        "<redacted-url>"
    );
}

#[test]
fn sensitive_header_presence_for_logs_never_returns_the_header_value() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-goog-api-key",
        HeaderValue::from_static("AIza-secret-value"),
    );
    assert_eq!(
        sensitive_header_presence_for_logs(&headers, "x-goog-api-key"),
        "<present>"
    );
}
