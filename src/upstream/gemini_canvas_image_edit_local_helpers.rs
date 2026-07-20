use futures::StreamExt;
use image::GenericImageView;
use rquest::header::HeaderMap;
use rquest::{Client, Method};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

use crate::error::{classify_network_error, GatewayError};
use crate::object_storage::gateway_object_storage;
use crate::protocol::gemini_web;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel;
use crate::upstream::gemini_canvas_direct_http_helpers::{
    fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale,
    redact_gemini_canvas_api_key_for_logs,
};
use crate::upstream::gemini_canvas_error_helpers::classify_gemini_canvas_pure_http_error;
use crate::upstream::gemini_canvas_followup_types::GeminiCanvasImageEditFollowupContext;
use crate::upstream::gemini_canvas_request_headers::{
    apply_gemini_canvas_browserish_text_headers, apply_gemini_canvas_cookie_header,
    apply_gemini_canvas_response_cookies, apply_gemini_canvas_signaler_headers,
};
use crate::upstream::gemini_canvas_runtime_error_helpers::{
    append_gateway_error_summary, summarize_gateway_error,
};
use crate::upstream::gemini_canvas_runtime_helpers::{
    current_unix_timestamp_i64, gemini_canvas_http_origin, gemini_canvas_signaler_zx_token,
};
use crate::upstream::header_map_helpers::header_map_string;
use crate::upstream::response_preview_helpers::{
    compact_response_preview, truncate_response_preview,
};
use crate::{protocol::gemini_canvas, routing::candidate::ProviderAccountPayload};

pub(crate) fn gemini_canvas_image_edit_force_heavy_only_enabled() -> bool {
    std::env::var_os("GEMINI_CANVAS_IMAGE_EDIT_FORCE_HEAVY_ONLY").is_some()
}

pub(crate) fn gemini_canvas_image_edit_upload_base_url(payload: &ProviderAccountPayload) -> String {
    let from_extra_body = payload
        .extra_body
        .as_ref()
        .and_then(|extra| {
            extra
                .get("imageEditUploadBaseUrl")
                .or_else(|| extra.get("uploadBaseUrl"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let from_env = std::env::var("GEMINI_CANVAS_IMAGE_EDIT_UPLOAD_BASE_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let base = from_extra_body
        .or(from_env)
        .unwrap_or_else(|| "https://push.clients6.google.com/upload/".to_string());
    if base.ends_with('/') {
        base
    } else {
        format!("{base}/")
    }
}

pub(crate) fn gemini_canvas_image_edit_missing_push_id_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image edit upload requires bootstrap push_id from the /app page.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_missing_push_id")
}

pub(crate) fn gemini_canvas_image_edit_missing_client_pctx_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image edit upload requires bootstrap client_pctx from the /app page.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_missing_client_pctx")
}

pub(crate) fn gemini_canvas_image_edit_missing_upload_url_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image edit upload start response did not expose an upload URL.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_missing_upload_url")
}

pub(crate) fn gemini_canvas_image_edit_missing_resource_path_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image edit upload finalize response did not return a contrib_service resource path.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_missing_resource_path")
}

pub(crate) fn gemini_canvas_image_edit_post_ack_missing_app_url_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image edit post-ack follow-up requires a concrete signaler app url.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_post_ack_missing_app_url")
}

pub(crate) fn gemini_canvas_image_edit_post_ack_bootstrap_missing_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image edit post-ack follow-up could not bootstrap /app and payload cache did not provide a fallback bootstrap.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_post_ack_bootstrap_missing")
}

pub(crate) fn gemini_canvas_image_edit_conversation_bootstrap_missing_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image edit conversation follow-up could not bootstrap /app and payload cache did not provide a fallback bootstrap.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_conversation_bootstrap_missing")
}

pub(crate) fn gemini_canvas_image_page_refresh_bootstrap_missing_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image page refresh could not bootstrap /app and payload cache did not provide a fallback bootstrap.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_page_refresh_bootstrap_missing")
}

pub(crate) fn gemini_canvas_image_edit_signaler_missing_account_id_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image edit signaler bootstrap did not expose S06Grb account id.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_signaler_missing_account_id")
}

pub(crate) fn gemini_canvas_image_edit_signaler_missing_api_key_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas image edit signaler bootstrap did not expose a Google API key.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_signaler_missing_api_key")
}

pub(crate) fn gemini_canvas_image_edit_signaler_all_keys_failed_error(
    failures: &str,
) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas image edit signaler bootstrap exhausted all Google API key candidates. failures={failures}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_signaler_all_keys_failed")
}

pub(crate) fn gemini_canvas_image_edit_signaler_handoff_ready_error(
    distinct_app_paths: usize,
    next_aid: u64,
    last_body_preview: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas image edit signaler reached concrete app paths but has not surfaced a usable image asset yet. distinct_app_paths={distinct_app_paths}; next_aid={next_aid}; last_body_preview={last_body_preview}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_signaler_handoff_ready")
}

pub(crate) fn gemini_canvas_image_edit_signaler_handoff_ready(
    distinct_app_paths: usize,
    first_app_path_elapsed: Option<Duration>,
) -> bool {
    let Some(first_app_path_elapsed) = first_app_path_elapsed else {
        return false;
    };
    distinct_app_paths > 0
        && (distinct_app_paths >= 3 || first_app_path_elapsed >= Duration::from_secs(120))
}

pub(crate) fn try_finish_gemini_canvas_image_edit_signaler_handoff_ready(
    distinct_app_paths: usize,
    first_app_path_elapsed: Option<Duration>,
    last_body_preview: Option<&str>,
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
    body: &str,
) -> Option<GatewayError> {
    if !gemini_canvas_image_edit_signaler_handoff_ready(distinct_app_paths, first_app_path_elapsed)
    {
        return None;
    }

    let last_body_preview_text = last_body_preview.unwrap_or("<none>");
    let handoff_error = gemini_canvas_image_edit_signaler_handoff_ready_error(
        distinct_app_paths,
        channel.next_aid,
        last_body_preview_text,
    );
    let handoff_message = handoff_error.message.clone();
    append_gemini_canvas_image_edit_trace("signaler.handoff-ready", handoff_message.as_str());
    if let Some(context) = edit_context {
        record_gemini_canvas_image_edit_signaler_followup_state_from_body(
            context,
            session,
            channel,
            locale_hint,
            body,
        );
    }
    Some(handoff_error)
}

pub(crate) fn gemini_canvas_image_edit_signaler_missing_asset_error(
    next_aid: u64,
    failures: &str,
    last_body_preview: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas image edit signaler poll did not expose a usable media asset. next_aid={next_aid}; failures={failures}; last_body_preview={last_body_preview}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_signaler_missing_asset")
}

pub(crate) struct GeminiCanvasImageEditSignalerBootstrapMaterial {
    pub(crate) account_id: String,
    pub(crate) api_key_candidates: Vec<String>,
}

pub(crate) fn resolve_gemini_canvas_image_edit_signaler_bootstrap_material(
    payload: &ProviderAccountPayload,
    storage_state: &Value,
    page_body: &str,
) -> Result<GeminiCanvasImageEditSignalerBootstrapMaterial, GatewayError> {
    let page_account_id = gemini_canvas::extract_signaler_account_id_from_page_blob(page_body);
    let runtime_account_id =
        gemini_canvas::extract_signaler_account_id_from_runtime_state(storage_state);
    let account_id = page_account_id
        .clone()
        .or_else(|| runtime_account_id.clone())
        .ok_or_else(|| {
            let runtime_top_keys = storage_state
                .as_object()
                .map(|object| {
                    object
                        .keys()
                        .take(12)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "<non-object>".to_string());
            append_gateway_error_summary(
                append_gateway_error_summary(
                    append_gateway_error_summary(
                        gemini_canvas_image_edit_signaler_missing_account_id_error(),
                        "page_account_id_present",
                        Some(if page_account_id.is_some() {
                            "true"
                        } else {
                            "false"
                        }),
                    ),
                    "runtime_account_id_present",
                    Some(if runtime_account_id.is_some() {
                        "true"
                    } else {
                        "false"
                    }),
                ),
                "runtime_top_keys",
                Some(&runtime_top_keys),
            )
        })?;
    let mut api_key_candidates = gemini_canvas::direct_http_google_api_keys(payload, storage_state);
    for candidate in gemini_canvas::extract_google_api_keys_from_page_blob(page_body) {
        if !api_key_candidates
            .iter()
            .any(|existing| existing == &candidate)
        {
            api_key_candidates.push(candidate);
        }
    }
    if api_key_candidates.is_empty() {
        return Err(gemini_canvas_image_edit_signaler_missing_api_key_error());
    }

    Ok(GeminiCanvasImageEditSignalerBootstrapMaterial {
        account_id,
        api_key_candidates,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn gemini_canvas_image_edit_conversation_followup_failed_error(
    bootstrap_page: &str,
    attempt: usize,
    prompt_preview: &str,
    entries_preview: &str,
    last_probe_preview: &str,
    last_full_preview: &str,
    last_completion_preview: &str,
    parity_probe_preview: &str,
    parity_full_preview: &str,
    parity_o30_preview: &str,
    parity_k4_preview: &str,
    failures: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas image edit conversation follow-up did not expose a usable image asset. bootstrap_page={bootstrap_page}; attempts={attempt}; prompt_preview={prompt_preview}; entries_preview={entries_preview}; last_probe_preview={last_probe_preview}; last_full_preview={last_full_preview}; last_completion_preview={last_completion_preview}; parity_probe_preview={parity_probe_preview}; parity_full_preview={parity_full_preview}; parity_o30_preview={parity_o30_preview}; parity_k4_preview={parity_k4_preview}; failures={failures}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_edit_conversation_followup_failed")
}

pub(crate) fn resolved_gemini_canvas_browser_runtime_state_object_key(
    payload: &ProviderAccountPayload,
    runtime: &gemini_canvas::GeminiCanvasRuntime,
) -> String {
    gemini_canvas::browser_runtime_state_object_key(payload)
        .unwrap_or_else(|| runtime.runtime_state_object_key.clone())
}

pub(crate) fn gemini_canvas_image_edit_source_extension(mime_type: &str) -> &'static str {
    match mime_type.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ => "png",
    }
}

pub(crate) fn gemini_canvas_image_edit_browser_encoder_script_path() -> Option<PathBuf> {
    let repo_relative = PathBuf::from("gateway")
        .join("scripts")
        .join("gemini-canvas-image-edit-encode.mjs");
    if repo_relative.exists() {
        return Some(repo_relative);
    }
    let gateway_relative = PathBuf::from("scripts").join("gemini-canvas-image-edit-encode.mjs");
    if gateway_relative.exists() {
        return Some(gateway_relative);
    }
    None
}

pub(crate) fn build_gemini_canvas_image_edit_browser_reencode_meta(
    script_path: &std::path::Path,
    source_path: &std::path::Path,
    output_path: &std::path::Path,
    stdout: &str,
    stderr: &str,
    used: bool,
    exit_code: Option<i32>,
) -> Value {
    let mut value = json!({
        "scriptPath": script_path.to_string_lossy(),
        "sourcePath": source_path.to_string_lossy(),
        "outputPath": output_path.to_string_lossy(),
        "stdout": stdout,
        "stderr": stderr,
        "used": used,
    });
    if let Some(code) = exit_code {
        value["exitCode"] = json!(code);
    }
    value
}

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
        compact_response_preview(body_text, 240)
    )
}

