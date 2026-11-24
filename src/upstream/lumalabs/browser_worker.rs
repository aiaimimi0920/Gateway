//! Luma Web metadata worker: reuse bounded pipes, shared slots and process-tree reaping.
use crate::{
    error::{ErrorKind, FallbackHint, GatewayError},
    upstream::{
        lumalabs_response_helpers::parse_lumalabs_browser_worker_verified_output,
        lumalabs_runtime_helpers::PreparedLumalabsBrowserWorkerLaunch,
        producer_browser_worker_io::WorkerOutputLimits,
        producer_browser_worker_process::execute_browser_worker_process_with_limits,
        PreparedProducerBrowserWorkerLaunch,
    },
};
use std::time::Duration;

pub(super) fn no_replay(mut error: GatewayError) -> GatewayError {
    error.provider_name = Some("lumalabs_compatible".into());
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Luma Web generation must not be replayed; remote credits may already be spent."
            .into(),
    };
    error
}
fn failure(code: &'static str, message: &str) -> GatewayError {
    no_replay(GatewayError::server_error(message).with_code(code))
}
pub(super) fn timeout_error() -> GatewayError {
    let mut error = failure(
        "lumalabs_browser_worker_timeout",
        "Luma Web execution timed out; stopping locally does not cancel remote generation.",
    );
    error.http_status = Some(504);
    error.kind = ErrorKind::Timeout;
    error
}
fn project_error(error: GatewayError) -> GatewayError {
    // Shared Producer identities and arbitrary OS/pipe diagnostics are not public Luma details.
    let (code, message) = match error.code.as_deref() {
        Some("producer_browser_worker_timeout") => return timeout_error(),
        Some("producer_browser_worker_spawn_failed") => (
            "lumalabs_browser_worker_spawn_failed",
            "Unable to launch the Luma Web browser worker.",
        ),
        Some("producer_browser_worker_stdin_failed") => (
            "lumalabs_browser_worker_stdin_failed",
            "Unable to write the Luma Web browser worker input.",
        ),
        Some("producer_browser_worker_nonzero_exit") => (
            "lumalabs_browser_worker_nonzero_exit",
            "Luma Web browser worker exited unsuccessfully; details omitted.",
        ),
        Some("producer_browser_worker_output_too_large") => (
            "lumalabs_browser_worker_output_too_large",
            "Luma Web browser worker exceeded its metadata byte budget.",
        ),
        Some("producer_browser_worker_output_buffer_allocation_failed") => (
            "lumalabs_browser_worker_output_buffer_allocation_failed",
            "Luma Web browser worker output allocation failed.",
        ),
        _ => (
            "lumalabs_browser_worker_wait_failed",
            "Luma Web browser worker execution failed; details omitted.",
        ),
    };
    failure(code, message)
}

pub(super) async fn run_value(
    launch: PreparedLumalabsBrowserWorkerLaunch,
    timeout: Duration,
    provider: &str,
) -> Result<serde_json::Value, GatewayError> {
    if launch.stdin_json.len() > 1024 * 1024 {
        return Err(failure(
            "lumalabs_browser_worker_input_too_large",
            "Luma Web worker input exceeded 1 MiB.",
        ));
    }
    // This deadline includes slot admission and pipe writes. Drop signals the existing supervisor,
    // which keeps ownership of the child/tree and permit until its asynchronous cleanup completes.
    let output = tokio::time::timeout(
        timeout,
        execute_browser_worker_process_with_limits(
            PreparedProducerBrowserWorkerLaunch {
                node_bin: launch.node_bin,
                script_path: launch.script_path,
                stdin_json: launch.stdin_json,
            },
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
            "lumalabs_browser_worker_output_parse_failed",
            "Luma Web worker returned invalid UTF-8; details omitted.",
        )
    })?;
    let _ = provider;
    serde_json::from_str(stdout).map_err(|_| {
        failure(
            "lumalabs_browser_worker_output_parse_failed",
            "Luma Web worker returned invalid JSON; details omitted.",
        )
    })
}

pub(super) fn parse_task(
    value: serde_json::Value,
    provider: &str,
) -> Result<super::task_result::TaskResult, GatewayError> {
    let stdout = value.to_string();
    parse_lumalabs_browser_worker_verified_output(stdout.trim(), "", provider).map_err(
        |mut error| {
            if matches!(
                error.code.as_deref(),
                Some(
                    "lumalabs_browser_worker_empty_output"
                        | "lumalabs_browser_worker_output_parse_failed"
                )
            ) {
                error.message =
                    "Luma Web worker returned invalid or empty JSON; details omitted.".into();
            }
            no_replay(error)
        },
    )?;
    super::task_result::TaskResult::from_value(value)
}

#[cfg(test)]
pub(super) async fn run(
    launch: PreparedLumalabsBrowserWorkerLaunch,
    timeout: Duration,
    provider: &str,
) -> Result<super::task_result::TaskResult, GatewayError> {
    parse_task(run_value(launch, timeout, provider).await?, provider)
}
