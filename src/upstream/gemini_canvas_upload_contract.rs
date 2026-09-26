use super::gemini_canvas_runtime_error_helpers::{
    append_gateway_error_summary, compact_sanitized_response_preview,
};
use super::response_preview_helpers::truncate_response_preview;
use crate::error::GatewayError;

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
            compact_sanitized_response_preview(body_text, 240)
        ),
        None => format!(
            "final_url={final_url}, content_type={}, body_preview={}",
            content_type.unwrap_or("<none>"),
            compact_sanitized_response_preview(body_text, 240)
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

#[cfg(test)]
#[path = "gemini_canvas_upload_contract_tests.rs"]
mod tests;