pub(crate) fn build_gemini_canvas_image_edit_upload_response_meta(
    final_url: &str,
    content_type: Option<&str>,
    upload_url_present: Option<bool>,
    body_text: &str,
) -> String {
    match upload_url_present {
        Some(upload_url_present) => format!(
            "final_url={final_url}, content_type={}, upload_url_present={}, body_preview={}",
            content_type.unwrap_or("<none>"),
            upload_url_present,
            compact_response_preview(body_text, 240)
        ),
        None => format!(
            "final_url={final_url}, content_type={}, body_preview={}",
            content_type.unwrap_or("<none>"),
            compact_response_preview(body_text, 240)
        ),
    }
}

pub(crate) fn build_gemini_canvas_image_edit_upload_start_request_contract(
    upload_base_url: &str,
    file_name: &str,
    mime_type: &str,
    byte_length: usize,
    push_id: &str,
    client_pctx: &str,
    referer: &str,
) -> String {
    format!(
        "upload_base_url={upload_base_url}, file_name={file_name}, mime_type={mime_type}, bytes={byte_length}, push_id={}, client_pctx={}, referer={referer}",
        truncate_response_preview(push_id, 24),
        truncate_response_preview(client_pctx, 24),
    )
}

pub(crate) fn build_gemini_canvas_image_edit_upload_finalize_request_contract(
    upload_url: &str,
    file_name: &str,
    mime_type: &str,
    byte_length: usize,
) -> String {
    format!(
        "upload_url={upload_url}, file_name={file_name}, mime_type={mime_type}, bytes={byte_length}"
    )
}

pub(crate) fn apply_gemini_canvas_image_edit_upload_base_headers(
    headers: &mut HeaderMap,
    payload: &ProviderAccountPayload,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    origin: &str,
    referer: &str,
    push_id: &str,
    client_pctx: &str,
) {
    apply_gemini_canvas_browserish_text_headers(headers);
    apply_gemini_canvas_cookie_header(headers, session);
    insert_header_map_value(
        headers,
        "accept-language",
        &gemini_canvas::locale_from_payload(payload),
    );
    insert_header_map_value(headers, "origin", origin);
    insert_header_map_value(headers, "referer", referer);
    insert_header_map_value(headers, "sec-fetch-site", "same-site");
    insert_header_map_value(headers, "push-id", push_id);
    insert_header_map_value(headers, "x-client-pctx", client_pctx);
    insert_header_map_value(headers, "x-tenant-id", "bard-storage");
}

pub(crate) fn apply_gemini_canvas_image_edit_upload_start_headers(
    headers: &mut HeaderMap,
    byte_length: usize,
) {
    insert_header_map_value(headers, "x-goog-upload-protocol", "resumable");
    insert_header_map_value(headers, "x-goog-upload-command", "start");
    insert_header_map_value(
        headers,
        "x-goog-upload-header-content-length",
        &byte_length.to_string(),
    );
    insert_header_map_value(
        headers,
        "content-type",
        "application/x-www-form-urlencoded;charset=UTF-8",
    );
}

pub(crate) fn apply_gemini_canvas_image_edit_upload_finalize_headers(headers: &mut HeaderMap) {
    insert_header_map_value(headers, "x-goog-upload-command", "upload, finalize");
    insert_header_map_value(headers, "x-goog-upload-offset", "0");
    insert_header_map_value(
        headers,
        "content-type",
        "application/x-www-form-urlencoded;charset=utf-8",
    );
}

pub(crate) fn append_gemini_canvas_image_edit_upload_contracts(
    error: GatewayError,
    request_contract: &str,
    response_meta: &str,
) -> GatewayError {
    append_gateway_error_summary(
        append_gateway_error_summary(error, "upload_request_contract", Some(request_contract)),
        "upload_response_meta",
        Some(response_meta),
    )
}

pub(crate) fn extract_gemini_canvas_image_edit_upload_url(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-goog-upload-url")
        .or_else(|| headers.get("x-goog-upload-control-url"))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) async fn upload_gemini_canvas_image_edit_inputs_with_http(
    http: &Client,
    payload: &ProviderAccountPayload,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    uploads: &[gemini_canvas::GeminiCanvasImageEditUpload],
    timeout: Duration,
) -> Result<Vec<gemini_canvas::GeminiCanvasUploadedFileRef>, GatewayError> {
    if uploads.is_empty() {
        return Ok(Vec::new());
    }

    let provider = "gemini_canvas_compatible";
    let origin = gemini_canvas_http_origin(payload);
    let referer = format!("{}/", payload.base_url.trim_end_matches('/'));
    let push_id = bootstrap
        .push_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(gemini_canvas_image_edit_missing_push_id_error)?;
    let client_pctx = bootstrap
        .client_pctx
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(gemini_canvas_image_edit_missing_client_pctx_error)?;
    let upload_base_url = gemini_canvas_image_edit_upload_base_url(payload);
    let request_timeout = timeout.max(Duration::from_secs(120));
    let mut uploaded_refs = Vec::with_capacity(uploads.len());

    for upload in uploads {
        let mut effective_upload_bytes = upload.bytes.clone();
        let mut browser_reencode_meta = None;
        if let Some(script_path) = gemini_canvas_image_edit_browser_encoder_script_path() {
            let source_extension =
                gemini_canvas_image_edit_source_extension(&upload.source_mime_type);
            let source_sha256 = hex::encode(Sha256::digest(&upload.source_bytes));
            let input_path = gemini_canvas_image_edit_debug_output_path(&format!(
                "gemini-canvas-image-edit-browser-source-{}.{}",
                &source_sha256[..12],
                source_extension
            ));
            let output_path = gemini_canvas_image_edit_debug_output_path(&format!(
                "gemini-canvas-image-edit-browser-encoded-{}.jpg",
                &source_sha256[..12]
            ));
            if let Some(parent) = input_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&input_path, &upload.source_bytes);
            let mut command = Command::new("node");
            command
                .arg(&script_path)
                .arg(&input_path)
                .arg(&output_path)
                .arg("127467")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Ok(Ok(output)) =
                tokio::time::timeout(Duration::from_secs(45), command.output()).await
            {
                let stdout_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr_text = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if output.status.success() && output_path.exists() {
                    if let Ok(browser_bytes) = std::fs::read(&output_path) {
                        if !browser_bytes.is_empty() {
                            effective_upload_bytes = browser_bytes;
                            browser_reencode_meta =
                                Some(build_gemini_canvas_image_edit_browser_reencode_meta(
                                    &script_path,
                                    &input_path,
                                    &output_path,
                                    &stdout_text,
                                    &stderr_text,
                                    true,
                                    None,
                                ));
                        }
                    }
                }
                if browser_reencode_meta.is_none() {
                    browser_reencode_meta =
                        Some(build_gemini_canvas_image_edit_browser_reencode_meta(
                            &script_path,
                            &input_path,
                            &output_path,
                            &stdout_text,
                            &stderr_text,
                            false,
                            output.status.code(),
                        ));
                }
            }
        }

        let upload_dimensions =
            image::load_from_memory(&effective_upload_bytes)
                .ok()
                .map(|image| {
                    let (width, height) = image.dimensions();
                    (width, height)
                });
        let mut upload_hasher = Sha256::new();
        upload_hasher.update(&effective_upload_bytes);
        let upload_sha256 = hex::encode(upload_hasher.finalize());
        let upload_debug_file_name = format!(
            "gemini-canvas-image-edit-upload-debug-{}.jpg",
            &upload_sha256[..12]
        );
        let upload_debug_output_path = write_gemini_canvas_image_edit_debug_bytes(
            &upload_debug_file_name,
            &effective_upload_bytes,
        )
        .map(|path| path.to_string_lossy().to_string());
        let start_body = format!("File name: {}", upload.file_name);
        let mut start_headers = HeaderMap::new();
        apply_gemini_canvas_image_edit_upload_base_headers(
            &mut start_headers,
            payload,
            session,
            &origin,
            &referer,
            push_id,
            client_pctx,
        );
        apply_gemini_canvas_image_edit_upload_start_headers(
            &mut start_headers,
            effective_upload_bytes.len(),
        );
        let start_request_contract = build_gemini_canvas_image_edit_upload_start_request_contract(
            &upload_base_url,
            &upload.file_name,
            &upload.mime_type,
            effective_upload_bytes.len(),
            push_id,
            client_pctx,
            &referer,
        );
        let start_response = http
            .request(Method::POST, &upload_base_url)
            .headers(start_headers)
            .timeout(request_timeout)
            .body(start_body)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let start_status = start_response.status().as_u16();
        let start_content_type = start_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let upload_url = extract_gemini_canvas_image_edit_upload_url(start_response.headers());
        let start_final_url = start_response.url().to_string();
        let start_body_text = start_response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let start_response_meta = build_gemini_canvas_image_edit_upload_response_meta(
            &start_final_url,
            start_content_type.as_deref(),
            Some(upload_url.is_some()),
            &start_body_text,
        );
        append_gemini_canvas_image_edit_debug_json(
            "gemini-canvas-image-edit-upload-debug-start.json",
            &build_gemini_canvas_image_edit_upload_debug_snapshot(
                &start_request_contract,
                &start_response_meta,
                &start_body_text,
                upload_url.as_deref(),
                &upload.file_name,
                &upload.mime_type,
                effective_upload_bytes.len(),
                &upload_sha256,
                upload_dimensions,
                upload_debug_output_path.as_deref(),
                browser_reencode_meta.clone(),
                None,
            ),
        );
        if !(200..300).contains(&start_status)
            || gemini_web::response_indicates_browser_challenge(
                start_status,
                start_content_type.as_deref(),
                &start_body_text,
            )
            || gemini_web::response_indicates_session_invalid(
                start_status,
                start_content_type.as_deref(),
                &start_body_text,
            )
        {
            return Err(append_gemini_canvas_image_edit_upload_contracts(
                classify_gemini_canvas_pure_http_error(
                    start_status,
                    start_content_type.as_deref(),
                    &start_body_text,
                ),
                &start_request_contract,
                &start_response_meta,
            ));
        }
        let upload_url = upload_url.ok_or_else(|| {
            append_gemini_canvas_image_edit_upload_contracts(
                gemini_canvas_image_edit_missing_upload_url_error(),
                &start_request_contract,
                &start_response_meta,
            )
        })?;

        let mut finalize_headers = HeaderMap::new();
        apply_gemini_canvas_image_edit_upload_base_headers(
            &mut finalize_headers,
            payload,
            session,
            &origin,
            &referer,
            push_id,
            client_pctx,
        );
        apply_gemini_canvas_image_edit_upload_finalize_headers(&mut finalize_headers);
        let finalize_request_contract =
            build_gemini_canvas_image_edit_upload_finalize_request_contract(
                &upload_url,
                &upload.file_name,
                &upload.mime_type,
                effective_upload_bytes.len(),
            );
        let finalize_response = http
            .request(Method::POST, &upload_url)
            .headers(finalize_headers)
            .timeout(request_timeout)
            .body(effective_upload_bytes.clone())
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let finalize_status = finalize_response.status().as_u16();
        let finalize_content_type = finalize_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let finalize_final_url = finalize_response.url().to_string();
        let finalize_body_text = finalize_response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let finalize_response_meta = build_gemini_canvas_image_edit_upload_response_meta(
            &finalize_final_url,
            finalize_content_type.as_deref(),
            None,
            &finalize_body_text,
        );
        let overridden_resource_path =
            std::env::var("GEMINI_CANVAS_IMAGE_EDIT_RESOURCE_PATH_OVERRIDE")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        append_gemini_canvas_image_edit_debug_json(
            "gemini-canvas-image-edit-upload-debug-finalize.json",
            &build_gemini_canvas_image_edit_upload_debug_snapshot(
                &finalize_request_contract,
                &finalize_response_meta,
                &finalize_body_text,
                Some(upload_url.as_str()),
                &upload.file_name,
                &upload.mime_type,
                effective_upload_bytes.len(),
                &upload_sha256,
                upload_dimensions,
                upload_debug_output_path.as_deref(),
                browser_reencode_meta.clone(),
                overridden_resource_path.as_deref(),
            ),
        );
        if !(200..300).contains(&finalize_status)
            || gemini_web::response_indicates_browser_challenge(
                finalize_status,
                finalize_content_type.as_deref(),
                &finalize_body_text,
            )
            || gemini_web::response_indicates_session_invalid(
                finalize_status,
                finalize_content_type.as_deref(),
                &finalize_body_text,
            )
        {
            return Err(append_gemini_canvas_image_edit_upload_contracts(
                classify_gemini_canvas_pure_http_error(
                    finalize_status,
                    finalize_content_type.as_deref(),
                    &finalize_body_text,
                ),
                &finalize_request_contract,
                &finalize_response_meta,
            ));
        }
        let resource_path = extract_gemini_canvas_image_edit_resource_path(
            overridden_resource_path.as_deref(),
            &finalize_body_text,
        )
        .ok_or_else(|| {
            append_gemini_canvas_image_edit_upload_contracts(
                gemini_canvas_image_edit_missing_resource_path_error(),
                &finalize_request_contract,
                &finalize_response_meta,
            )
        })?;
        uploaded_refs.push(gemini_canvas::GeminiCanvasUploadedFileRef {
            resource_path,
            mime_type: upload.mime_type.clone(),
            file_name: upload.file_name.clone(),
        });
    }

    Ok(uploaded_refs)
}

