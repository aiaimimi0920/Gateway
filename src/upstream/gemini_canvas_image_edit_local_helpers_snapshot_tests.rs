use super::*;
use serde_json::json;
#[test]
fn build_gemini_canvas_image_edit_template_miss_debug_snapshot_preserves_contract() {
    let snapshot = build_gemini_canvas_image_edit_template_miss_debug_snapshot(
        "credential-runtime/gemini-canvas/runtime.json",
        Some("credential-runtime/gemini-canvas/image-edit-sidecar.json"),
        true,
        false,
        true,
        Some("remote-sidecar"),
        true,
    );

    assert_eq!(
        snapshot["runtimeStateObjectKey"],
        "credential-runtime/gemini-canvas/runtime.json"
    );
    assert_eq!(
        snapshot["imageEditSidecarKey"],
        "credential-runtime/gemini-canvas/image-edit-sidecar.json"
    );
    assert_eq!(snapshot["storageStateHasImageEditTemplate"], true);
    assert_eq!(snapshot["storageStateHasImageEditTemplateLegacy"], false);
    assert_eq!(snapshot["localMirrorExists"], true);
    assert_eq!(snapshot["selectedTemplateSource"], "remote-sidecar");
    assert_eq!(snapshot["sidecarHasTemplateKey"], true);
}

#[test]
fn build_gemini_canvas_image_edit_template_miss_debug_snapshot_preserves_nullables() {
    let snapshot = build_gemini_canvas_image_edit_template_miss_debug_snapshot(
        "credential-runtime/gemini-canvas/runtime.json",
        None,
        false,
        false,
        false,
        None,
        false,
    );

    assert!(snapshot["imageEditSidecarKey"].is_null());
    assert_eq!(snapshot["storageStateHasImageEditTemplate"], false);
    assert_eq!(snapshot["storageStateHasImageEditTemplateLegacy"], false);
    assert_eq!(snapshot["localMirrorExists"], false);
    assert!(snapshot["selectedTemplateSource"].is_null());
    assert_eq!(snapshot["sidecarHasTemplateKey"], false);
}

#[test]
fn build_gemini_canvas_image_edit_template_pre_refresh_debug_extra_preserves_contract() {
    let uploaded_refs = vec![json!({
        "resourcePath": "contrib-service://image-edit-upload/asset-1",
        "fileName": "source.png",
        "mimeType": "image/png",
    })];
    let seed_snapshot = json!({
        "opaqueState": "opaque-state",
        "requestHex": "616263",
        "requestUuid": "seed-request-uuid",
    });

    let extra = build_gemini_canvas_image_edit_template_pre_refresh_debug_extra(
        "request-uuid",
        "credential-runtime/gemini-canvas/runtime.json",
        "credential-runtime/gemini-canvas/image-edit-sidecar.json",
        Some("remote-sidecar"),
        true,
        false,
        true,
        &uploaded_refs,
        seed_snapshot,
    );

    assert_eq!(extra["requestUuid"], "request-uuid");
    assert_eq!(
        extra["runtimeStateObjectKey"],
        "credential-runtime/gemini-canvas/runtime.json"
    );
    assert_eq!(
        extra["imageEditSidecarKey"],
        "credential-runtime/gemini-canvas/image-edit-sidecar.json"
    );
    assert_eq!(extra["selectedTemplateSource"], "remote-sidecar");
    assert_eq!(extra["storageStateHasTemplate"], true);
    assert_eq!(extra["remoteSidecarHasTemplate"], false);
    assert_eq!(extra["localSidecarHasTemplate"], true);
    assert_eq!(
        extra["uploadedRefs"][0]["resourcePath"],
        "contrib-service://image-edit-upload/asset-1"
    );
    assert_eq!(extra["uploadedRefs"][0]["fileName"], "source.png");
    assert_eq!(extra["uploadedRefs"][0]["mimeType"], "image/png");
    assert_eq!(extra["seed"]["opaqueState"], "opaque-state");
    assert_eq!(extra["seed"]["requestHex"], "616263");
    assert_eq!(extra["seed"]["requestUuid"], "seed-request-uuid");
}

#[test]
fn build_gemini_canvas_image_edit_template_pre_refresh_debug_extra_preserves_nullables() {
    let extra = build_gemini_canvas_image_edit_template_pre_refresh_debug_extra(
        "request-uuid",
        "credential-runtime/gemini-canvas/runtime.json",
        "credential-runtime/gemini-canvas/image-edit-sidecar.json",
        None,
        false,
        false,
        false,
        &[],
        Value::Null,
    );

    assert!(extra["selectedTemplateSource"].is_null());
    assert_eq!(extra["storageStateHasTemplate"], false);
    assert_eq!(extra["remoteSidecarHasTemplate"], false);
    assert_eq!(extra["localSidecarHasTemplate"], false);
    assert_eq!(extra["uploadedRefs"], json!([]));
    assert!(extra["seed"].is_null());
}

