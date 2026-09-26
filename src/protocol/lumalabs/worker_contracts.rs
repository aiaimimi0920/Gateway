//! LumaLabs worker result admission and error contracts.

use crate::error::{sanitize_provider_error_message, GatewayError};
use serde_json::Value;

const BROWSER_WORKER_DETAIL_MAX_BYTES: usize = 16 * 1024;
const OVERSIZED_BROWSER_WORKER_DETAIL: &str =
    "[upstream detail omitted: safe processing limit exceeded]";

fn sanitize_browser_worker_detail(value: &str) -> String {
    // Admit before copying or regex processing untrusted worker output.
    if value.len() > BROWSER_WORKER_DETAIL_MAX_BYTES {
        OVERSIZED_BROWSER_WORKER_DETAIL.to_string()
    } else {
        sanitize_provider_error_message(value)
    }
}

fn sanitized_browser_worker_error(message: &str, code: &'static str) -> GatewayError {
    GatewayError::server_error(sanitize_browser_worker_detail(message))
        .with_provider("lumalabs_compatible")
        .with_code(code)
}

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "LumaLabs adapters currently support image, video, and audio-generation passthrough endpoints",
    )
    .with_code("unsupported_lumalabs_endpoint")
}

pub fn unsupported_image_inputs_error() -> GatewayError {
    GatewayError::bad_request(
        "LumaLabs image generation currently supports prompt-only requests and does not accept image uploads or masks.",
    )
    .with_provider("lumalabs_compatible")
    .with_code("unsupported_lumalabs_image_inputs")
}

pub fn extract_remote_executor_signed_url(value: &Value) -> Result<String, GatewayError> {
    value
        .get("signedUrl")
        .and_then(|entry| entry.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Remote LumaLabs browser executor succeeded without a signedUrl.",
            )
            .with_provider("lumalabs_compatible")
            .with_code("lumalabs_browser_executor_missing_signed_url")
        })
}

pub fn extract_browser_worker_signed_url(
    signed_url: Option<String>,
) -> Result<String, GatewayError> {
    signed_url.ok_or_else(|| {
        GatewayError::server_error("LumaLabs browser worker reported success without a signed URL.")
            .with_provider("lumalabs_compatible")
            .with_code("lumalabs_browser_worker_missing_signed_url")
    })
}

pub fn empty_browser_worker_output_error(stderr: &str) -> GatewayError {
    let stderr = if stderr.is_empty() {
        "<empty>".to_string()
    } else {
        sanitize_browser_worker_detail(stderr)
    };
    sanitized_browser_worker_error(
        &format!("LumaLabs browser worker did not return JSON output. stderr: {stderr}"),
        "lumalabs_browser_worker_empty_output",
    )
}

pub fn browser_worker_output_parse_error(error: &str, stdout: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    let stdout = sanitize_browser_worker_detail(stdout);
    sanitized_browser_worker_error(
        &format!("Failed to parse LumaLabs browser worker output: {error}. stdout: {stdout}"),
        "lumalabs_browser_worker_output_parse_failed",
    )
}

pub fn browser_worker_wait_failed_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("LumaLabs browser worker failed before producing output: {error}"),
        "lumalabs_browser_worker_wait_failed",
    )
}

pub fn browser_worker_stdin_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to write LumaLabs browser worker input: {error}"),
        "lumalabs_browser_worker_stdin_failed",
    )
}

pub fn browser_worker_spawn_failed_error(
    _script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to launch LumaLabs browser worker: {error}"),
        "lumalabs_browser_worker_spawn_failed",
    )
}

pub fn browser_worker_input_serialize_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to serialize LumaLabs browser worker input: {error}"),
        "lumalabs_browser_worker_input_serialize_failed",
    )
}
