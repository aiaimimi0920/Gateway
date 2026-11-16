//! Udio metadata subprocess: shared tree/stdio ownership and no generation replay.
use crate::{
    error::{ErrorKind, FallbackHint, GatewayError},
    upstream::{
        browser_worker_types::UdioBrowserWorkerSuccess,
        producer_browser_worker_io::WorkerOutputLimits,
        producer_browser_worker_process::execute_browser_worker_process_with_limits,
        udio_response_helpers::parse_udio_browser_worker_verified_output,
        PreparedProducerBrowserWorkerLaunch,
    },
};
use std::time::Duration;

pub(super) fn no_replay(mut error: GatewayError) -> GatewayError {
    error.provider_name = Some("udio_compatible".into());
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Udio generation must not be replayed; remote credits may already be spent.".into(),
    };
    error
}
fn failure(code: &'static str, message: &str) -> GatewayError {
    no_replay(GatewayError::server_error(message).with_code(code))
}
pub(super) fn timeout_error() -> GatewayError {
    let mut error = failure(
        "udio_browser_worker_timeout",
        "Udio execution timed out; stopping locally does not cancel remote generation.",
    );
    error.http_status = Some(504);
    error.kind = ErrorKind::Timeout;
    error
}
fn project_error(error: GatewayError) -> GatewayError {
    // Shared Producer codes and arbitrary OS/pipe diagnostics are not a public detail surface.
    let (code, message) = match error.code.as_deref() {
        Some("producer_browser_worker_timeout") => return timeout_error(),
        Some("producer_browser_worker_spawn_failed") => (
            "udio_browser_worker_spawn_failed",
            "Unable to launch the Udio browser worker.",
        ),
        Some("producer_browser_worker_stdin_failed") => (
            "udio_browser_worker_stdin_failed",
            "Unable to write the Udio browser worker input.",
        ),
        Some("producer_browser_worker_nonzero_exit") => (
            "udio_browser_worker_nonzero_exit",
            "Udio browser worker exited unsuccessfully; details omitted.",
        ),
        Some("producer_browser_worker_output_too_large") => (
            "udio_browser_worker_output_too_large",
            "Udio browser worker exceeded its metadata byte budget.",
        ),
        Some("producer_browser_worker_output_buffer_allocation_failed") => (
            "udio_browser_worker_output_buffer_allocation_failed",
            "Udio browser worker output allocation failed.",
        ),
        _ => (
            "udio_browser_worker_wait_failed",
            "Udio browser worker execution failed; details omitted.",
        ),
    };
    failure(code, message)
}

pub(super) async fn run(
    launch: PreparedProducerBrowserWorkerLaunch,
    timeout: Duration,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    if launch.stdin_json.len() > 1024 * 1024 {
        return Err(failure(
            "udio_browser_worker_input_too_large",
            "Udio worker input exceeded 1 MiB.",
        ));
    }
    // Includes shared-slot waiting and stdin writes. Dropping this future signals the existing
    // supervisor; it retains its process/permit until asynchronous tree cleanup finishes.
    let output = tokio::time::timeout(
        timeout,
        execute_browser_worker_process_with_limits(
            launch,
            timeout,
            WorkerOutputLimits {
                stdout: 128 * 1024,
                stderr: 16 * 1024,
            },
        ),
    )
    .await
    .map_err(|_| timeout_error())?
    .map_err(project_error)?;
    let stdout = std::str::from_utf8(&output.stdout).map_err(|_| {
        failure(
            "udio_browser_worker_output_parse_failed",
            "Udio browser worker returned invalid UTF-8; details omitted.",
        )
    })?;
    // stderr is bounded for resource safety, never forwarded as an arbitrary business body.
    parse_udio_browser_worker_verified_output(stdout.trim(), "", "udio_compatible").map_err(
        |mut error| {
            if matches!(
                error.code.as_deref(),
                Some(
                    "udio_browser_worker_empty_output" | "udio_browser_worker_output_parse_failed"
                )
            ) {
                error.message =
                    "Udio browser worker returned invalid or empty JSON; details omitted.".into();
            }
            no_replay(error)
        },
    )
}