#[test]
fn build_gemini_canvas_image_edit_template_post_refresh_debug_extra_preserves_contract() {
    let extra = build_gemini_canvas_image_edit_template_post_refresh_debug_extra(
        "request-uuid",
        "credential-runtime/gemini-canvas/runtime.json",
        "credential-runtime/gemini-canvas/image-edit-sidecar.json",
        Some("build-label"),
        Some("session-id"),
        Some("zh-CN"),
    );

    assert_eq!(extra["requestUuid"], "request-uuid");
    assert_eq!(
        extra["runtimeStateObjectKey"],
        "credential-runtime/gemini-canvas/runtime.json"
    );
    assert_eq!(
        extra["imageEditSidecarKey"],
        "credential-runtime/gemini-canvas/image-edit-sidecar.json"
    );
    assert_eq!(extra["bootstrapBuildLabel"], "build-label");
    assert_eq!(extra["bootstrapSessionId"], "session-id");
    assert_eq!(extra["bootstrapLanguage"], "zh-CN");
}

#[test]
fn build_gemini_canvas_image_edit_template_post_refresh_debug_extra_preserves_nullables() {
    let extra = build_gemini_canvas_image_edit_template_post_refresh_debug_extra(
        "request-uuid",
        "credential-runtime/gemini-canvas/runtime.json",
        "credential-runtime/gemini-canvas/image-edit-sidecar.json",
        None,
        None,
        None,
    );

    assert_eq!(extra["requestUuid"], "request-uuid");
    assert!(extra["bootstrapBuildLabel"].is_null());
    assert!(extra["bootstrapSessionId"].is_null());
    assert!(extra["bootstrapLanguage"].is_null());
}

#[test]
fn build_gemini_canvas_image_edit_upload_debug_snapshot_preserves_start_contract() {
    let browser_reencode = json!({
        "used": true,
        "stdout": "encoded",
    });
    let snapshot = build_gemini_canvas_image_edit_upload_debug_snapshot(
        "start-contract",
        "status=200",
        "{\"ok\":true}",
        Some("https://push.clients6.google.com/upload/session"),
        "source.png",
        "image/png",
        1234,
        "deadbeef",
        Some((512, 768)),
        Some(".runtime/gemini-canvas-image-edit-upload-debug-deadbeef.jpg"),
        Some(browser_reencode),
        None,
    );

    assert_eq!(snapshot["requestContract"], "start-contract");
    assert_eq!(snapshot["responseMeta"], "status=200");
    assert_eq!(snapshot["responseBody"], "{\"ok\":true}");
    assert_eq!(
        snapshot["uploadUrl"],
        "https://push.clients6.google.com/upload/session"
    );
    assert_eq!(snapshot["fileName"], "source.png");
    assert_eq!(snapshot["mimeType"], "image/png");
    assert_eq!(snapshot["byteLength"], 1234);
    assert_eq!(snapshot["sha256"], "deadbeef");
    assert_eq!(snapshot["dimensions"]["width"], 512);
    assert_eq!(snapshot["dimensions"]["height"], 768);
    assert_eq!(
        snapshot["debugUploadFile"],
        ".runtime/gemini-canvas-image-edit-upload-debug-deadbeef.jpg"
    );
    assert_eq!(snapshot["browserReencode"]["used"], true);
    assert_eq!(snapshot["browserReencode"]["stdout"], "encoded");
    assert!(snapshot.get("resourcePathOverride").is_none());
}

#[test]
fn build_gemini_canvas_image_edit_upload_debug_snapshot_preserves_finalize_override() {
    let snapshot = build_gemini_canvas_image_edit_upload_debug_snapshot(
        "finalize-contract",
        "status=200",
        "{\"resourcePath\":\"contrib-service://asset\"}",
        Some("https://push.clients6.google.com/upload/session"),
        "source.png",
        "image/png",
        1234,
        "deadbeef",
        None,
        None,
        None,
        Some("contrib-service://override"),
    );

    assert!(snapshot["dimensions"].is_null());
    assert!(snapshot["debugUploadFile"].is_null());
    assert!(snapshot["browserReencode"].is_null());
    assert_eq!(
        snapshot["resourcePathOverride"],
        "contrib-service://override"
    );
}

