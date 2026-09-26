use super::image_edit_local_helpers_test_support::make_payload;
use super::*;
use serde_json::json;
use std::collections::HashMap;
#[test]
fn gemini_canvas_image_edit_upload_base_url_appends_trailing_slash() {
    let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    payload.extra_body = Some(HashMap::from([(
        "imageEditUploadBaseUrl".to_string(),
        json!(" https://upload.example.com/base "),
    )]));

    assert_eq!(
        gemini_canvas_image_edit_upload_base_url(&payload),
        "https://upload.example.com/base/"
    );
}

#[test]
fn resolved_gemini_canvas_browser_runtime_state_object_key_prefers_payload_override() {
    let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    payload.extra_body = Some(HashMap::from([(
        "browserRuntimeStateObjectKey".to_string(),
        json!("browser/runtime.json"),
    )]));
    let runtime = gemini_canvas::GeminiCanvasRuntime {
        runtime_state_object_key: "default/runtime.json".to_string(),
        share_id: "share".to_string(),
        api_base_url: "https://gemini.google.com".to_string(),
    };

    assert_eq!(
        resolved_gemini_canvas_browser_runtime_state_object_key(&payload, &runtime),
        "browser/runtime.json"
    );
}
#[test]
fn apply_gemini_canvas_image_edit_upload_base_headers_preserves_common_contract() {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };
    let mut headers = HeaderMap::new();

    apply_gemini_canvas_image_edit_upload_base_headers(
        &mut headers,
        &payload,
        &session,
        "https://gemini.google.com",
        "https://gemini.google.com/app",
        "push-id-123",
        "client-pctx-456",
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
        header_map_string(&headers, "origin").as_deref(),
        Some("https://gemini.google.com")
    );
    assert_eq!(
        header_map_string(&headers, "referer").as_deref(),
        Some("https://gemini.google.com/app")
    );
    assert_eq!(
        header_map_string(&headers, "sec-fetch-site").as_deref(),
        Some("same-site")
    );
    assert_eq!(
        header_map_string(&headers, "push-id").as_deref(),
        Some("push-id-123")
    );
    assert_eq!(
        header_map_string(&headers, "x-client-pctx").as_deref(),
        Some("client-pctx-456")
    );
    assert_eq!(
        header_map_string(&headers, "x-tenant-id").as_deref(),
        Some("bard-storage")
    );
}

#[test]
fn apply_gemini_canvas_image_edit_upload_start_headers_preserves_contract() {
    let mut headers = HeaderMap::new();

    apply_gemini_canvas_image_edit_upload_start_headers(&mut headers, 1234);

    assert_eq!(
        header_map_string(&headers, "x-goog-upload-protocol").as_deref(),
        Some("resumable")
    );
    assert_eq!(
        header_map_string(&headers, "x-goog-upload-command").as_deref(),
        Some("start")
    );
    assert_eq!(
        header_map_string(&headers, "x-goog-upload-header-content-length").as_deref(),
        Some("1234")
    );
    assert_eq!(
        header_map_string(&headers, "content-type").as_deref(),
        Some("application/x-www-form-urlencoded;charset=UTF-8")
    );
}

#[test]
fn apply_gemini_canvas_image_edit_upload_finalize_headers_preserves_contract() {
    let mut headers = HeaderMap::new();

    apply_gemini_canvas_image_edit_upload_finalize_headers(&mut headers);

    assert_eq!(
        header_map_string(&headers, "x-goog-upload-command").as_deref(),
        Some("upload, finalize")
    );
    assert_eq!(
        header_map_string(&headers, "x-goog-upload-offset").as_deref(),
        Some("0")
    );
    assert_eq!(
        header_map_string(&headers, "content-type").as_deref(),
        Some("application/x-www-form-urlencoded;charset=utf-8")
    );
}

#[test]
fn extract_gemini_canvas_image_edit_upload_url_prefers_primary_contract() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-goog-upload-url",
        rquest::header::HeaderValue::from_static(
            " https://push.clients6.google.com/upload/primary ",
        ),
    );
    headers.insert(
        "x-goog-upload-control-url",
        rquest::header::HeaderValue::from_static(
            "https://push.clients6.google.com/upload/fallback",
        ),
    );

    let upload_url = extract_gemini_canvas_image_edit_upload_url(&headers);

    assert_eq!(
        upload_url.as_deref(),
        Some("https://push.clients6.google.com/upload/primary")
    );
}

#[test]
fn extract_gemini_canvas_image_edit_upload_url_falls_back_to_control_contract() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-goog-upload-control-url",
        rquest::header::HeaderValue::from_static(
            "https://push.clients6.google.com/upload/fallback",
        ),
    );

    let upload_url = extract_gemini_canvas_image_edit_upload_url(&headers);

    assert_eq!(
        upload_url.as_deref(),
        Some("https://push.clients6.google.com/upload/fallback")
    );
}

#[test]
fn extract_gemini_canvas_image_edit_upload_url_rejects_blank_contract() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-goog-upload-url",
        rquest::header::HeaderValue::from_static("   "),
    );

    let upload_url = extract_gemini_canvas_image_edit_upload_url(&headers);

    assert_eq!(upload_url, None);
}

#[test]
fn extract_gemini_canvas_image_edit_resource_path_prefers_override_contract() {
    let resource_path = extract_gemini_canvas_image_edit_resource_path(
        Some("/contrib_service/ttl_1d/override"),
        " /contrib_service/ttl_1d/body ",
    );

    assert_eq!(
        resource_path.as_deref(),
        Some("/contrib_service/ttl_1d/override")
    );
}

#[test]
fn extract_gemini_canvas_image_edit_resource_path_falls_back_to_body_contract() {
    let resource_path =
        extract_gemini_canvas_image_edit_resource_path(None, " /contrib_service/ttl_1d/body ");

    assert_eq!(
        resource_path.as_deref(),
        Some("/contrib_service/ttl_1d/body")
    );
}

#[test]
fn extract_gemini_canvas_image_edit_resource_path_rejects_invalid_contract() {
    let resource_path = extract_gemini_canvas_image_edit_resource_path(None, "not-a-contrib-path");

    assert_eq!(resource_path, None);
}
