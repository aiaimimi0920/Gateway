//! Mandatory normalization has bounded submission, unlike best-effort diagnostics.
use crate::error::GatewayError;
use crate::protocol::{gemini_business, gemini_canvas};
use serde_json::Value;
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;

static NORMALIZATION_SLOTS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(2)));

pub(super) async fn normalize_uploads(
    body: &Value,
) -> Result<Vec<gemini_canvas::GeminiCanvasImageEditUpload>, GatewayError> {
    normalize_with(
        body,
        NORMALIZATION_SLOTS.clone(),
        gemini_canvas::normalize_image_edit_uploads,
    )
    .await
}

async fn normalize_with(
    body: &Value,
    slots: Arc<Semaphore>,
    normalize: impl FnOnce(
            Vec<gemini_business::GeminiBusinessUpload>,
        ) -> Result<Vec<gemini_canvas::GeminiCanvasImageEditUpload>, GatewayError>
        + Send
        + 'static,
) -> Result<Vec<gemini_canvas::GeminiCanvasImageEditUpload>, GatewayError> {
    // Preserve the parser's empty/non-array images behavior without submitting work.
    if !body
        .get("images")
        .and_then(Value::as_array)
        .is_some_and(|images| !images.is_empty())
        && body.get("mask").is_none()
    {
        return Ok(Vec::new());
    }
    // Admission precedes copying base64 strings. Mandatory inputs are never skipped.
    let permit = slots.try_acquire_owned().map_err(|_| {
        GatewayError::service_unavailable("Gemini Canvas image normalization is busy.")
            .with_provider("gemini_canvas_compatible")
            .with_code("image_edit_normalization_busy")
    })?;
    let uploads = gemini_business::extract_uploads_from_request_body(body)?;
    tokio::task::spawn_blocking(move || {
        // Caller cancellation cannot release capacity while owned decoding still runs.
        let _permit = permit;
        normalize(uploads)
    })
    .await
    .map_err(|_| {
        GatewayError::server_error("Gemini Canvas image normalization worker failed.")
            .with_provider("gemini_canvas_compatible")
            .with_code("image_edit_normalization_worker_failed")
    })?
}

#[cfg(test)]
#[path = "gemini_canvas_image_normalization_tests.rs"]
mod tests;
