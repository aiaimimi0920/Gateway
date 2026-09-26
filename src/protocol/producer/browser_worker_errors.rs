use crate::error::{sanitize_provider_error_message, GatewayError};

const BROWSER_WORKER_DETAIL_MAX_BYTES: usize = 16 * 1024;
const OVERSIZED_BROWSER_WORKER_DETAIL: &str =
    "[upstream detail omitted: safe processing limit exceeded]";

fn sanitize_browser_worker_detail(value: &str) -> String {
    if value.len() > BROWSER_WORKER_DETAIL_MAX_BYTES {
        OVERSIZED_BROWSER_WORKER_DETAIL.to_string()
    } else {
        sanitize_provider_error_message(value)
    }
}

fn sanitized_browser_worker_error(message: &str, code: &'static str) -> GatewayError {
    GatewayError::server_error(sanitize_browser_worker_detail(message))
        .with_provider("producer_compatible")
        .with_code(code)
}

pub fn missing_browser_worker_result_error() -> GatewayError {
    GatewayError::server_error("Producer browser worker reported success without a result payload.")
        .with_provider("producer_compatible")
        .with_code("producer_browser_worker_missing_result")
}

pub fn empty_browser_worker_output_error(stderr: &str) -> GatewayError {
    let stderr = if stderr.is_empty() {
        "<empty>".to_string()
    } else {
        sanitize_browser_worker_detail(stderr)
    };
    sanitized_browser_worker_error(
        &format!(
            "Producer browser worker did not return JSON output. stderr: {}",
            stderr
        ),
        "producer_browser_worker_empty_output",
    )
}

pub fn browser_worker_output_parse_error(error: &str, stdout: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    let stdout = sanitize_browser_worker_detail(stdout);
    sanitized_browser_worker_error(
        &format!("Failed to parse Producer browser worker output: {error}. stdout: {stdout}"),
        "producer_browser_worker_output_parse_failed",
    )
}

pub fn browser_worker_output_too_large_error(stream: &str, max_bytes: usize) -> GatewayError {
    let stream = if stream == "stderr" {
        "stderr"
    } else {
        "stdout"
    };
    GatewayError::server_error(format!(
        "Producer browser worker {stream} exceeded the {max_bytes}-byte output limit."
    ))
    .with_provider("producer_compatible")
    .with_code("producer_browser_worker_output_too_large")
}

pub fn browser_worker_output_buffer_allocation_error(stream: &str) -> GatewayError {
    let stream = if stream == "stderr" {
        "stderr"
    } else {
        "stdout"
    };
    GatewayError::server_error(format!(
        "Producer browser worker {stream} buffer allocation failed."
    ))
    .with_provider("producer_compatible")
    .with_code("producer_browser_worker_output_buffer_allocation_failed")
}

pub fn browser_worker_nonzero_exit_error(exit_code: Option<i32>, stderr: &str) -> GatewayError {
    let exit_code = exit_code
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let stderr = if stderr.is_empty() {
        "<empty>".to_string()
    } else {
        sanitize_browser_worker_detail(stderr)
    };
    sanitized_browser_worker_error(
        &format!(
            "Producer browser worker exited unsuccessfully with code {exit_code}. stderr: {stderr}"
        ),
        "producer_browser_worker_nonzero_exit",
    )
}

pub fn browser_worker_wait_failed_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Producer browser worker failed before producing output: {error}"),
        "producer_browser_worker_wait_failed",
    )
}

pub fn browser_worker_timeout_error() -> GatewayError {
    GatewayError::server_error("Producer browser worker timed out before producing output.")
        .with_provider("producer_compatible")
        .with_code("producer_browser_worker_timeout")
}

pub fn browser_worker_stdin_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to write Producer browser worker input: {error}"),
        "producer_browser_worker_stdin_failed",
    )
}

pub fn browser_worker_spawn_failed_error(
    _script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to launch Producer browser worker: {error}"),
        "producer_browser_worker_spawn_failed",
    )
}

pub fn browser_worker_input_serialize_error(error: &str) -> GatewayError {
    let error = sanitize_browser_worker_detail(error);
    sanitized_browser_worker_error(
        &format!("Failed to serialize Producer browser worker input: {error}"),
        "producer_browser_worker_input_serialize_failed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn untrusted_worker_text() -> String {
        format!(
            "phase failed\nBearer fake-bearer-secret\r\nsk-test-fake-secret-123456\ntoken=fake-assignment-secret\n{}",
            "界".repeat(700)
        )
    }

    fn assert_sanitized(error: &GatewayError, expected_code: &str) {
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.code.as_deref(), Some(expected_code));
        assert!(error.message.contains("[REDACTED]"));
        assert!(!error.message.contains("fake-bearer-secret"));
        assert!(!error.message.contains("sk-test-fake-secret-123456"));
        assert!(!error.message.contains("fake-assignment-secret"));
        assert!(!error.message.chars().any(char::is_control));
        assert!(error.message.chars().count() <= 512);
    }

    #[test]
    fn dynamic_browser_worker_errors_sanitize_and_bound_untrusted_text() {
        let raw = untrusted_worker_text();
        for (error, code) in [
            (
                empty_browser_worker_output_error(&raw),
                "producer_browser_worker_empty_output",
            ),
            (
                browser_worker_output_parse_error(&raw, &raw),
                "producer_browser_worker_output_parse_failed",
            ),
            (
                browser_worker_wait_failed_error(&raw),
                "producer_browser_worker_wait_failed",
            ),
            (
                browser_worker_nonzero_exit_error(Some(23), &raw),
                "producer_browser_worker_nonzero_exit",
            ),
            (
                browser_worker_stdin_error(&raw),
                "producer_browser_worker_stdin_failed",
            ),
            (
                browser_worker_input_serialize_error(&raw),
                "producer_browser_worker_input_serialize_failed",
            ),
        ] {
            assert_sanitized(&error, code);
        }
    }

    #[test]
    fn spawn_error_hides_local_script_path_and_sanitizes_cause() {
        let error = browser_worker_spawn_failed_error(
            std::path::Path::new("C:/Users/private-user/runtime/producer-browser-worker.mjs"),
            &untrusted_worker_text(),
        );

        assert_sanitized(&error, "producer_browser_worker_spawn_failed");
        assert!(error
            .message
            .starts_with("Failed to launch Producer browser worker:"));
        assert!(!error.message.contains("private-user"));
        assert!(!error.message.contains("producer-browser-worker.mjs"));
    }

    #[test]
    fn oversized_browser_worker_details_are_omitted_before_sanitizing() {
        let oversized = format!(
            "{}token=fake-oversized-tail-secret",
            "x".repeat(BROWSER_WORKER_DETAIL_MAX_BYTES + 1)
        );
        let error = browser_worker_output_parse_error(&oversized, &oversized);

        assert_eq!(
            error.code.as_deref(),
            Some("producer_browser_worker_output_parse_failed")
        );
        assert!(error.message.contains("safe processing limit exceeded"));
        assert!(!error.message.contains("fake-oversized-tail-secret"));
        assert!(error.message.chars().count() <= 512);
    }
}
