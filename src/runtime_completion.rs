use crate::error::GatewayError;
use std::future::Future;

pub(super) async fn complete_after_freebuff_cleanup(
    server_result: anyhow::Result<()>,
    cleanup: impl Future<Output = Result<(), GatewayError>>,
) -> anyhow::Result<()> {
    // Cleanup is awaited even when HTTP serving failed; preserve that original failure.
    let cleanup_result = cleanup.await;
    if let Err(error) = &cleanup_result {
        tracing::warn!(code = ?error.code, "FreeBuff runtime shutdown incomplete");
    }
    combine(server_result, cleanup_result.map_err(Into::into))
}

pub(super) async fn complete_after_runtime_cleanup(
    server_result: anyhow::Result<()>,
    cleanup: impl Future<Output = anyhow::Result<()>>,
) -> anyhow::Result<()> {
    let cleanup_result = cleanup.await;
    if cleanup_result.is_err() {
        tracing::warn!("Gateway background runtime shutdown incomplete");
    }
    combine(server_result, cleanup_result)
}

fn combine(
    server_result: anyhow::Result<()>,
    cleanup_result: anyhow::Result<()>,
) -> anyhow::Result<()> {
    // Report the original serving error, without skipping or hiding cleanup execution.
    server_result?;
    cleanup_result
}

#[cfg(test)]
#[path = "runtime_completion_tests.rs"]
mod tests;
