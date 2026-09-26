use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::gemini_canvas_debug_file::write_debug_file;
use super::gemini_canvas_debug_redaction::{sanitize_debug_snapshot, sanitize_debug_text};
use super::gemini_canvas_debug_snapshot::{
    gemini_canvas_debug_form_snapshot, gemini_canvas_debug_query_snapshot,
};
use super::gemini_canvas_runtime_paths::gemini_canvas_image_edit_debug_output_path;
use super::gemini_canvas_trace_writer::append_bounded_gemini_canvas_trace;
use super::response_preview_helpers::compact_response_preview;

pub(crate) fn gemini_canvas_image_edit_trace_enabled() -> bool {
    std::env::var_os("GEMINI_CANVAS_IMAGE_EDIT_TRACE").is_some()
}

pub(crate) fn append_gemini_canvas_image_edit_debug_json(
    file_name: &str,
    snapshot: impl FnOnce() -> Value,
) {
    if !gemini_canvas_image_edit_trace_enabled() {
        return;
    }
    let directory = gemini_canvas_image_edit_debug_output_path("gemini-canvas-debug");
    if let Ok(bytes) = serde_json::to_vec_pretty(&sanitize_debug_snapshot(&snapshot())) {
        let _ = write_debug_file(&directory, file_name, &bytes);
    }
}

pub(crate) fn write_gemini_canvas_image_edit_debug_bytes(
    file_name: &str,
    bytes: &[u8],
) -> Option<PathBuf> {
    if !gemini_canvas_image_edit_trace_enabled()
        || (file_name.ends_with(".jpg")
            && std::env::var("GEMINI_CANVAS_IMAGE_EDIT_TRACE_RAW_IMAGES").as_deref() != Ok("1"))
    {
        return None;
    }
    let directory = gemini_canvas_image_edit_debug_output_path("gemini-canvas-debug");
    write_debug_file(&directory, file_name, bytes)
        .ok()
        .flatten()
}

pub(crate) fn append_gemini_canvas_image_edit_request_debug_snapshot(
    file_name: &str,
    stage: &str,
    url: &str,
    query: &[(String, String)],
    form: &[(String, String)],
    headers: impl FnOnce() -> Value,
    extra: impl FnOnce() -> Value,
) {
    if !gemini_canvas_image_edit_trace_enabled() {
        return;
    }
    let captured_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    append_gemini_canvas_image_edit_debug_json(file_name, || {
        json!({
            "stage": stage,
            "capturedAtMs": captured_at_ms,
            "url": url,
            "query": gemini_canvas_debug_query_snapshot(query),
            "form": gemini_canvas_debug_form_snapshot(form),
            "headers": headers(),
            "extra": extra(),
        })
    });
}

pub(crate) fn append_gemini_canvas_image_edit_stream_response_debug_snapshot(
    file_stem: &str,
    stage: &str,
    final_url: &str,
    status: u16,
    location: Option<&str>,
    content_type: Option<&str>,
    body_text: &str,
    extra: impl FnOnce() -> Value,
) {
    if !gemini_canvas_image_edit_trace_enabled() {
        return;
    }
    let body_length = body_text.len();
    let body_text = sanitize_debug_text(body_text);
    append_gemini_canvas_image_edit_debug_json(&format!("{file_stem}.json"), || {
        json!({
            "stage": stage,
            "capturedAtMs": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or_default(),
            "finalUrl": final_url,
            "status": status,
            "location": location,
            "contentType": content_type,
            "bodyLength": body_length,
            "bodyPreview": compact_response_preview(&body_text, 320),
            "extra": extra(),
        })
    });
    let _ = write_gemini_canvas_image_edit_debug_bytes(
        &format!("{file_stem}.txt"),
        body_text.as_bytes(),
    );
}

pub(crate) fn append_gemini_canvas_image_edit_trace<T: AsRef<str>>(
    stage: &str,
    detail: impl FnOnce() -> T,
) {
    if !gemini_canvas_image_edit_trace_enabled() {
        return;
    }
    let path = gemini_canvas_image_edit_debug_output_path("gemini-canvas-image-edit-trace.log");
    let _ = append_bounded_gemini_canvas_trace(&path, stage, detail().as_ref());
}
