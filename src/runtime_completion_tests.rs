use super::*;
use futures::FutureExt;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::oneshot;

async fn wait_for_cleanup(server: anyhow::Result<()>) -> anyhow::Result<()> {
    let entered = AtomicBool::new(false);
    let (release, wait) = oneshot::channel();
    let cleanup = async {
        entered.store(true, Ordering::Release);
        wait.await.unwrap();
        Ok(())
    };
    let mut completion = Box::pin(complete_after_freebuff_cleanup(server, cleanup));
    assert!(completion.as_mut().now_or_never().is_none());
    assert!(entered.load(Ordering::Acquire));
    release.send(()).unwrap();
    completion.await
}

#[tokio::test]
async fn successful_server_exit_waits_for_cleanup() {
    wait_for_cleanup(Ok(())).await.unwrap();
}

#[tokio::test]
async fn failed_server_exit_waits_for_cleanup_and_preserves_server_error() {
    let error = wait_for_cleanup(Err(anyhow::anyhow!("server_fixture")))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "server_fixture");
}

fn cleanup_error() -> GatewayError {
    GatewayError::service_unavailable("cleanup fixture").with_code("freebuff_run_shutdown_timeout")
}

#[tokio::test]
async fn cleanup_failure_after_server_success_is_returned() {
    let error = complete_after_freebuff_cleanup(Ok(()), async { Err(cleanup_error()) })
        .await
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<GatewayError>()
            .unwrap()
            .code
            .as_deref(),
        Some("freebuff_run_shutdown_timeout")
    );
}

#[tokio::test]
async fn dual_failure_still_runs_cleanup_and_preserves_server_error() {
    let entered = AtomicBool::new(false);
    let error = complete_after_freebuff_cleanup(Err(anyhow::anyhow!("server_fixture")), async {
        entered.store(true, Ordering::Release);
        Err(cleanup_error())
    })
    .await
    .unwrap_err();
    assert!(entered.load(Ordering::Acquire));
    assert_eq!(error.to_string(), "server_fixture");
}