#[test]
fn build_gemini_canvas_image_edit_heavy_builder_debug_extra_preserves_contract() {
    let extra = build_gemini_canvas_image_edit_heavy_builder_debug_extra(
        "request-uuid",
        "credential-runtime/gemini-canvas/runtime.json",
        "credential-runtime/gemini-canvas/image-edit-sidecar.json",
        "stream-request-contract",
        Some("build-label"),
        Some("session-id"),
        Some("zh-CN"),
    );

    assert_eq!(extra["requestUuid"], "request-uuid");
    assert_eq!(
        extra["runtimeStateObjectKey"],
        "credential-runtime/gemini-canvas/runtime.json"
    );
    assert_eq!(
        extra["imageEditSidecarKey"],
        "credential-runtime/gemini-canvas/image-edit-sidecar.json"
    );
    assert_eq!(extra["streamRequestContract"], "stream-request-contract");
    assert_eq!(extra["bootstrapBuildLabel"], "build-label");
    assert_eq!(extra["bootstrapSessionId"], "session-id");
    assert_eq!(extra["bootstrapLanguage"], "zh-CN");
}

#[test]
fn build_gemini_canvas_image_edit_heavy_builder_debug_extra_preserves_nullables() {
    let extra = build_gemini_canvas_image_edit_heavy_builder_debug_extra(
        "request-uuid",
        "credential-runtime/gemini-canvas/runtime.json",
        "credential-runtime/gemini-canvas/image-edit-sidecar.json",
        "stream-request-contract",
        None,
        None,
        None,
    );

    assert_eq!(extra["streamRequestContract"], "stream-request-contract");
    assert!(extra["bootstrapBuildLabel"].is_null());
    assert!(extra["bootstrapSessionId"].is_null());
    assert!(extra["bootstrapLanguage"].is_null());
}

#[test]
fn build_gemini_canvas_image_edit_stream_response_heavy_debug_extra_preserves_contract() {
    let extra = build_gemini_canvas_image_edit_stream_response_heavy_debug_extra(
        "credential-runtime/gemini-canvas/runtime.json",
        "request-uuid",
        "stream-request-contract",
    );

    assert_eq!(
        extra["runtimeStateObjectKey"],
        "credential-runtime/gemini-canvas/runtime.json"
    );
    assert_eq!(extra["requestUuid"], "request-uuid");
    assert_eq!(extra["streamRequestContract"], "stream-request-contract");
}

#[test]
fn build_gemini_canvas_image_edit_stream_response_heavy_debug_extra_preserves_distinct_values() {
    let extra = build_gemini_canvas_image_edit_stream_response_heavy_debug_extra(
        "credential-runtime/gemini-canvas/browser-runtime.json",
        "browser-request-uuid",
        "browser-stream-request-contract",
    );

    assert_eq!(
        extra["runtimeStateObjectKey"],
        "credential-runtime/gemini-canvas/browser-runtime.json"
    );
    assert_eq!(extra["requestUuid"], "browser-request-uuid");
    assert_eq!(
        extra["streamRequestContract"],
        "browser-stream-request-contract"
    );
}

#[test]
fn build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra_preserves_contract(
) {
    let extra = build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra(
        "credential-runtime/gemini-canvas/runtime.json",
        true,
    );

    assert_eq!(
        extra["runtimeStateObjectKey"],
        "credential-runtime/gemini-canvas/runtime.json"
    );
    assert_eq!(extra["allowReplayTemplate"], true);
}

#[test]
fn build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra_preserves_false_flag(
) {
    let extra = build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra(
        "credential-runtime/gemini-canvas/runtime.json",
        false,
    );

    assert_eq!(extra["allowReplayTemplate"], false);
}

#[test]
fn build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra_preserves_contract(
) {
    let extra = build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra(
        "credential-runtime/gemini-canvas/runtime.json",
        Some("build-label"),
        Some("session-id"),
        Some("zh-CN"),
    );

    assert_eq!(
        extra["runtimeStateObjectKey"],
        "credential-runtime/gemini-canvas/runtime.json"
    );
    assert_eq!(extra["bootstrapBuildLabel"], "build-label");
    assert_eq!(extra["bootstrapSessionId"], "session-id");
    assert_eq!(extra["bootstrapLanguage"], "zh-CN");
}

#[test]
fn build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra_preserves_nullables(
) {
    let extra = build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra(
        "credential-runtime/gemini-canvas/runtime.json",
        None,
        None,
        None,
    );

    assert_eq!(
        extra["runtimeStateObjectKey"],
        "credential-runtime/gemini-canvas/runtime.json"
    );
    assert!(extra["bootstrapBuildLabel"].is_null());
    assert!(extra["bootstrapSessionId"].is_null());
    assert!(extra["bootstrapLanguage"].is_null());
}