pub(crate) async fn open_gemini_canvas_image_edit_signaler_channel_with_http(
    http: &Client,
    plain_http: &Client,
    payload: &ProviderAccountPayload,
    runtime: &gemini_canvas::GeminiCanvasRuntime,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    locale_override: Option<&str>,
    timeout: Duration,
) -> Result<GeminiCanvasSignalerChannel, GatewayError> {
    let base_url = payload.base_url.trim_end_matches('/');
    let app_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
    let storage_state = gateway_object_storage()?
        .read_json(&runtime.runtime_state_object_key)
        .await?;
    let storage_locale = gemini_canvas::harvest_image_edit_template_locale(&storage_state);
    let payload_locale = gemini_canvas::locale_from_payload(payload);
    let effective_locale = locale_override
        .map(str::to_string)
        .or(storage_locale.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(payload_locale);
    let page_body = fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
        http,
        plain_http,
        payload,
        session,
        &app_url,
        timeout,
        Some(effective_locale.as_str()),
    )
    .await?;
    let bootstrap_material = resolve_gemini_canvas_image_edit_signaler_bootstrap_material(
        payload,
        &storage_state,
        &page_body,
    )?;
    let account_id = bootstrap_material.account_id;
    let api_key_candidates = bootstrap_material.api_key_candidates;

    let mut failures = Vec::new();
    for api_key in api_key_candidates {
        let choose_server_url = format!(
            "https://signaler-pa.clients6.google.com/punctual/v1/chooseServer?key={api_key}"
        );
        let choose_server_body =
            gemini_canvas::build_image_edit_signaler_choose_server_body(&account_id);
        let choose_server_response =
            match send_gemini_canvas_signaler_request_refreshing_session_with_http(
                http,
                payload,
                session,
                Method::POST,
                &choose_server_url,
                Some("application/json+protobuf"),
                None,
                Some(choose_server_body),
                timeout
                    .min(Duration::from_secs(30))
                    .max(Duration::from_secs(15)),
                locale_override.or(storage_locale.as_deref()),
            )
            .await
            {
                Ok(body) => body,
                Err(error) => {
                    failures.push(format!(
                        "key={} chooseServer={}",
                        redact_gemini_canvas_api_key_for_logs(&api_key),
                        summarize_gateway_error(&error)
                    ));
                    continue;
                }
            };
        let gsession_id =
            match gemini_canvas::parse_signaler_choose_server_response(&choose_server_response) {
                Ok(value) => value,
                Err(error) => {
                    failures.push(format!(
                        "key={} chooseServerParse={}",
                        redact_gemini_canvas_api_key_for_logs(&api_key),
                        summarize_gateway_error(&error)
                    ));
                    continue;
                }
            };

        let open_channel_url = format!(
            "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?VER=8&gsessionid={}&key={}&RID={}&CVER=22&zx={}&t=1",
            gsession_id,
            api_key,
            7000 + (current_unix_timestamp_i64().unsigned_abs() % 1000),
            gemini_canvas_signaler_zx_token()
        );
        let open_channel_body =
            gemini_canvas::build_image_edit_signaler_open_channel_body(&account_id);
        let open_channel_response =
            match send_gemini_canvas_signaler_request_refreshing_session_with_http(
                http,
                payload,
                session,
                Method::POST,
                &open_channel_url,
                Some("application/x-www-form-urlencoded"),
                Some("application/json+protobuf"),
                Some(open_channel_body),
                timeout
                    .min(Duration::from_secs(30))
                    .max(Duration::from_secs(15)),
                locale_override.or(storage_locale.as_deref()),
            )
            .await
            {
                Ok(body) => body,
                Err(error) => {
                    failures.push(format!(
                        "key={} openChannel={}",
                        redact_gemini_canvas_api_key_for_logs(&api_key),
                        summarize_gateway_error(&error)
                    ));
                    continue;
                }
            };
        let sid = match gemini_canvas::parse_signaler_open_channel_sid(&open_channel_response) {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!(
                    "key={} openChannelParse={}",
                    redact_gemini_canvas_api_key_for_logs(&api_key),
                    summarize_gateway_error(&error)
                ));
                continue;
            }
        };

        return Ok(GeminiCanvasSignalerChannel {
            api_key,
            gsession_id,
            sid,
            next_aid: 0,
        });
    }

    Err(gemini_canvas_image_edit_signaler_all_keys_failed_error(
        failures.join(" | ").as_str(),
    ))
}

pub(crate) async fn prewarm_gemini_canvas_image_edit_signaler_with_http(
    http: &Client,
    plain_http: &Client,
    payload: &ProviderAccountPayload,
    runtime: &gemini_canvas::GeminiCanvasRuntime,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    locale_override: Option<&str>,
    timeout: Duration,
) -> Result<GeminiCanvasSignalerChannel, GatewayError> {
    let mut channel = open_gemini_canvas_image_edit_signaler_channel_with_http(
        http,
        plain_http,
        payload,
        runtime,
        session,
        locale_override,
        timeout,
    )
    .await?;
    let poll_url = build_gemini_canvas_image_edit_signaler_poll_url(
        &channel,
        &gemini_canvas_signaler_zx_token(),
    );
    let body = send_gemini_canvas_signaler_poll_request_refreshing_session_with_http(
        http,
        payload,
        session,
        &poll_url,
        timeout
            .min(Duration::from_secs(60))
            .max(Duration::from_secs(20)),
        channel.next_aid,
        locale_override,
    )
    .await?;
    let _ = refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http(
        http,
        payload,
        session,
        &channel,
        &body,
        timeout
            .min(Duration::from_secs(20))
            .max(Duration::from_secs(10)),
        locale_override,
    )
    .await;
    update_gemini_canvas_image_edit_signaler_next_aid_from_body(&mut channel, &body);
    Ok(channel)
}

pub(crate) async fn prepare_gemini_canvas_image_edit_signaler_poll_state_with_http(
    http: &Client,
    plain_http: &Client,
    payload: &ProviderAccountPayload,
    runtime: &gemini_canvas::GeminiCanvasRuntime,
    storage_state: &Value,
    base_url: &str,
    app_url: &str,
    auth_user: &str,
    locale_hint: Option<&str>,
    edit_context: Option<&GeminiCanvasImageEditFollowupContext>,
    timeout: Duration,
) -> Result<
    (
        gemini_canvas::GeminiCanvasPureHttpSession,
        GeminiCanvasSignalerChannel,
    ),
    GatewayError,
