//! Best-effort upload diagnostics; admission and bytes outlive caller cancellation.
use super::gemini_canvas_diagnostics::{
    gemini_canvas_image_edit_trace_enabled, write_gemini_canvas_image_edit_debug_bytes,
};
use bytes::Bytes;
use image::GenericImageView;
use sha2::{Digest, Sha256};
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;

static DEBUG_SLOTS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

pub(super) struct GeminiCanvasUploadDebug {
    pub(super) dimensions: Option<(u32, u32)>,
    pub(super) sha256: String,
    pub(super) output_path: Option<String>,
}

pub(super) async fn prepare_gemini_canvas_upload_debug(
    bytes: &Bytes,
) -> Option<GeminiCanvasUploadDebug> {
    run_bounded_debug(
        gemini_canvas_image_edit_trace_enabled(),
        bytes,
        DEBUG_SLOTS.clone(),
        |bytes| build_gemini_canvas_upload_debug(&bytes),
    )
    .await
}

async fn run_bounded_debug<T: Send + 'static>(
    enabled: bool,
    bytes: &Bytes,
    slots: Arc<Semaphore>,
    build: impl FnOnce(Bytes) -> T + Send + 'static,
) -> Option<T> {
    if !enabled {
        return None;
    }
    // Diagnostics must not queue unbounded jobs or make overload an upload error.
    let permit = slots.try_acquire_owned().ok()?;
    let bytes = bytes.clone();
    tokio::task::spawn_blocking(move || {
        // Keep capacity until the real worker exits, even if its caller is dropped.
        let _permit = permit;
        build(bytes)
    })
    .await
    .ok()
}

pub(super) fn build_gemini_canvas_upload_debug(bytes: &[u8]) -> GeminiCanvasUploadDebug {
    let dimensions = image::load_from_memory(bytes)
        .ok()
        .map(|image| image.dimensions());
    let sha256 = hex::encode(Sha256::digest(bytes));
    let file_name = format!(
        "gemini-canvas-image-edit-upload-debug-{}.jpg",
        &sha256[..12]
    );
    let output_path = write_gemini_canvas_image_edit_debug_bytes(&file_name, bytes)
        .map(|path| path.to_string_lossy().to_string());
    GeminiCanvasUploadDebug {
        dimensions,
        sha256,
        output_path,
    }
}

#[cfg(test)]
#[path = "gemini_canvas_upload_debug_tests.rs"]
mod tests;
