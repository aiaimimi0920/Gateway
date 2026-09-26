//! Suno browser worker result and error contracts.

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
        .with_provider("suno_compatible")
        .with_code(code)
}

pub fn unsupported_media_endpoint_error() -> GatewayError {
    GatewayError::bad_request("Suno response finalizer received an unsupported endpoint.")
        .with_provider("suno_compatible")
        .with_code("unsupported_suno_endpoint")
}

pub fn missing_browser_worker_result_error() -> GatewayError {
    GatewayError::server_error("Suno browser worker reported success without a result payload.")
        .with_provider("suno_compatible")
        .with_code("suno_browser_worker_missing_result")
}

pub fn missing_browser_worker_clips_error() -> GatewayError {
    GatewayError::server_error("Suno browser worker completed without returning any clips.")
        .with_provider("suno_compatible")
        .with_code("suno_missing_feed_clips")
}

pub fn empty_browser_worker_output_error(stderr: &str) -> GatewayError {
    let stderr = if stderr.is_empty() {
        "<empty>".to_string()
    } else {
        sanitize_browser_worker_detail(stderr)
    };
    sanitized_browser_worker_error(
        &format!("Suno browser worker did not return JSON output. stderr: {stderr}"),
        "suno_browser_worker_empty_output",
    )
}

pub fn browser_worker_output_parse_error(error: &str, stdout: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    let stdout = sanitize_browser_worker_detail(stdout);
    sanitized_browser_worker_error(
        &format!("Failed to parse Suno browser worker output: {error}. stdout: {stdout}"),
        "suno_browser_worker_output_parse_failed",
    )
}

pub fn browser_worker_wait_failed_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Suno browser worker failed before producing output: {error}"),
        "suno_browser_worker_wait_failed",
    )
}

pub fn browser_worker_timeout_error() -> GatewayError {
    GatewayError::server_error("Suno browser worker timed out before producing output.")
        .with_provider("suno_compatible")
        .with_code("suno_browser_worker_timeout")
}

pub fn browser_worker_stdin_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to write Suno browser worker input: {error}"),
        "suno_browser_worker_stdin_failed",
    )
}

pub fn browser_worker_spawn_failed_error(
    _script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to launch Suno browser worker: {error}"),
        "suno_browser_worker_spawn_failed",
    )
}

pub fn browser_worker_input_serialize_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to serialize Suno browser worker input: {error}"),
        "suno_browser_worker_input_serialize_failed",
    )
}

pub fn remote_browser_worker_result_parse_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to parse remote Suno browser worker result: {error}"),
        "suno_remote_result_parse_failed",
    )
}