> {
    if let Some((session, channel)) = edit_context.and_then(|context| {
        context
            .signaler_session
            .clone()
            .zip(context.signaler_channel.clone())
    }) {
        return Ok((session, channel));
    }

    let mut session = gemini_canvas::storage_state_to_pure_http_session(
        storage_state,
        app_url,
        base_url,
        auth_user,
    )?;
    let channel = open_gemini_canvas_image_edit_signaler_channel_with_http(
        http,
        plain_http,
        payload,
        runtime,
        &mut session,
        locale_hint,
        timeout,
    )
    .await?;
    Ok((session, channel))
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_locator_from_body(
    context: &mut GeminiCanvasImageEditFollowupContext,
    body: &str,
) {
    if context.signaler_response_id.is_some() {
        return;
    }

    if let Ok(locator) = gemini_canvas::extract_stream_generate_locator(body) {
        context.signaler_response_id = Some(locator.response_id);
        context.signaler_conversation_id = Some(locator.conversation_id);
    } else if let Some(response_id) = gemini_canvas::extract_stream_generate_response_id(body).ok()
    {
        context.signaler_response_id = Some(response_id);
    }
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_poll_body_preview(
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    body: &str,
) -> String {
    if let Some(context) = edit_context {
        record_gemini_canvas_image_edit_signaler_locator_from_body(context, body);
    }
    compact_response_preview(body, 240)
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_followup_state_from_body(
    context: &mut GeminiCanvasImageEditFollowupContext,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
    body: &str,
) {
    record_gemini_canvas_image_edit_signaler_followup_state(context, session, channel, locale_hint);
    record_gemini_canvas_image_edit_signaler_locator_from_body(context, body);
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_followup_state(
    context: &mut GeminiCanvasImageEditFollowupContext,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
) {
    context.signaler_session = Some(session.clone());
    context.signaler_channel = Some(channel.clone());
    if context.locale_hint.is_none() {
        context.locale_hint = locale_hint.map(str::to_string);
    }
}

pub(crate) fn extract_gemini_canvas_image_edit_signaler_assets_from_body(
    body: &str,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
) -> Result<Vec<gemini_canvas::GeminiCanvasMediaAsset>, GatewayError> {
    let assets = gemini_canvas::extract_page_blob_media_assets(
        body,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
    )?;
    if let Some(context) = edit_context {
        record_gemini_canvas_image_edit_signaler_followup_state_from_body(
            context,
            session,
            channel,
            locale_hint,
            body,
        );
    }
    Ok(assets)
}

pub(crate) fn try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body(
    body: String,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
) -> Option<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String)> {
    let assets = extract_gemini_canvas_image_edit_signaler_assets_from_body(
        &body,
        session,
        channel,
        locale_hint,
        edit_context,
    )
    .ok()?;
    Some((assets, body))
}

pub(crate) fn finish_gemini_canvas_image_edit_signaler_missing_asset(
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
    failures: &[String],
    last_body_preview: Option<String>,
) -> GatewayError {
    if let Some(context) = edit_context {
        record_gemini_canvas_image_edit_signaler_followup_state(
            context,
            session,
            channel,
            locale_hint,
        );
    }

    let failure_summary = failures.join(" | ");
    let last_body_preview_text = last_body_preview.unwrap_or_else(|| "<none>".to_string());
    gemini_canvas_image_edit_signaler_missing_asset_error(
        channel.next_aid,
        &failure_summary,
        &last_body_preview_text,
    )
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_app_path(
    base_url: &str,
    app_path: &str,
    seen_app_paths: &mut HashSet<String>,
    first_app_path_seen_at: &mut Option<Instant>,
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
) -> String {
    let page_url = format!("{base_url}{app_path}");
    append_gemini_canvas_image_edit_trace("signaler.app-path", page_url.as_str());
    seen_app_paths.insert(page_url.clone());
    if first_app_path_seen_at.is_none() {
        *first_app_path_seen_at = Some(Instant::now());
    }
    if let Some(context) = edit_context {
        if !context
            .signaler_app_urls
            .iter()
            .any(|value| value == &page_url)
        {
            context.signaler_app_urls.push(page_url.clone());
        }
        context.signaler_app_url = Some(page_url.clone());
    }
    page_url
}

pub(crate) fn build_gemini_canvas_image_edit_signaler_poll_url(
    channel: &GeminiCanvasSignalerChannel,
    zx_token: &str,
) -> String {
    format!(
        "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?VER=8&gsessionid={}&key={}&RID=rpc&SID={}&AID={}&CI=0&TYPE=xmlhttp&zx={}&t=1",
        channel.gsession_id,
        channel.api_key,
        channel.sid,
        channel.next_aid,
        zx_token
    )
}

pub(crate) fn update_gemini_canvas_image_edit_signaler_next_aid_from_body(
    channel: &mut GeminiCanvasSignalerChannel,
    body: &str,
) -> Option<u64> {
    let max_aid = gemini_canvas::extract_signaler_long_poll_max_aid(body)?;
    channel.next_aid = max_aid;
    Some(max_aid)
}

pub(crate) fn gemini_canvas_image_edit_signaler_page_failure_entry(
    page_url: &str,
    label: &str,
    detail: impl AsRef<str>,
) -> String {
    format!("page_url={} {}={}", page_url, label, detail.as_ref())
}

pub(crate) fn gemini_canvas_image_edit_signaler_poll_error_entry(
    next_aid: u64,
    detail: impl AsRef<str>,
) -> String {
    format!("poll_aid={} error={}", next_aid, detail.as_ref())
}

pub(crate) fn gemini_canvas_image_edit_signaler_refresh_error_entry(
    next_aid: u64,
    detail: impl AsRef<str>,
) -> String {
    format!("refresh_creds aid={} error={}", next_aid, detail.as_ref())
}

pub(crate) async fn send_gemini_canvas_signaler_request_refreshing_session_with_http(
    http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    method: Method,
    url: &str,
    content_type: Option<&str>,
    webchannel_content_type: Option<&str>,
    body: Option<String>,
    timeout: Duration,
    locale_override: Option<&str>,
) -> Result<String, GatewayError> {
    let provider = "gemini_canvas_compatible";
    let origin = payload.base_url.trim_end_matches('/');
    let referer = format!("{}/", payload.base_url.trim_end_matches('/'));
    let accept_language = locale_override
        .map(str::to_string)
        .unwrap_or_else(|| gemini_canvas::locale_from_payload(payload));
    let mut headers = HeaderMap::new();
    apply_gemini_canvas_signaler_headers(&mut headers);
    apply_gemini_canvas_cookie_header(&mut headers, session);
    insert_header_map_value(&mut headers, "accept-language", &accept_language);
    insert_header_map_value(&mut headers, "origin", origin);
    insert_header_map_value(&mut headers, "referer", &referer);
    insert_header_map_value(
        &mut headers,
        "x-goog-authuser",
        &gemini_canvas::direct_http_auth_user(payload),
    );
    if let Some(content_type) = content_type {
        insert_header_map_value(&mut headers, "content-type", content_type);
    }
    if let Some(webchannel_content_type) = webchannel_content_type {
        insert_header_map_value(
            &mut headers,
            "x-webchannel-content-type",
            webchannel_content_type,
        );
    }
    let mut request = http.request(method, url).headers(headers).timeout(timeout);
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    apply_gemini_canvas_response_cookies(response.headers(), session);
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    if !(200..300).contains(&status)
        || gemini_web::response_indicates_browser_challenge(
            status,
            content_type.as_deref(),
            &body_text,
        )
        || gemini_web::response_indicates_session_invalid(
            status,
            content_type.as_deref(),
            &body_text,
        )
    {
        return Err(classify_gemini_canvas_pure_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
    }
    Ok(body_text)
}

pub(crate) async fn send_gemini_canvas_signaler_poll_request_refreshing_session_with_http(
    http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    url: &str,
    timeout: Duration,
    aid_hint: u64,
    locale_override: Option<&str>,
) -> Result<String, GatewayError> {
    let provider = "gemini_canvas_compatible";
    let origin = payload.base_url.trim_end_matches('/');
    let referer = format!("{}/", payload.base_url.trim_end_matches('/'));
    let accept_language = locale_override
        .map(str::to_string)
        .unwrap_or_else(|| gemini_canvas::locale_from_payload(payload));
    let mut headers = HeaderMap::new();
    apply_gemini_canvas_signaler_headers(&mut headers);
    apply_gemini_canvas_cookie_header(&mut headers, session);
    insert_header_map_value(&mut headers, "accept-language", &accept_language);
    insert_header_map_value(&mut headers, "origin", origin);
    insert_header_map_value(&mut headers, "referer", &referer);
    insert_header_map_value(
        &mut headers,
        "x-goog-authuser",
        &gemini_canvas::direct_http_auth_user(payload),
    );
    let response = http
        .request(Method::GET, url)
        .headers(headers)
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    apply_gemini_canvas_response_cookies(response.headers(), session);
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = collect_gemini_canvas_signaler_body(response, provider, aid_hint).await?;
    if !(200..300).contains(&status)
        || gemini_web::response_indicates_browser_challenge(
            status,
            content_type.as_deref(),
            &body_text,
        )
        || gemini_web::response_indicates_session_invalid(
            status,
            content_type.as_deref(),
            &body_text,
        )
    {
        return Err(classify_gemini_canvas_pure_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
    }
    Ok(body_text)
}

pub(crate) async fn refresh_gemini_canvas_image_edit_signaler_creds_with_http(
    http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    refresh_token: &str,
    timeout: Duration,
    locale_override: Option<&str>,
) -> Result<String, GatewayError> {
    let url = format!(
        "https://signaler-pa.clients6.google.com/punctual/v1/refreshCreds?key={}&gsessionid={}",
        channel.api_key, channel.gsession_id
    );
    let body = gemini_canvas::build_image_edit_signaler_refresh_creds_body(refresh_token);
    send_gemini_canvas_signaler_request_refreshing_session_with_http(
        http,
        payload,
        session,
        Method::POST,
        &url,
        Some("application/json+protobuf"),
        None,
        Some(body),
        timeout,
        locale_override,
    )
    .await
}

pub(crate) async fn refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http(
    http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    body: &str,
    timeout: Duration,
    locale_override: Option<&str>,
) -> Result<Option<String>, GatewayError> {
    let Some(refresh_token) = gemini_canvas::extract_signaler_long_poll_refresh_token(body) else {
        return Ok(None);
    };
    refresh_gemini_canvas_image_edit_signaler_creds_with_http(
        http,
        payload,
        session,
        channel,
        &refresh_token,
        timeout,
        locale_override,
    )
    .await?;
    Ok(Some(refresh_token))
}

async fn collect_gemini_canvas_signaler_body(
    response: rquest::Response,
    provider: &str,
    aid_hint: u64,
) -> Result<String, GatewayError> {
    let mut stream = response.bytes_stream();
    let mut body_text = String::new();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                body_text.push_str(String::from_utf8_lossy(&chunk).as_ref());
                if gemini_canvas::extract_page_blob_media_assets(
                    &body_text,
                    gemini_canvas::GeminiCanvasMediaOperation::Image,
                )
                .is_ok()
                {
                    return Ok(body_text);
                }
                if !gemini_canvas::extract_signaler_app_paths(&body_text).is_empty() {
                    return Ok(body_text);
                }
                if aid_hint == 0
                    && gemini_canvas::extract_signaler_long_poll_refresh_token(&body_text).is_some()
                    && gemini_canvas::extract_signaler_long_poll_max_aid(&body_text).is_some()
                {
                    return Ok(body_text);
                }
            }
            Err(error) => {
                if !body_text.is_empty() {
                    return Ok(body_text);
                }
                return Err(classify_network_error(&error, Some(provider)));
            }
        }
    }

    Ok(body_text)
}

pub(crate) fn extract_gemini_canvas_image_edit_resource_path(
    resource_path_override: Option<&str>,
    finalize_body_text: &str,
) -> Option<String> {
    resource_path_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let trimmed = finalize_body_text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .filter(|value| value.starts_with("/contrib_service/"))
        .map(ToString::to_string)
}

fn gemini_canvas_image_edit_trace_enabled() -> bool {
    std::env::var_os("GEMINI_CANVAS_IMAGE_EDIT_TRACE").is_some()
}

fn gemini_canvas_local_runtime_root() -> PathBuf {
    let direct = PathBuf::from(".runtime");
    if direct.exists() {
        return direct;
    }

    let parent = PathBuf::from("..").join(".runtime");
    if parent.exists() {
        return parent;
    }

    direct
}

pub(crate) fn gemini_canvas_image_edit_debug_output_path(name: &str) -> PathBuf {
    gemini_canvas_local_runtime_root().join(name)
}

fn sanitize_gemini_canvas_debug_header_value(name: &str, value: &str) -> String {
    if name.eq_ignore_ascii_case("cookie") || name.eq_ignore_ascii_case("authorization") {
        return "<redacted>".to_string();
    }
    value.to_string()
}

pub(crate) fn gemini_canvas_debug_headers_snapshot_from_pairs(pairs: &[(String, String)]) -> Value {
    let mut raw = serde_json::Map::new();
    let mut parsed = serde_json::Map::new();
    for (name, value) in pairs {
        raw.insert(
            name.clone(),
            Value::String(sanitize_gemini_canvas_debug_header_value(name, value)),
        );
        if name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb")
            || name.eq_ignore_ascii_case("x-goog-ext-525005358-jspb")
            || name.eq_ignore_ascii_case("x-goog-ext-73010989-jspb")
            || name.eq_ignore_ascii_case("x-goog-ext-73010990-jspb")
        {
            parsed.insert(
                name.clone(),
                serde_json::from_str::<Value>(value)
                    .unwrap_or_else(|_| Value::String(value.clone())),
            );
        }
    }
    json!({
        "raw": raw,
        "parsed": parsed,
    })
}

pub(crate) fn gemini_canvas_debug_headers_snapshot_from_hash_map(
    headers: &HashMap<String, String>,
) -> Value {
    let mut pairs = headers
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect::<Vec<_>>();
    pairs.sort_by(|left, right| left.0.cmp(&right.0));
    gemini_canvas_debug_headers_snapshot_from_pairs(&pairs)
}

pub(crate) fn gemini_canvas_debug_headers_snapshot_from_header_map(headers: &HeaderMap) -> Value {
    let pairs = headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|entry| (name.as_str().to_string(), entry.to_string()))
        })
        .collect::<Vec<_>>();
    gemini_canvas_debug_headers_snapshot_from_pairs(&pairs)
}