#[test]
fn build_gemini_canvas_image_edit_uploaded_refs_debug_snapshot_preserves_contract() {
    let uploaded_refs = vec![gemini_canvas::GeminiCanvasUploadedFileRef {
        resource_path: "/contrib_service/ttl_1d/example".to_string(),
        mime_type: "image/png".to_string(),
        file_name: "source.png".to_string(),
    }];

    let snapshot = build_gemini_canvas_image_edit_uploaded_refs_debug_snapshot(&uploaded_refs);

    assert_eq!(snapshot.len(), 1);
    assert_eq!(
        snapshot[0]["resourcePath"],
        "/contrib_service/ttl_1d/example"
    );
    assert_eq!(snapshot[0]["fileName"], "source.png");
    assert_eq!(snapshot[0]["mimeType"], "image/png");
}

#[test]
fn build_gemini_canvas_image_edit_uploaded_refs_debug_snapshot_preserves_empty_contract() {
    let snapshot = build_gemini_canvas_image_edit_uploaded_refs_debug_snapshot(&[]);
    assert_eq!(snapshot, Vec::<Value>::new());
}

#[test]
fn build_gemini_canvas_image_edit_seed_debug_snapshot_preserves_contract() {
    let seed = gemini_canvas::GeminiCanvasStreamGenerateSeed {
        opaque_state: Some("opaque-state".to_string()),
        request_hex: Some("0123456789abcdef0123456789abcdef".to_string()),
        request_uuid: Some("seed-request-id".to_string()),
    };

    let snapshot = build_gemini_canvas_image_edit_seed_debug_snapshot(Some(&seed));

    assert_eq!(snapshot["opaqueState"], "opaque-state");
    assert_eq!(snapshot["requestHex"], "0123456789abcdef0123456789abcdef");
    assert_eq!(snapshot["requestUuid"], "seed-request-id");
}

#[test]
fn build_gemini_canvas_image_edit_seed_debug_snapshot_preserves_null_contract() {
    let snapshot = build_gemini_canvas_image_edit_seed_debug_snapshot(None);
    assert_eq!(snapshot, Value::Null);
}

#[test]
fn build_gemini_canvas_image_edit_stream_request_contract_preserves_shape() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "origin",
        "https://gemini.google.com".parse().expect("origin header"),
    );
    headers.insert(
        "referer",
        "https://gemini.google.com/app"
            .parse()
            .expect("referer header"),
    );
    headers.insert(
        "x-origin",
        "https://gemini.google.com"
            .parse()
            .expect("x-origin header"),
    );
    headers.insert(
        "authorization",
        "Bearer token".parse().expect("auth header"),
    );
    headers.insert("cookie", "SID=abc".parse().expect("cookie header"));

    let contract = build_gemini_canvas_image_edit_stream_request_contract(
        "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate",
        "https://gemini.google.com/_/BardChatUi/data/batchexecute",
        "/app",
        3,
        false,
        true,
        &headers,
    );

    assert_eq!(
        contract,
        "stream_url=https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate, bootstrap_url=https://gemini.google.com/_/BardChatUi/data/batchexecute, page_path=/app, mode_index=3, is_text_mode=false, is_image_mode=true, origin=https://gemini.google.com, referer=https://gemini.google.com/app, x-origin=https://gemini.google.com, authorization=<present>, cookie=<present>"
    );
}

#[test]
fn build_gemini_canvas_image_edit_stream_request_contract_marks_missing_headers() {
    let contract = build_gemini_canvas_image_edit_stream_request_contract(
        "https://gemini.google.com/stream",
        "https://gemini.google.com/bootstrap",
        "/app",
        0,
        true,
        false,
        &HeaderMap::new(),
    );

    assert_eq!(
        contract,
        "stream_url=https://gemini.google.com/stream, bootstrap_url=https://gemini.google.com/bootstrap, page_path=/app, mode_index=0, is_text_mode=true, is_image_mode=false, origin=<none>, referer=<none>, x-origin=<none>, authorization=<none>, cookie=<none>"
    );
}

#[test]
fn build_gemini_canvas_image_edit_stream_response_meta_preserves_shape() {
    let meta = build_gemini_canvas_image_edit_stream_response_meta(
        "https://gemini.google.com/stream/final",
        Some("https://gemini.google.com/next"),
        Some("text/plain"),
        "abcdefghijklmnopqrstuvwxyz",
    );

    assert_eq!(
        meta,
        "final_url=https://gemini.google.com/stream/final, location=https://gemini.google.com/next, content_type=text/plain, body_preview=abcdefghijklmnopqrstuvwxyz"
    );
}

#[test]
fn build_gemini_canvas_image_edit_stream_response_meta_marks_missing_fields() {
    let meta = build_gemini_canvas_image_edit_stream_response_meta(
        "https://gemini.google.com/stream/final",
        None,
        None,
        "",
    );

    assert_eq!(
        meta,
        "final_url=https://gemini.google.com/stream/final, location=<none>, content_type=<none>, body_preview=<empty>"
    );
}
