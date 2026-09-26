use rquest::header::HeaderMap;
use serde_json::Value;
use std::time::Duration;

use crate::error::GatewayError;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel;
use crate::upstream::gemini_canvas_diagnostics::append_gemini_canvas_image_edit_trace;
use crate::upstream::gemini_canvas_followup_types::GeminiCanvasImageEditFollowupContext;
use crate::upstream::gemini_canvas_request_headers::{
    apply_gemini_canvas_browserish_text_headers, apply_gemini_canvas_cookie_header,
};
use crate::upstream::gemini_canvas_runtime_error_helpers::append_gateway_error_summary;
#[cfg(test)]
use crate::upstream::header_map_helpers::header_map_string;
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
    append_gemini_canvas_image_edit_trace("signaler.handoff-ready", || handoff_message.as_str());
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

#[path = "gemini_canvas_image_edit_snapshots.rs"]
mod image_edit_snapshots;
pub(crate) use image_edit_snapshots::{
    build_gemini_canvas_image_edit_heavy_builder_debug_extra,
    build_gemini_canvas_image_edit_seed_debug_snapshot,
    build_gemini_canvas_image_edit_stream_request_contract,
    build_gemini_canvas_image_edit_stream_response_heavy_debug_extra,
    build_gemini_canvas_image_edit_stream_response_meta,
    build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra,
    build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra,
    build_gemini_canvas_image_edit_template_miss_debug_snapshot,
    build_gemini_canvas_image_edit_template_post_refresh_debug_extra,
    build_gemini_canvas_image_edit_template_pre_refresh_debug_extra,
    build_gemini_canvas_image_edit_upload_debug_snapshot,
    build_gemini_canvas_image_edit_uploaded_refs_debug_snapshot,
};

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

pub(crate) fn extract_gemini_canvas_image_edit_upload_url(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-goog-upload-url")
        .or_else(|| headers.get("x-goog-upload-control-url"))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[path = "gemini_canvas_image_edit_signaler.rs"]
mod signaler;
pub(crate) use signaler::*;

#[cfg(test)]
#[path = "gemini_canvas_image_edit_local_helpers_test_support.rs"]
mod image_edit_local_helpers_test_support;

#[cfg(test)]
#[path = "gemini_canvas_image_edit_local_helpers_error_tests.rs"]
mod image_edit_local_helpers_error_tests;

#[cfg(test)]
#[path = "gemini_canvas_image_edit_local_helpers_transport_tests.rs"]
mod image_edit_local_helpers_transport_tests;

#[cfg(test)]
#[path = "gemini_canvas_image_edit_local_helpers_state_tests.rs"]
mod image_edit_local_helpers_state_tests;

#[cfg(test)]
#[path = "gemini_canvas_image_edit_local_helpers_asset_tests.rs"]
mod image_edit_local_helpers_asset_tests;

#[cfg(test)]
#[path = "gemini_canvas_image_edit_local_helpers_contract_tests.rs"]
mod image_edit_local_helpers_contract_tests;

#[cfg(test)]
#[path = "gemini_canvas_image_edit_local_helpers_snapshot_tests.rs"]
mod image_edit_local_helpers_snapshot_tests;
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

#[path = "gemini_canvas_runtime_mirror.rs"]
mod runtime_mirror;
pub(crate) use runtime_mirror::{
    gemini_canvas_runtime_mirror_json_path, read_gemini_canvas_runtime_mirror_json,
};

pub(crate) fn gemini_canvas_sidecar_has_any_key(value: &Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| value.get(*key).is_some())
}
