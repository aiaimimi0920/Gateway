//! Sequential resumable upload transport and its per-upload resource lifetime.
use super::gemini_canvas_diagnostics::append_gemini_canvas_image_edit_debug_json;
use super::gemini_canvas_error_helpers::classify_gemini_canvas_pure_http_error;
use super::gemini_canvas_image_edit_local_helpers::{
    apply_gemini_canvas_image_edit_upload_base_headers,
    apply_gemini_canvas_image_edit_upload_finalize_headers,
    apply_gemini_canvas_image_edit_upload_start_headers,
    build_gemini_canvas_image_edit_upload_debug_snapshot,
    extract_gemini_canvas_image_edit_resource_path, extract_gemini_canvas_image_edit_upload_url,
    gemini_canvas_image_edit_missing_client_pctx_error,
    gemini_canvas_image_edit_missing_push_id_error,
    gemini_canvas_image_edit_missing_resource_path_error,
    gemini_canvas_image_edit_missing_upload_url_error, gemini_canvas_image_edit_upload_base_url,
};
use super::gemini_canvas_image_encoder::reencode_gemini_canvas_image_edit_input;
use super::gemini_canvas_runtime_helpers::gemini_canvas_http_origin;
use super::gemini_canvas_upload_contract::{
    append_gemini_canvas_image_edit_upload_contracts,
    build_gemini_canvas_image_edit_upload_finalize_request_contract,
    build_gemini_canvas_image_edit_upload_response_meta,
    build_gemini_canvas_image_edit_upload_start_request_contract,
};
use super::gemini_canvas_upload_debug::prepare_gemini_canvas_upload_debug;
use crate::error::{classify_network_error, GatewayError};
use crate::protocol::upstream_body::collect_bounded_upstream_charset_text_with_provider;
use crate::protocol::{gemini_canvas, gemini_web};
use crate::routing::candidate::ProviderAccountPayload;
use bytes::Bytes;
use rquest::header::HeaderMap;
use rquest::{Client, Method};
use std::time::Duration;

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
        let encoding =
            reencode_gemini_canvas_image_edit_input(&upload.source_bytes, &upload.source_mime_type)
                .await;
        let effective_upload_bytes = select_upload_bytes(encoding.bytes, &upload.bytes);
        let browser_reencode_meta = encoding.metadata;

        // Admitted raw capture finishes before a network failure can short-circuit upload.
        let upload_debug = prepare_gemini_canvas_upload_debug(&effective_upload_bytes).await;
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
        // Error diagnostics remain available even when optional snapshots are disabled.
        let start_request_contract = std::cell::LazyCell::new(|| {
            build_gemini_canvas_image_edit_upload_start_request_contract(
                &upload_base_url,
                &upload.file_name,
                &upload.mime_type,
                effective_upload_bytes.len(),
                push_id,
                client_pctx,
                &referer,
            )
        });
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
        let start_body_text = collect_bounded_upstream_charset_text_with_provider(
            start_response,
            "Gemini Canvas upload start response",
            provider,
        )
        .await?;
        let upload_url_present = upload_url.is_some();
        let start_response_meta = std::cell::LazyCell::new(|| {
            build_gemini_canvas_image_edit_upload_response_meta(
                &start_final_url,
                start_content_type.as_deref(),
                Some(upload_url_present),
                &start_body_text,
            )
        });
        if let Some(upload_debug) = &upload_debug {
            append_gemini_canvas_image_edit_debug_json(
                "gemini-canvas-image-edit-upload-debug-start.json",
                || {
                    build_gemini_canvas_image_edit_upload_debug_snapshot(
                        &start_request_contract,
                        &start_response_meta,
                        &start_body_text,
                        upload_url.as_deref(),
                        &upload.file_name,
                        &upload.mime_type,
                        effective_upload_bytes.len(),
                        &upload_debug.sha256,
                        upload_debug.dimensions,
                        upload_debug.output_path.as_deref(),
                        browser_reencode_meta.clone(),
                        None,
                    )
                },
            );
        }
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
        let finalize_request_contract = std::cell::LazyCell::new(|| {
            build_gemini_canvas_image_edit_upload_finalize_request_contract(
                &upload_url,
                &upload.file_name,
                &upload.mime_type,
                effective_upload_bytes.len(),
            )
        });
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
        let finalize_body_text = collect_bounded_upstream_charset_text_with_provider(
            finalize_response,
            "Gemini Canvas upload finalize response",
            provider,
        )
        .await?;
        let finalize_response_meta = std::cell::LazyCell::new(|| {
            build_gemini_canvas_image_edit_upload_response_meta(
                &finalize_final_url,
                finalize_content_type.as_deref(),
                None,
                &finalize_body_text,
            )
        });
        let overridden_resource_path =
            std::env::var("GEMINI_CANVAS_IMAGE_EDIT_RESOURCE_PATH_OVERRIDE")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        if let Some(upload_debug) = &upload_debug {
            append_gemini_canvas_image_edit_debug_json(
                "gemini-canvas-image-edit-upload-debug-finalize.json",
                || {
                    build_gemini_canvas_image_edit_upload_debug_snapshot(
                        &finalize_request_contract,
                        &finalize_response_meta,
                        &finalize_body_text,
                        Some(upload_url.as_str()),
                        &upload.file_name,
                        &upload.mime_type,
                        effective_upload_bytes.len(),
                        &upload_debug.sha256,
                        upload_debug.dimensions,
                        upload_debug.output_path.as_deref(),
                        browser_reencode_meta.clone(),
                        overridden_resource_path.as_deref(),
                    )
                },
            );
        }
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

fn select_upload_bytes(encoded: Option<Vec<u8>>, fallback: &[u8]) -> Bytes {
    // Copy only the fallback; request and diagnostic readers share the selected allocation.
    encoded.map_or_else(|| Bytes::copy_from_slice(fallback), Bytes::from)
}

#[cfg(test)]
#[path = "gemini_canvas_upload_bytes_tests.rs"]
mod upload_bytes_tests;
