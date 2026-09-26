//! Decode HTTP replay output and serialize browser executor service results.

use serde_json::{json, Value};

use crate::error::{classify_upstream_error, GatewayError};
use crate::upstream::browser_worker_runtime_helpers::{
    gemini_canvas_http_replay_worker_empty_output_error,
    gemini_canvas_http_replay_worker_output_parse_error,
};
use crate::upstream::browser_worker_types::{
    GeminiCanvasHttpReplayWorkerResult, GeminiCanvasHttpReplayWorkerSuccess,
};
use crate::upstream::gemini::canvas_program_web_reverse as program;

pub(crate) fn parse_http_replay_worker_output(
    stdout: &str,
    stderr: &str,
) -> Result<GeminiCanvasHttpReplayWorkerResult, GatewayError> {
    if stdout.trim().is_empty() {
        return Err(gemini_canvas_http_replay_worker_empty_output_error(stderr));
    }

    serde_json::from_str::<GeminiCanvasHttpReplayWorkerResult>(stdout).map_err(|error| {
        gemini_canvas_http_replay_worker_output_parse_error(error.to_string().as_str(), stdout)
    })
}

pub(crate) fn extract_http_replay_worker_success(
    result: GeminiCanvasHttpReplayWorkerResult,
) -> Result<GeminiCanvasHttpReplayWorkerSuccess, GatewayError> {
    Ok(GeminiCanvasHttpReplayWorkerSuccess {
        status: result.status.unwrap_or(200),
        content_type: result.content_type,
        body_text: result.body_text.unwrap_or_default(),
    })
}

pub(crate) fn classify_http_replay_worker_failure(
    result: GeminiCanvasHttpReplayWorkerResult,
    stderr: &str,
    provider: &str,
) -> GatewayError {
    let status = result
        .error
        .as_ref()
        .and_then(|entry| entry.status)
        .or(result.status)
        .unwrap_or(500);
    let body_text = result
        .error
        .as_ref()
        .and_then(|entry| entry.body_text.as_deref())
        .or_else(|| stderr.is_empty().then_some("").or(Some(stderr)))
        .unwrap_or_default();
    let mut gateway_error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(worker_error) = result.error {
        if let Some(code) = worker_error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = worker_error.message {
            gateway_error.message = message;
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = Some(status);
        }
    }
    gateway_error
}

pub(crate) fn build_gemini_canvas_browser_executor_service_result(
    result: &program::GeminiCanvasBrowserInvocationResult,
) -> Value {
    json!({
        "operation": result.operation,
        "bodyText": result.body_text,
        "media": result
            .media
            .iter()
            .map(|asset| {
                json!({
                    "kind": asset.kind,
                    "url": asset.url,
                    "mimeType": asset.mime_type,
                    "alt": asset.alt,
                    "width": asset.width,
                    "height": asset.height,
                    "durationSeconds": asset.duration_seconds,
                })
            })
            .collect::<Vec<_>>(),
    })
}