fn gemini_canvas_debug_query_snapshot(query: &[(String, String)]) -> Value {
    let items = query
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect::<Vec<_>>();
    let mut object = serde_json::Map::new();
    for (name, value) in query {
        object.insert(name.clone(), Value::String(value.clone()));
    }
    json!({
        "items": items,
        "object": object,
    })
}

pub(crate) fn gemini_canvas_debug_form_snapshot(form: &[(String, String)]) -> Value {
    let items = form
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect::<Vec<_>>();
    let mut object = serde_json::Map::new();
    let mut parsed = serde_json::Map::new();
    for (name, value) in form {
        object.insert(name.clone(), Value::String(value.clone()));
        if name == "f.req" {
            if let Ok(outer) = serde_json::from_str::<Value>(value) {
                parsed.insert("f.req.outer".to_string(), outer.clone());
                if let Some(inner_payload) = outer.get(1).and_then(Value::as_str) {
                    let inner = serde_json::from_str::<Value>(inner_payload)
                        .unwrap_or_else(|_| Value::String(inner_payload.to_string()));
                    parsed.insert("f.req.inner".to_string(), inner);
                }
            }
        }
    }
    json!({
        "items": items,
        "object": object,
        "parsed": parsed,
    })
}

pub(crate) fn append_gemini_canvas_image_edit_debug_json(file_name: &str, snapshot: &Value) {
    if !gemini_canvas_image_edit_trace_enabled() {
        return;
    }
    let path = gemini_canvas_image_edit_debug_output_path(file_name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(snapshot) {
        let _ = std::fs::write(path, bytes);
    }
}

pub(crate) fn write_gemini_canvas_image_edit_debug_bytes(
    file_name: &str,
    bytes: &[u8],
) -> Option<PathBuf> {
    if !gemini_canvas_image_edit_trace_enabled() {
        return None;
    }
    let path = gemini_canvas_image_edit_debug_output_path(file_name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, bytes).ok()?;
    Some(path)
}

pub(crate) fn append_gemini_canvas_image_edit_request_debug_snapshot(
    file_name: &str,
    stage: &str,
    url: &str,
    query: &[(String, String)],
    form: &[(String, String)],
    headers: Value,
    extra: Value,
) {
    let captured_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    append_gemini_canvas_image_edit_debug_json(
        file_name,
        &json!({
            "stage": stage,
            "capturedAtMs": captured_at_ms,
            "url": url,
            "query": gemini_canvas_debug_query_snapshot(query),
            "form": gemini_canvas_debug_form_snapshot(form),
            "headers": headers,
            "extra": extra,
        }),
    );
}

pub(crate) fn append_gemini_canvas_image_edit_stream_response_debug_snapshot(
    file_stem: &str,
    stage: &str,
    final_url: &str,
    status: u16,
    location: Option<&str>,
    content_type: Option<&str>,
    body_text: &str,
    extra: Value,
) {
    append_gemini_canvas_image_edit_debug_json(
        &format!("{file_stem}.json"),
        &json!({
            "stage": stage,
            "capturedAtMs": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or_default(),
            "finalUrl": final_url,
            "status": status,
            "location": location,
            "contentType": content_type,
            "bodyLength": body_text.len(),
            "bodyPreview": compact_response_preview(body_text, 320),
            "extra": extra,
        }),
    );
    let _ = write_gemini_canvas_image_edit_debug_bytes(
        &format!("{file_stem}.txt"),
        body_text.as_bytes(),
    );
}

pub(crate) fn gemini_canvas_runtime_mirror_json_path(key: &str) -> Option<PathBuf> {
    let normalized = key
        .trim()
        .trim_start_matches("./")
        .trim_start_matches(".\\")
        .trim_start_matches('/')
        .trim_start_matches('\\')
        .replace('\\', "/");
    if normalized.is_empty() {
        return None;
    }
    let direct = gemini_canvas_local_runtime_root()
        .join("ai-gateway-objects")
        .join(&normalized);
    if direct.exists() {
        return Some(direct);
    }

    let parent = PathBuf::from("..")
        .join(".runtime")
        .join("ai-gateway-objects")
        .join(&normalized);
    if parent.exists() {
        return Some(parent);
    }

    Some(direct)
}

pub(crate) fn read_gemini_canvas_runtime_mirror_json(key: &str) -> Option<Value> {
    let path = gemini_canvas_runtime_mirror_json_path(key)?;
    let text = std::fs::read_to_string(path).ok()?;
    let trimmed = text.trim_start_matches('\u{feff}');
    serde_json::from_str::<Value>(trimmed).ok()
}

pub(crate) fn gemini_canvas_sidecar_has_any_key(value: &Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| value.get(*key).is_some())
}

