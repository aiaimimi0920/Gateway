use super::{Client, FreeBuffRuntimeConfig, GatewayError, OwnedSemaphorePermit, RunLifetime};
use crate::protocol::freebuff::transport::start_run;
use std::time::Duration;

const START_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
#[path = "run_start_tests.rs"]
mod tests;

pub(super) async fn start_tracked_run(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
    permit: OwnedSemaphorePermit,
) -> Result<(String, RunLifetime), GatewayError> {
    let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
        GatewayError::service_unavailable("FreeBuff START requires a running runtime")
            .with_code("freebuff_start_runtime_unavailable")
            .with_provider("freebuff_compatible")
    })?;
    let (client, config) = (client.clone(), config.clone());
    // Dropping the caller's JoinHandle detaches, not aborts, this permit-bounded operation.
    runtime
        .spawn(async move {
            let result = tokio::time::timeout(START_TIMEOUT, start_run(&client, &config))
                .await
                .unwrap_or_else(|_| {
                    Err(
                        GatewayError::service_unavailable("FreeBuff START timed out")
                            .with_code("freebuff_start_timeout")
                            .with_provider("freebuff_compatible"),
                    )
                });
            if let Err(error) = &result {
                // Without a returned ID the protocol cannot confirm or compensate a remote START.
                tracing::warn!(code = ?error.code, "freebuff START failed; remote state unresolved");
            }
            let run_id = result?;
            let lifetime = RunLifetime::tracked(client, config, run_id.clone(), permit);
            // If no caller receives this output, its lifetime drops and schedules bounded FINISH.
            Ok((run_id, lifetime))
        })
        .await
        .map_err(|_| {
            GatewayError::server_error("FreeBuff START task failed to complete")
                .with_code("freebuff_start_task_failed")
                .with_provider("freebuff_compatible")
        })?
}
