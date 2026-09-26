//! Image-edit diagnostic payload schemas and redacted stream contracts.
use crate::protocol::gemini_canvas;
use crate::upstream::gemini_canvas_runtime_error_helpers::compact_sanitized_response_preview;
use crate::upstream::header_map_helpers::header_map_string;
use rquest::header::HeaderMap;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn build_gemini_canvas_image_edit_template_miss_debug_snapshot(
    runtime_state_object_key: &str,
    image_edit_sidecar_key: Option<&str>,
    storage_state_has_image_edit_template: bool,
    storage_state_has_image_edit_template_legacy: bool,
    local_mirror_exists: bool,
    selected_template_source: Option<&str>,
    sidecar_has_template_key: bool,
) -> Value {
    json!({
        "runtimeStateObjectKey": runtime_state_object_key,
        "imageEditSidecarKey": image_edit_sidecar_key,
        "storageStateHasImageEditTemplate": storage_state_has_image_edit_template,
        "storageStateHasImageEditTemplateLegacy": storage_state_has_image_edit_template_legacy,
        "localMirrorExists": local_mirror_exists,
        "selectedTemplateSource": selected_template_source,
        "sidecarHasTemplateKey": sidecar_has_template_key,
    })
}

pub(crate) fn build_gemini_canvas_image_edit_template_pre_refresh_debug_extra(
    request_uuid: &str,
    runtime_state_object_key: &str,
    image_edit_sidecar_key: &str,
    selected_template_source: Option<&str>,
    storage_state_has_template: bool,
    remote_sidecar_has_template: bool,
    local_sidecar_has_template: bool,
    uploaded_refs: &[Value],
    seed: Value,
) -> Value {
    json!({
        "requestUuid": request_uuid,
        "runtimeStateObjectKey": runtime_state_object_key,
        "imageEditSidecarKey": image_edit_sidecar_key,
        "selectedTemplateSource": selected_template_source,
        "storageStateHasTemplate": storage_state_has_template,
        "remoteSidecarHasTemplate": remote_sidecar_has_template,
        "localSidecarHasTemplate": local_sidecar_has_template,
        "uploadedRefs": uploaded_refs,
        "seed": seed,
    })
}

pub(crate) fn build_gemini_canvas_image_edit_template_post_refresh_debug_extra(
    request_uuid: &str,
    runtime_state_object_key: &str,
    image_edit_sidecar_key: &str,
    bootstrap_build_label: Option<&str>,
    bootstrap_session_id: Option<&str>,
    bootstrap_language: Option<&str>,
) -> Value {
    json!({
        "requestUuid": request_uuid,
        "runtimeStateObjectKey": runtime_state_object_key,
        "imageEditSidecarKey": image_edit_sidecar_key,
        "bootstrapBuildLabel": bootstrap_build_label,
        "bootstrapSessionId": bootstrap_session_id,
        "bootstrapLanguage": bootstrap_language,
    })
}

pub(crate) fn build_gemini_canvas_image_edit_upload_debug_snapshot(
    request_contract: &str,
    response_meta: &str,
    response_body: &str,
    upload_url: Option<&str>,
    file_name: &str,
    mime_type: &str,
    byte_length: usize,
    sha256: &str,
    dimensions: Option<(u32, u32)>,
    debug_upload_file: Option<&str>,
    browser_reencode: Option<Value>,
    resource_path_override: Option<&str>,
) -> Value {
    let mut value = json!({
        "capturedAtMs": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default(),
        "requestContract": request_contract,
        "responseMeta": response_meta,
        "responseBody": response_body,
        "uploadUrl": upload_url,
        "fileName": file_name,
        "mimeType": mime_type,
        "byteLength": byte_length,
        "sha256": sha256,
        "dimensions": dimensions.map(|(width, height)| json!({ "width": width, "height": height })),
        "debugUploadFile": debug_upload_file,
        "browserReencode": browser_reencode,
    });
    if let Some(resource_path_override) = resource_path_override {
        value["resourcePathOverride"] = json!(resource_path_override);
    }
    value
}

