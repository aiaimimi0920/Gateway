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
    server_result?;
    cleanup_result?;
    Ok(())
}

#[cfg(test)]
#[path = "runtime_completion_tests.rs"]
mod tests;
