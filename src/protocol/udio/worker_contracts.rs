//! Udio browser runtime, worker result and error contracts.

use crate::error::{sanitize_provider_error_message, GatewayError};

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
        .with_provider("udio_compatible")
        .with_code(code)
}

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Udio adapters currently support image, music, and video-generation passthrough endpoints",
    )
    .with_provider("udio_compatible")
    .with_code("unsupported_udio_endpoint")
}

pub fn missing_browser_runtime_error() -> GatewayError {
    GatewayError::server_error(
        "Udio browser-backed requests require a session cookie or runtimeStateObjectKey-backed browser state.",
    )
    .with_provider("udio_compatible")
    .with_code("missing_udio_browser_runtime")
}

pub fn missing_browser_worker_result_error() -> GatewayError {
    GatewayError::server_error("Udio browser worker reported success without a result payload.")
        .with_provider("udio_compatible")
        .with_code("udio_browser_worker_missing_result")
}

pub fn missing_browser_worker_track_ids_error() -> GatewayError {
    GatewayError::server_error("Udio browser worker completed without returning any track ids.")
        .with_provider("udio_compatible")
        .with_code("udio_missing_track_ids")
}

pub fn empty_browser_worker_output_error(stderr: &str) -> GatewayError {
    let stderr = if stderr.is_empty() {
        "<empty>".to_string()
    } else {
        sanitize_browser_worker_detail(stderr)
    };
    sanitized_browser_worker_error(
        &format!("Udio browser worker did not return JSON output. stderr: {stderr}"),
        "udio_browser_worker_empty_output",
    )
}

pub fn browser_worker_output_parse_error(error: &str, stdout: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    let stdout = sanitize_browser_worker_detail(stdout);
    sanitized_browser_worker_error(
        &format!("Failed to parse Udio browser worker output: {error}. stdout: {stdout}"),
        "udio_browser_worker_output_parse_failed",
    )
}

pub fn browser_worker_wait_failed_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Udio browser worker failed before producing output: {error}"),
        "udio_browser_worker_wait_failed",
    )
}

pub fn browser_worker_timeout_error() -> GatewayError {
    GatewayError::server_error("Udio browser worker timed out before producing output.")
        .with_provider("udio_compatible")
        .with_code("udio_browser_worker_timeout")
}

pub fn browser_worker_stdin_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to write Udio browser worker input: {error}"),
        "udio_browser_worker_stdin_failed",
    )
}

pub fn browser_worker_spawn_failed_error(
    _script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to launch Udio browser worker: {error}"),
        "udio_browser_worker_spawn_failed",
    )
}

pub fn browser_worker_input_serialize_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to serialize Udio browser worker input: {error}"),
        "udio_browser_worker_input_serialize_failed",
    )
}

pub fn remote_browser_worker_result_parse_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to parse remote Udio browser worker result: {error}"),
        "udio_remote_result_parse_failed",
    )
}