pub(crate) fn build_gemini_canvas_image_edit_heavy_builder_debug_extra(
    request_uuid: &str,
    runtime_state_object_key: &str,
    image_edit_sidecar_key: &str,
    stream_request_contract: &str,
    bootstrap_build_label: Option<&str>,
    bootstrap_session_id: Option<&str>,
    bootstrap_language: Option<&str>,
) -> Value {
    json!({
        "requestUuid": request_uuid,
        "runtimeStateObjectKey": runtime_state_object_key,
        "imageEditSidecarKey": image_edit_sidecar_key,
        "streamRequestContract": stream_request_contract,
        "bootstrapBuildLabel": bootstrap_build_label,
        "bootstrapSessionId": bootstrap_session_id,
        "bootstrapLanguage": bootstrap_language,
    })
}

pub(crate) fn build_gemini_canvas_image_edit_stream_response_heavy_debug_extra(
    runtime_state_object_key: &str,
    request_uuid: &str,
    stream_request_contract: &str,
) -> Value {
    json!({
        "runtimeStateObjectKey": runtime_state_object_key,
        "requestUuid": request_uuid,
        "streamRequestContract": stream_request_contract,
    })
}

pub(crate) fn build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra(
    runtime_state_object_key: &str,
    allow_replay_template: bool,
) -> Value {
    json!({
        "runtimeStateObjectKey": runtime_state_object_key,
        "allowReplayTemplate": allow_replay_template,
    })
}

pub(crate) fn build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra(
    runtime_state_object_key: &str,
    bootstrap_build_label: Option<&str>,
    bootstrap_session_id: Option<&str>,
    bootstrap_language: Option<&str>,
) -> Value {
    json!({
        "runtimeStateObjectKey": runtime_state_object_key,
        "bootstrapBuildLabel": bootstrap_build_label,
        "bootstrapSessionId": bootstrap_session_id,
        "bootstrapLanguage": bootstrap_language,
    })
}

pub(crate) fn build_gemini_canvas_image_edit_uploaded_refs_debug_snapshot(
    uploaded_refs: &[gemini_canvas::GeminiCanvasUploadedFileRef],
) -> Vec<Value> {
    uploaded_refs
        .iter()
        .map(|entry| {
            json!({
                "resourcePath": entry.resource_path,
                "fileName": entry.file_name,
                "mimeType": entry.mime_type,
            })
        })
        .collect()
}

pub(crate) fn build_gemini_canvas_image_edit_seed_debug_snapshot(
    seed: Option<&gemini_canvas::GeminiCanvasStreamGenerateSeed>,
) -> Value {
    seed.map(|seed| {
        json!({
            "opaqueState": seed.opaque_state,
            "requestHex": seed.request_hex,
            "requestUuid": seed.request_uuid,
        })
    })
    .unwrap_or(Value::Null)
}

pub(crate) fn build_gemini_canvas_image_edit_stream_request_contract(
    stream_url: &str,
    bootstrap_url: &str,
    page_path: &str,
    mode_index: i64,
    is_text_mode: bool,
    is_image_mode: bool,
    headers: &HeaderMap,
) -> String {
    format!(
        "stream_url={stream_url}, bootstrap_url={bootstrap_url}, page_path={page_path}, mode_index={}, is_text_mode={}, is_image_mode={}, origin={}, referer={}, x-origin={}, authorization={}, cookie={}",
        mode_index,
        is_text_mode,
        is_image_mode,
        header_map_string(headers, "origin").unwrap_or_else(|| "<none>".to_string()),
        header_map_string(headers, "referer").unwrap_or_else(|| "<none>".to_string()),
        header_map_string(headers, "x-origin").unwrap_or_else(|| "<none>".to_string()),
        if headers.contains_key("authorization") {
            "<present>"
        } else {
            "<none>"
        },
        if headers.contains_key("cookie") {
            "<present>"
        } else {
            "<none>"
        },
    )
}

pub(crate) fn build_gemini_canvas_image_edit_stream_response_meta(
    final_url: &str,
    location: Option<&str>,
    content_type: Option<&str>,
    body_text: &str,
) -> String {
    format!(
        "final_url={final_url}, location={}, content_type={}, body_preview={}",
        location.unwrap_or("<none>"),
        content_type.unwrap_or("<none>"),
        compact_sanitized_response_preview(body_text, 240)
    )
}
