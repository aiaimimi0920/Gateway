//! Suno metadata worker: bounded pipes and shared process-tree/cancellation ownership.
use super::transport::no_replay;
use crate::{
    error::{ErrorKind, GatewayError},
    upstream::{
        browser_worker_types::SunoBrowserWorkerSuccess,
        producer_browser_worker_io::WorkerOutputLimits,
        producer_browser_worker_process::execute_browser_worker_process_with_limits,
        suno_response_helpers::parse_suno_browser_worker_verified_output,
        PreparedProducerBrowserWorkerLaunch,
    },
};
use std::time::Duration;

const INPUT_LIMIT: usize = 1024 * 1024;
const OUTPUT_LIMITS: WorkerOutputLimits = WorkerOutputLimits {
    stdout: 128 * 1024,
    stderr: 16 * 1024,
};

fn failure(code: &'static str, message: &str) -> GatewayError {
    no_replay(
        GatewayError::server_error(message)
            .with_provider("suno_compatible")
            .with_code(code),
    )
}
pub(super) fn timeout_error() -> GatewayError {
    let mut error = failure(
        "suno_browser_worker_timeout",
        "Suno browser worker timed out; remote generation may still exist.",
    );
    error.http_status = Some(504);
    error.kind = ErrorKind::Timeout;
    error
}
fn project_error(error: GatewayError) -> GatewayError {
    // The shared supervisor's Producer namespace and raw process details never escape this owner.
    let (code, message) = match error.code.as_deref() {
        Some("producer_browser_worker_timeout") => return timeout_error(),
        Some("producer_browser_worker_spawn_failed") => (
            "suno_browser_worker_spawn_failed",
            "Unable to launch the Suno browser worker.",
        ),
        Some("producer_browser_worker_stdin_failed") => (
            "suno_browser_worker_stdin_failed",
            "Unable to write the Suno browser worker input.",
        ),
        Some("producer_browser_worker_nonzero_exit") => (
            "suno_browser_worker_nonzero_exit",
            "Suno browser worker exited unsuccessfully; output details omitted.",
        ),
        Some("producer_browser_worker_output_too_large") => (
            "suno_browser_worker_output_too_large",
            "Suno browser worker output exceeded its metadata byte budget.",
        ),
        Some("producer_browser_worker_output_buffer_allocation_failed") => (
            "suno_browser_worker_output_buffer_allocation_failed",
            "Suno browser worker output allocation failed.",
        ),
        _ => (
            "suno_browser_worker_wait_failed",
            "Suno browser worker execution failed; output details omitted.",
        ),
    };
    failure(code, message)
}

pub(super) async fn run(
    launch: PreparedProducerBrowserWorkerLaunch,
    timeout: Duration,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    if launch.stdin_json.len() > INPUT_LIMIT {
        return Err(failure(
            "suno_browser_worker_input_too_large",
            "Suno browser worker input exceeded 1 MiB.",
        ));
    }
    // This outer deadline includes waiting for a shared slot and stdin writes, not just child.wait().
    // Drop cancels the existing supervisor, which retains ownership until its child/tree are reaped.
    let output = tokio::time::timeout(
        timeout,
        execute_browser_worker_process_with_limits(launch, timeout, OUTPUT_LIMITS),
    )
    .await
    .map_err(|_| timeout_error())?
    .map_err(project_error)?;
    parse_suno_browser_worker_verified_output(
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim(),
        "suno_compatible",
    )
    .map_err(|mut error| {
        if matches!(
            error.code.as_deref(),
            Some("suno_browser_worker_empty_output" | "suno_browser_worker_output_parse_failed")
        ) {
            error.message =
                "Suno browser worker returned invalid or empty JSON; output details omitted."
                    .into();
        }
        no_replay(error)
    })
}