pub(crate) fn append_gemini_canvas_image_edit_trace(stage: &str, detail: impl AsRef<str>) {
    if !gemini_canvas_image_edit_trace_enabled() {
        return;
    }
    let path = gemini_canvas_image_edit_debug_output_path("gemini-canvas-image-edit-trace.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let _ = writeln!(file, "{}\t{}\t{}", millis, stage, detail.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::future::Future;

    fn make_payload(
        adapter: &str,
        base_url: &str,
    ) -> crate::routing::candidate::ProviderAccountPayload {
        serde_json::from_value(json!({
            "adapter": adapter,
            "baseUrl": base_url,
            "apiKey": "sk-test"
        }))
        .expect("payload")
    }

    #[test]
    fn image_edit_missing_push_id_error_matches_contract() {
        let error = gemini_canvas_image_edit_missing_push_id_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_missing_push_id")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit upload requires bootstrap push_id from the /app page."
        );
    }

    #[test]
    fn image_edit_missing_client_pctx_error_matches_contract() {
        let error = gemini_canvas_image_edit_missing_client_pctx_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_missing_client_pctx")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit upload requires bootstrap client_pctx from the /app page."
        );
    }

    #[test]
    fn image_edit_missing_upload_url_error_matches_contract() {
        let error = gemini_canvas_image_edit_missing_upload_url_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_missing_upload_url")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit upload start response did not expose an upload URL."
        );
    }

    #[test]
    fn image_edit_missing_resource_path_error_matches_contract() {
        let error = gemini_canvas_image_edit_missing_resource_path_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_missing_resource_path")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit upload finalize response did not return a contrib_service resource path."
        );
    }

    #[test]
    fn upload_gemini_canvas_image_edit_inputs_with_http_returns_uploaded_refs_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<
                Output = Result<Vec<gemini_canvas::GeminiCanvasUploadedFileRef>, GatewayError>,
            >,
        {
        }

        let http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let bootstrap = crate::protocol::gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: None,
            session_id: None,
            language: "en-US".to_string(),
            push_id: Some("push-id-123".to_string()),
            client_pctx: Some("client-pctx-456".to_string()),
            app_page_path: Some("/app".to_string()),
        };
        let uploads = Vec::<gemini_canvas::GeminiCanvasImageEditUpload>::new();

        assert_future_output(upload_gemini_canvas_image_edit_inputs_with_http(
            &http,
            &payload,
            &session,
            &bootstrap,
            &uploads,
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn send_gemini_canvas_signaler_request_refreshing_session_with_http_returns_string_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<String, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };

        assert_future_output(
            send_gemini_canvas_signaler_request_refreshing_session_with_http(
                &http,
                &payload,
                &mut session,
                Method::POST,
                "https://signaler-pa.clients6.google.com/punctual/v1/chooseServer?key=test",
                Some("application/json+protobuf"),
                None,
                Some("body".to_string()),
                std::time::Duration::from_secs(1),
                Some("en-US"),
            ),
        );
    }

    #[test]
    fn send_gemini_canvas_signaler_poll_request_refreshing_session_with_http_returns_string_result()
    {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<String, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };

        assert_future_output(
            send_gemini_canvas_signaler_poll_request_refreshing_session_with_http(
                &http,
                &payload,
                &mut session,
                "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?VER=8&RID=rpc",
                std::time::Duration::from_secs(1),
                0,
                Some("en-US"),
            ),
        );
    }

    #[test]
    fn refresh_gemini_canvas_image_edit_signaler_creds_with_http_returns_string_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<String, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
            api_key: "api-key".to_string(),
            gsession_id: "gsession-id".to_string(),
            sid: "sid".to_string(),
            next_aid: 0,
        };

        assert_future_output(refresh_gemini_canvas_image_edit_signaler_creds_with_http(
            &http,
            &payload,
            &mut session,
            &channel,
            "refresh-token",
            std::time::Duration::from_secs(1),
            Some("en-US"),
        ));
    }

    #[test]
    fn refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http_skips_missing_token() {
        let http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
            api_key: "api-key".to_string(),
            gsession_id: "gsession-id".to_string(),
            sid: "sid".to_string(),
            next_aid: 0,
        };

        let refreshed = futures::executor::block_on(
            refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http(
                &http,
                &payload,
                &mut session,
                &channel,
                "long-poll body without refresh token",
                std::time::Duration::from_secs(1),
                Some("en-US"),
            ),
        )
        .expect("missing refresh token should be a successful no-op");

        assert_eq!(refreshed, None);
        assert_eq!(session.auth_user.as_str(), "1");
    }

    #[test]
    fn open_gemini_canvas_image_edit_signaler_channel_with_http_returns_channel_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<
                Output = Result<
                    crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel,
                    GatewayError,
                >,
            >,
        {
        }

        let http = rquest::Client::new();
        let plain_http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let runtime = gemini_canvas::GeminiCanvasRuntime {
            runtime_state_object_key: "credential-runtime/gemini-canvas/runtime.json".to_string(),
            share_id: "share".to_string(),
            api_base_url: "https://gemini.google.com".to_string(),
        };
        let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };

        assert_future_output(open_gemini_canvas_image_edit_signaler_channel_with_http(
            &http,
            &plain_http,
            &payload,
            &runtime,
            &mut session,
            Some("en-US"),
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn prewarm_gemini_canvas_image_edit_signaler_with_http_returns_channel_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<
                Output = Result<
                    crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel,
                    GatewayError,
                >,
            >,
        {
        }

        let http = rquest::Client::new();
        let plain_http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let runtime = gemini_canvas::GeminiCanvasRuntime {
            runtime_state_object_key: "credential-runtime/gemini-canvas/runtime.json".to_string(),
            share_id: "share".to_string(),
            api_base_url: "https://gemini.google.com".to_string(),
        };
        let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };

        assert_future_output(prewarm_gemini_canvas_image_edit_signaler_with_http(
            &http,
            &plain_http,
            &payload,
            &runtime,
            &mut session,
            Some("en-US"),
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn prepare_gemini_canvas_image_edit_signaler_poll_state_with_http_returns_session_and_channel_result(
    ) {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<
                Output = Result<
                    (
                        gemini_canvas::GeminiCanvasPureHttpSession,
                        crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel,
                    ),
                    GatewayError,
                >,
            >,
        {
        }

        let http = rquest::Client::new();
        let plain_http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let runtime = gemini_canvas::GeminiCanvasRuntime {
            runtime_state_object_key: "credential-runtime/gemini-canvas/runtime.json".to_string(),
            share_id: "share".to_string(),
            api_base_url: "https://gemini.google.com".to_string(),
        };
        let storage_state = json!({
            "cookies": [],
        });

        assert_future_output(prepare_gemini_canvas_image_edit_signaler_poll_state_with_http(
            &http,
            &plain_http,
            &payload,
            &runtime,
            &storage_state,
            "https://gemini.google.com",
            "https://gemini.google.com/app",
            "1",
            Some("en-US"),
            None::<&crate::upstream::gemini_canvas_followup_types::GeminiCanvasImageEditFollowupContext>,
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn record_gemini_canvas_image_edit_signaler_locator_from_body_preserves_locator_contract() {
        let frame1 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
        ])])
        .expect("frame1");
        let frame2 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
        ])])
        .expect("frame2");
        let body = format!(
            ")]}}'\n\n{}\n{}\n{}\n{}\n",
            frame1.encode_utf16().count(),
            frame1,
            frame2.encode_utf16().count(),
            frame2
        );
        let mut context = GeminiCanvasImageEditFollowupContext {
            prompt: "edit prompt".to_string(),
            request_started_at: std::time::SystemTime::UNIX_EPOCH,
            locale_hint: None,
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        };

        record_gemini_canvas_image_edit_signaler_locator_from_body(&mut context, &body);

        assert_eq!(
            context.signaler_response_id.as_deref(),
            Some("r_e0e4aa76ab2755e3")
        );
        assert_eq!(
            context.signaler_conversation_id.as_deref(),
            Some("c_1004db0ef60d9dda")
        );
    }

    #[test]
    fn record_gemini_canvas_image_edit_signaler_poll_body_preview_preserves_locator_contract() {
        let frame1 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
        ])])
        .expect("frame1");
        let frame2 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
        ])])
        .expect("frame2");
        let body = format!(
            ")]}}'\n\n{}\n{}\n{}\n{}\n",
            frame1.encode_utf16().count(),
            frame1,
            frame2.encode_utf16().count(),
            frame2
        );
        let mut context = GeminiCanvasImageEditFollowupContext {
            prompt: "edit prompt".to_string(),
            request_started_at: std::time::SystemTime::UNIX_EPOCH,
            locale_hint: None,
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        };

        let preview =
            record_gemini_canvas_image_edit_signaler_poll_body_preview(Some(&mut context), &body);

        assert_eq!(preview, compact_response_preview(&body, 240));
        assert_eq!(
            context.signaler_response_id.as_deref(),
            Some("r_e0e4aa76ab2755e3")
        );
        assert_eq!(
            context.signaler_conversation_id.as_deref(),
            Some("c_1004db0ef60d9dda")
        );
    }

    #[test]
    fn record_gemini_canvas_image_edit_signaler_followup_state_from_body_preserves_state_contract()
    {
        let frame1 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
        ])])
        .expect("frame1");
        let frame2 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
        ])])
        .expect("frame2");
        let body = format!(
            ")]}}'\n\n{}\n{}\n{}\n{}\n",
            frame1.encode_utf16().count(),
            frame1,
            frame2.encode_utf16().count(),
            frame2
        );
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
            api_key: "api-key".to_string(),
            gsession_id: "gsession-id".to_string(),
            sid: "sid".to_string(),
            next_aid: 7,
        };
        let mut context = GeminiCanvasImageEditFollowupContext {
            prompt: "edit prompt".to_string(),
            request_started_at: std::time::SystemTime::UNIX_EPOCH,
            locale_hint: None,
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        };

        record_gemini_canvas_image_edit_signaler_followup_state_from_body(
            &mut context,
            &session,
            &channel,
            Some("en-US"),
            &body,
        );

        assert_eq!(
            context
                .signaler_session
                .as_ref()
                .map(|value| value.auth_user.as_str()),
            Some("1")
        );
        assert_eq!(
            context
                .signaler_channel
                .as_ref()
                .map(|value| value.next_aid),
            Some(7)
        );
        assert_eq!(context.locale_hint.as_deref(), Some("en-US"));
        assert_eq!(
            context.signaler_response_id.as_deref(),
            Some("r_e0e4aa76ab2755e3")
        );
        assert_eq!(
            context.signaler_conversation_id.as_deref(),
            Some("c_1004db0ef60d9dda")
        );
    }

    #[test]
    fn record_gemini_canvas_image_edit_signaler_followup_state_preserves_existing_locale_contract()
    {
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
            api_key: "api-key".to_string(),
            gsession_id: "gsession-id".to_string(),
            sid: "sid".to_string(),
            next_aid: 7,
        };
        let mut context = GeminiCanvasImageEditFollowupContext {
            prompt: "edit prompt".to_string(),
            request_started_at: std::time::SystemTime::UNIX_EPOCH,
            locale_hint: Some("zh-CN".to_string()),
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        };

        record_gemini_canvas_image_edit_signaler_followup_state(
            &mut context,
            &session,
            &channel,
            Some("en-US"),
        );

        assert_eq!(context.locale_hint.as_deref(), Some("zh-CN"));
        assert_eq!(
            context
                .signaler_session
                .as_ref()
                .map(|value| value.auth_user.as_str()),
            Some("1")
        );
        assert_eq!(
            context
                .signaler_channel
                .as_ref()
                .map(|value| value.next_aid),
            Some(7)
        );
    }

    #[test]
    fn finish_gemini_canvas_image_edit_signaler_missing_asset_records_state_and_error_contract() {
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
            api_key: "api-key".to_string(),
            gsession_id: "gsession-id".to_string(),
            sid: "sid".to_string(),
            next_aid: 9,
        };
        let mut context = GeminiCanvasImageEditFollowupContext {
            prompt: "edit prompt".to_string(),
            request_started_at: std::time::SystemTime::UNIX_EPOCH,
            locale_hint: None,
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        };
        let failures = vec!["poll1=empty".to_string(), "poll2=403".to_string()];

        let error = finish_gemini_canvas_image_edit_signaler_missing_asset(
            Some(&mut context),
            &session,
            &channel,
            Some("en-US"),
            &failures,
            Some("<preview>".to_string()),
        );

        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_signaler_missing_asset")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit signaler poll did not expose a usable media asset. next_aid=9; failures=poll1=empty | poll2=403; last_body_preview=<preview>"
        );
        assert_eq!(context.locale_hint.as_deref(), Some("en-US"));
        assert_eq!(
            context
                .signaler_session
                .as_ref()
                .map(|value| value.auth_user.as_str()),
            Some("1")
        );
        assert_eq!(
            context
                .signaler_channel
                .as_ref()
                .map(|value| value.next_aid),
            Some(9)
        );
    }

    #[test]
    fn record_gemini_canvas_image_edit_signaler_app_path_preserves_app_url_contract() {
        let mut seen_app_paths = std::collections::HashSet::new();
        let mut first_app_path_seen_at = None;
        let mut context = GeminiCanvasImageEditFollowupContext {
            prompt: "edit prompt".to_string(),
            request_started_at: std::time::SystemTime::UNIX_EPOCH,
            locale_hint: None,
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        };

        let page_url = record_gemini_canvas_image_edit_signaler_app_path(
            "https://gemini.google.com",
            "/app/1004db0ef60d9dda",
            &mut seen_app_paths,
            &mut first_app_path_seen_at,
            Some(&mut context),
        );
        let first_seen = first_app_path_seen_at;
        let duplicate_page_url = record_gemini_canvas_image_edit_signaler_app_path(
            "https://gemini.google.com",
            "/app/1004db0ef60d9dda",
            &mut seen_app_paths,
            &mut first_app_path_seen_at,
            Some(&mut context),
        );

        assert_eq!(page_url, "https://gemini.google.com/app/1004db0ef60d9dda");
        assert_eq!(duplicate_page_url, page_url);
        assert!(seen_app_paths.contains(&page_url));
        assert_eq!(seen_app_paths.len(), 1);
        assert!(first_app_path_seen_at.is_some());
        assert_eq!(first_app_path_seen_at, first_seen);
        assert_eq!(context.signaler_app_url.as_deref(), Some(page_url.as_str()));
        assert_eq!(context.signaler_app_urls, vec![page_url]);
    }

    #[test]
    fn gemini_canvas_image_edit_signaler_handoff_ready_preserves_threshold_contract() {
        assert!(!gemini_canvas_image_edit_signaler_handoff_ready(
            0,
            Some(std::time::Duration::from_secs(121)),
        ));
        assert!(!gemini_canvas_image_edit_signaler_handoff_ready(
            2,
            Some(std::time::Duration::from_secs(119)),
        ));
        assert!(gemini_canvas_image_edit_signaler_handoff_ready(
            2,
            Some(std::time::Duration::from_secs(120)),
        ));
        assert!(gemini_canvas_image_edit_signaler_handoff_ready(
            3,
            Some(std::time::Duration::from_secs(1)),
        ));
        assert!(!gemini_canvas_image_edit_signaler_handoff_ready(3, None));
    }

    #[test]
    fn try_finish_gemini_canvas_image_edit_signaler_handoff_ready_records_state_and_error_contract()
    {
        let frame1 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
        ])])
        .expect("frame1");
        let frame2 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
        ])])
        .expect("frame2");
        let body = format!(
            ")]}}'\n\n{}\n{}\n{}\n{}\n",
            frame1.encode_utf16().count(),
            frame1,
            frame2.encode_utf16().count(),
            frame2
        );
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
            api_key: "api-key".to_string(),
            gsession_id: "gsession-id".to_string(),
            sid: "sid".to_string(),
            next_aid: 42,
        };
        let mut context = GeminiCanvasImageEditFollowupContext {
            prompt: "edit prompt".to_string(),
            request_started_at: std::time::SystemTime::UNIX_EPOCH,
            locale_hint: None,
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        };

        let error = try_finish_gemini_canvas_image_edit_signaler_handoff_ready(
            3,
            Some(std::time::Duration::from_secs(1)),
            Some("{\"state\":\"ready\"}"),
            Some(&mut context),
            &session,
            &channel,
            Some("en-US"),
            &body,
        )
        .expect("handoff should be ready");

        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_signaler_handoff_ready")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit signaler reached concrete app paths but has not surfaced a usable image asset yet. distinct_app_paths=3; next_aid=42; last_body_preview={\"state\":\"ready\"}"
        );
        assert_eq!(context.locale_hint.as_deref(), Some("en-US"));
        assert_eq!(
            context
                .signaler_session
                .as_ref()
                .map(|value| value.auth_user.as_str()),
            Some("1")
        );
        assert_eq!(
            context
                .signaler_channel
                .as_ref()
                .map(|value| value.next_aid),
            Some(42)
        );
        assert_eq!(
            context.signaler_response_id.as_deref(),
            Some("r_e0e4aa76ab2755e3")
        );
        assert_eq!(
            context.signaler_conversation_id.as_deref(),
            Some("c_1004db0ef60d9dda")
        );
    }

    #[test]
    fn build_gemini_canvas_image_edit_signaler_poll_url_preserves_contract() {
        let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
            api_key: "api-key".to_string(),
            gsession_id: "gsession-id".to_string(),
            sid: "sid".to_string(),
            next_aid: 42,
        };

        let poll_url = build_gemini_canvas_image_edit_signaler_poll_url(&channel, "zx-token-123");

        assert_eq!(
            poll_url,
            "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?VER=8&gsessionid=gsession-id&key=api-key&RID=rpc&SID=sid&AID=42&CI=0&TYPE=xmlhttp&zx=zx-token-123&t=1"
        );
    }

    #[test]
    fn update_gemini_canvas_image_edit_signaler_next_aid_from_body_preserves_max_aid_contract() {
        let mut channel =
            crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
                api_key: "api-key".to_string(),
                gsession_id: "gsession-id".to_string(),
                sid: "sid".to_string(),
                next_aid: 7,
            };
        let body = concat!(
            "188\n",
            "[[1,[[null,null,[\"d5ty4AOG\"]]]],[2,[[[[\"1\",[[\"1777607372727478\"]]]]]]]]",
            "167\n",
            "[[18,[[[[\"4\",[null,null,[\"1777607613519203\"]]],",
            "[\"3\",[null,null,[\"1777607613519203\"]]]]]]]]"
        );

        let updated =
            update_gemini_canvas_image_edit_signaler_next_aid_from_body(&mut channel, body);

        assert_eq!(updated, Some(18));
        assert_eq!(channel.next_aid, 18);
    }

    #[test]
    fn gemini_canvas_image_edit_signaler_page_failure_entry_preserves_contract() {
        let entry = gemini_canvas_image_edit_signaler_page_failure_entry(
            "https://gemini.google.com/app/abc",
            "fetch",
            "503 service unavailable",
        );

        assert_eq!(
            entry,
            "page_url=https://gemini.google.com/app/abc fetch=503 service unavailable"
        );
    }

    #[test]
    fn gemini_canvas_image_edit_signaler_poll_error_entry_preserves_contract() {
        let entry =
            gemini_canvas_image_edit_signaler_poll_error_entry(42, "transport channel closed");

        assert_eq!(entry, "poll_aid=42 error=transport channel closed");
    }

    #[test]
    fn gemini_canvas_image_edit_signaler_refresh_error_entry_preserves_contract() {
        let entry =
            gemini_canvas_image_edit_signaler_refresh_error_entry(43, "refresh token expired");

        assert_eq!(entry, "refresh_creds aid=43 error=refresh token expired");
    }

    #[test]
    fn extract_gemini_canvas_image_edit_signaler_assets_from_body_preserves_followup_state_contract(
    ) {
        let frame1 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
        ])])
        .expect("frame1");
        let frame2 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
        ])])
        .expect("frame2");
        let body = format!(
            ")]}}'\n\n{}\n{}\n{}\n{}\n<script>window.__IMAGE__=[\"https:\\/\\/lh3.googleusercontent.com\\/rd-ogw\\/asset-token=s32-c\"];</script>",
            frame1.encode_utf16().count(),
            frame1,
            frame2.encode_utf16().count(),
            frame2
        );
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
            api_key: "api-key".to_string(),
            gsession_id: "gsession-id".to_string(),
            sid: "sid".to_string(),
            next_aid: 7,
        };
        let mut context = GeminiCanvasImageEditFollowupContext {
            prompt: "edit prompt".to_string(),
            request_started_at: std::time::SystemTime::UNIX_EPOCH,
            locale_hint: None,
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        };

        let assets = extract_gemini_canvas_image_edit_signaler_assets_from_body(
            &body,
            &session,
            &channel,
            Some("en-US"),
            Some(&mut context),
        )
        .expect("image asset should be extracted");

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "image");
        assert_eq!(assets[0].mime_type, "image/png");
        assert_eq!(
            assets[0].url,
            "https://lh3.googleusercontent.com/rd-ogw/asset-token=s32-c"
        );
        assert_eq!(context.locale_hint.as_deref(), Some("en-US"));
        assert_eq!(
            context
                .signaler_session
                .as_ref()
                .map(|value| value.auth_user.as_str()),
            Some("1")
        );
        assert_eq!(
            context
                .signaler_channel
                .as_ref()
                .map(|value| value.next_aid),
            Some(7)
        );
        assert_eq!(
            context.signaler_response_id.as_deref(),
            Some("r_e0e4aa76ab2755e3")
        );
        assert_eq!(
            context.signaler_conversation_id.as_deref(),
            Some("c_1004db0ef60d9dda")
        );
    }

    #[test]
    fn try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body_preserves_body_contract(
    ) {
        let frame1 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
        ])])
        .expect("frame1");
        let frame2 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
        ])])
        .expect("frame2");
        let body = format!(
            ")]}}'\n\n{}\n{}\n{}\n{}\n<script>window.__IMAGE__=[\"https:\\/\\/lh3.googleusercontent.com\\/rd-ogw\\/asset-token=s32-c\"];</script>",
            frame1.encode_utf16().count(),
            frame1,
            frame2.encode_utf16().count(),
            frame2
        );
        let expected_body = body.clone();
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
            api_key: "api-key".to_string(),
            gsession_id: "gsession-id".to_string(),
            sid: "sid".to_string(),
            next_aid: 7,
        };
        let mut context = GeminiCanvasImageEditFollowupContext {
            prompt: "edit prompt".to_string(),
            request_started_at: std::time::SystemTime::UNIX_EPOCH,
            locale_hint: None,
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        };

        let (assets, returned_body) =
            try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body(
                body,
                &session,
                &channel,
                Some("en-US"),
                Some(&mut context),
            )
            .expect("image asset response should be extracted");

        assert_eq!(returned_body, expected_body);
        assert_eq!(assets.len(), 1);
        assert_eq!(
            context.signaler_response_id.as_deref(),
            Some("r_e0e4aa76ab2755e3")
        );
        assert_eq!(
            context.signaler_conversation_id.as_deref(),
            Some("c_1004db0ef60d9dda")
        );
    }

    #[test]
    fn resolve_gemini_canvas_image_edit_signaler_bootstrap_material_prefers_page_account_and_merges_keys(
    ) {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let storage_state = json!({
            "signalerAccountId": "runtime-account-id",
            "apiKey": "AIzaStorageKey123456789012345",
        });
        let page_body = r#"
            <script>
              window.WIZ_global_data={"S06Grb":"page-account-id"};
              window.firebaseConfig={"apiKey":"AIzaPageKey123456789012345"};
            </script>
        "#;

        let material = resolve_gemini_canvas_image_edit_signaler_bootstrap_material(
            &payload,
            &storage_state,
            page_body,
        )
        .expect("bootstrap material");

        assert_eq!(material.account_id, "page-account-id");
        assert!(material
            .api_key_candidates
            .iter()
            .any(|candidate| candidate == "sk-test"));
        assert!(material
            .api_key_candidates
            .iter()
            .any(|candidate| candidate == "AIzaPageKey123456789012345"));
    }

    #[test]
    fn image_edit_post_ack_missing_app_url_error_matches_contract() {
        let error = gemini_canvas_image_edit_post_ack_missing_app_url_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_post_ack_missing_app_url")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit post-ack follow-up requires a concrete signaler app url."
        );
    }

    #[test]
    fn image_edit_post_ack_bootstrap_missing_error_matches_contract() {
        let error = gemini_canvas_image_edit_post_ack_bootstrap_missing_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_post_ack_bootstrap_missing")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit post-ack follow-up could not bootstrap /app and payload cache did not provide a fallback bootstrap."
        );
    }

    #[test]
    fn image_edit_conversation_bootstrap_missing_error_matches_contract() {
        let error = gemini_canvas_image_edit_conversation_bootstrap_missing_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_conversation_bootstrap_missing")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit conversation follow-up could not bootstrap /app and payload cache did not provide a fallback bootstrap."
        );
    }

    #[test]
    fn image_edit_page_refresh_bootstrap_missing_error_matches_contract() {
        let error = gemini_canvas_image_page_refresh_bootstrap_missing_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_page_refresh_bootstrap_missing")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image page refresh could not bootstrap /app and payload cache did not provide a fallback bootstrap."
        );
    }

    #[test]
    fn image_edit_signaler_missing_account_id_error_matches_contract() {
        let error = gemini_canvas_image_edit_signaler_missing_account_id_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_signaler_missing_account_id")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit signaler bootstrap did not expose S06Grb account id."
        );
    }

    #[test]
    fn image_edit_signaler_missing_api_key_error_matches_contract() {
        let error = gemini_canvas_image_edit_signaler_missing_api_key_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_signaler_missing_api_key")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit signaler bootstrap did not expose a Google API key."
        );
    }

    #[test]
    fn image_edit_signaler_all_keys_failed_error_matches_contract() {
        let error =
            gemini_canvas_image_edit_signaler_all_keys_failed_error("key1=403 | key2=timeout");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_signaler_all_keys_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit signaler bootstrap exhausted all Google API key candidates. failures=key1=403 | key2=timeout"
        );
    }

    #[test]
    fn image_edit_signaler_handoff_ready_error_matches_contract() {
        let error =
            gemini_canvas_image_edit_signaler_handoff_ready_error(3, 42, "{\"state\":\"ready\"}");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_signaler_handoff_ready")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit signaler reached concrete app paths but has not surfaced a usable image asset yet. distinct_app_paths=3; next_aid=42; last_body_preview={\"state\":\"ready\"}"
        );
    }

    #[test]
    fn image_edit_signaler_missing_asset_error_matches_contract() {
        let error = gemini_canvas_image_edit_signaler_missing_asset_error(
            9,
            "poll1=empty | poll2=403",
            "<preview>",
        );
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_signaler_missing_asset")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit signaler poll did not expose a usable media asset. next_aid=9; failures=poll1=empty | poll2=403; last_body_preview=<preview>"
        );
    }

    #[test]
    fn image_edit_conversation_followup_failed_error_matches_contract() {
        let error = gemini_canvas_image_edit_conversation_followup_failed_error(
            "/app/canvas",
            4,
            "prompt-preview",
            "entries-preview",
            "probe-preview",
            "full-preview",
            "completion-preview",
            "parity-probe",
            "parity-full",
            "parity-o30",
            "parity-k4",
            "attempt1=timeout | attempt2=empty",
        );
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_conversation_followup_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edit conversation follow-up did not expose a usable image asset. bootstrap_page=/app/canvas; attempts=4; prompt_preview=prompt-preview; entries_preview=entries-preview; last_probe_preview=probe-preview; last_full_preview=full-preview; last_completion_preview=completion-preview; parity_probe_preview=parity-probe; parity_full_preview=parity-full; parity_o30_preview=parity-o30; parity_k4_preview=parity-k4; failures=attempt1=timeout | attempt2=empty"
        );
    }

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
    fn gemini_canvas_debug_headers_snapshot_redacts_sensitive_values_and_parses_jspb_json() {
        let snapshot = gemini_canvas_debug_headers_snapshot_from_pairs(&[
            ("cookie".to_string(), "SID=secret".to_string()),
            ("authorization".to_string(), "Bearer secret".to_string()),
            (
                "x-goog-ext-73010989-jspb".to_string(),
                "{\"foo\":1}".to_string(),
            ),
        ]);

        assert_eq!(snapshot["raw"]["cookie"], "<redacted>");
        assert_eq!(snapshot["raw"]["authorization"], "<redacted>");
        assert_eq!(snapshot["parsed"]["x-goog-ext-73010989-jspb"]["foo"], 1);
    }

    #[test]
    fn gemini_canvas_debug_form_snapshot_parses_f_req_outer_and_inner_payload() {
        let outer = serde_json::to_string(&json!([null, "{\"hello\":\"world\"}"]))
            .expect("serialize f.req outer");
        let snapshot = gemini_canvas_debug_form_snapshot(&[
            ("f.req".to_string(), outer),
            ("at".to_string(), "token".to_string()),
        ]);

        assert_eq!(snapshot["object"]["at"], "token");
        assert_eq!(
            snapshot["parsed"]["f.req.outer"][1],
            "{\"hello\":\"world\"}"
        );
        assert_eq!(snapshot["parsed"]["f.req.inner"]["hello"], "world");
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
    fn gemini_canvas_image_edit_source_extension_maps_known_mime_types() {
        assert_eq!(
            gemini_canvas_image_edit_source_extension("image/jpeg"),
            "jpg"
        );
        assert_eq!(
            gemini_canvas_image_edit_source_extension("image/webp"),
            "webp"
        );
        assert_eq!(
            gemini_canvas_image_edit_source_extension("image/unknown"),
            "png"
        );
    }

    #[test]
    fn build_gemini_canvas_image_edit_browser_reencode_meta_marks_used_contract() {
        let meta = build_gemini_canvas_image_edit_browser_reencode_meta(
            std::path::Path::new("gateway/scripts/gemini-canvas-image-edit-encode.mjs"),
            std::path::Path::new(".runtime/source.png"),
            std::path::Path::new(".runtime/output.jpg"),
            "encoded",
            "",
            true,
            None,
        );

        assert_eq!(
            meta["scriptPath"],
            "gateway/scripts/gemini-canvas-image-edit-encode.mjs"
        );
        assert_eq!(meta["sourcePath"], ".runtime/source.png");
        assert_eq!(meta["outputPath"], ".runtime/output.jpg");
        assert_eq!(meta["stdout"], "encoded");
        assert_eq!(meta["stderr"], "");
        assert_eq!(meta["used"], true);
        assert!(meta.get("exitCode").is_none());
    }

    #[test]
    fn build_gemini_canvas_image_edit_browser_reencode_meta_preserves_exit_code_contract() {
        let meta = build_gemini_canvas_image_edit_browser_reencode_meta(
            std::path::Path::new("gateway/scripts/gemini-canvas-image-edit-encode.mjs"),
            std::path::Path::new(".runtime/source.png"),
            std::path::Path::new(".runtime/output.jpg"),
            "",
            "failed",
            false,
            Some(9),
        );

        assert_eq!(meta["used"], false);
        assert_eq!(meta["stderr"], "failed");
        assert_eq!(meta["exitCode"], 9);
    }

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
    fn build_gemini_canvas_image_edit_stream_response_heavy_debug_extra_preserves_distinct_values()
    {
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
        let extra =
            build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra(
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
        let extra =
            build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra(
                "credential-runtime/gemini-canvas/runtime.json",
                false,
            );

        assert_eq!(extra["allowReplayTemplate"], false);
    }

    #[test]
    fn build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra_preserves_contract(
    ) {
        let extra =
            build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra(
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
        let extra =
            build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra(
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

    #[test]
    fn build_gemini_canvas_image_edit_upload_response_meta_preserves_start_shape() {
        let meta = build_gemini_canvas_image_edit_upload_response_meta(
            "https://push.clients6.google.com/upload/final",
            Some("text/plain"),
            Some(true),
            "abcdefghijklmnopqrstuvwxyz",
        );

        assert_eq!(
            meta,
            "final_url=https://push.clients6.google.com/upload/final, content_type=text/plain, upload_url_present=true, body_preview=abcdefghijklmnopqrstuvwxyz"
        );
    }

    #[test]
    fn build_gemini_canvas_image_edit_upload_response_meta_preserves_finalize_shape() {
        let meta = build_gemini_canvas_image_edit_upload_response_meta(
            "https://push.clients6.google.com/upload/final",
            None,
            None,
            "",
        );

        assert_eq!(
            meta,
            "final_url=https://push.clients6.google.com/upload/final, content_type=<none>, body_preview=<empty>"
        );
    }

    #[test]
    fn build_gemini_canvas_image_edit_upload_start_request_contract_preserves_shape() {
        let contract = build_gemini_canvas_image_edit_upload_start_request_contract(
            "https://push.clients6.google.com/upload/",
            "source.png",
            "image/png",
            1234,
            "push-id-abcdefghijklmnopqrstuvwxyz",
            "client-pctx-abcdefghijklmnopqrstuvwxyz",
            "https://gemini.google.com/app",
        );

        assert_eq!(
            contract,
            "upload_base_url=https://push.clients6.google.com/upload/, file_name=source.png, mime_type=image/png, bytes=1234, push_id=push-id-abcdefghijklmnop, client_pctx=client-pctx-abcdefghijkl, referer=https://gemini.google.com/app"
        );
    }

    #[test]
    fn build_gemini_canvas_image_edit_upload_start_request_contract_preserves_short_tokens() {
        let contract = build_gemini_canvas_image_edit_upload_start_request_contract(
            "https://push.clients6.google.com/upload/",
            "tiny.png",
            "image/png",
            9,
            "push-1",
            "ctx-1",
            "https://gemini.google.com/app",
        );

        assert_eq!(
            contract,
            "upload_base_url=https://push.clients6.google.com/upload/, file_name=tiny.png, mime_type=image/png, bytes=9, push_id=push-1, client_pctx=ctx-1, referer=https://gemini.google.com/app"
        );
    }

    #[test]
    fn build_gemini_canvas_image_edit_upload_finalize_request_contract_preserves_shape() {
        let contract = build_gemini_canvas_image_edit_upload_finalize_request_contract(
            "https://push.clients6.google.com/upload/session",
            "source.png",
            "image/png",
            1234,
        );

        assert_eq!(
            contract,
            "upload_url=https://push.clients6.google.com/upload/session, file_name=source.png, mime_type=image/png, bytes=1234"
        );
    }

    #[test]
    fn build_gemini_canvas_image_edit_upload_finalize_request_contract_preserves_zero_bytes() {
        let contract = build_gemini_canvas_image_edit_upload_finalize_request_contract(
            "https://push.clients6.google.com/upload/session",
            "empty.bin",
            "application/octet-stream",
            0,
        );

        assert_eq!(
            contract,
            "upload_url=https://push.clients6.google.com/upload/session, file_name=empty.bin, mime_type=application/octet-stream, bytes=0"
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
    fn append_gemini_canvas_image_edit_upload_contracts_preserves_message_contract() {
        let error = GatewayError::service_unavailable("base message")
            .with_provider("gemini_canvas_compatible")
            .with_code("base_code");

        let updated =
            append_gemini_canvas_image_edit_upload_contracts(error, "request=abc", "response=def");

        assert_eq!(
            updated.message,
            "base message; upload_request_contract=request=abc; upload_response_meta=response=def"
        );
        assert_eq!(updated.code.as_deref(), Some("base_code"));
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
        let resource_path =
            extract_gemini_canvas_image_edit_resource_path(None, "not-a-contrib-path");

        assert_eq!(resource_path, None);
    }
}
