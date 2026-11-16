//! Explicit Node fixture launch: no test-wide env mutation, platform calls or credentials.
use super::super::browser_worker;
use crate::{error::FallbackHint, upstream::PreparedProducerBrowserWorkerLaunch};
use serde_json::json;
use std::{path::PathBuf, time::Duration};

fn launch(mode: &str) -> PreparedProducerBrowserWorkerLaunch {
    PreparedProducerBrowserWorkerLaunch {
        node_bin: "node".into(),
        script_path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/tests/fixtures/suno-worker-process.mjs"),
        stdin_json: serde_json::to_vec(&json!({"mode": mode})).unwrap(),
    }
}
fn assert_error(error: crate::error::GatewayError, code: &str) {
    assert_eq!(error.code.as_deref(), Some(code));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    assert!(!error.message.contains("synthetic-private-token"));
}
#[tokio::test]
async fn suno_worker_rejects_nonzero_even_with_success_json() {
    let error = browser_worker::run(launch("nonzero"), Duration::from_secs(5))
        .await
        .expect_err("nonzero exit must not report success");
    assert_error(error, "suno_browser_worker_nonzero_exit");
}
#[tokio::test]
async fn suno_worker_enforces_both_output_budgets() {
    for stream in ["stdout", "stderr"] {
        let error = browser_worker::run(launch(stream), Duration::from_secs(5))
            .await
            .expect_err("oversized metadata must fail");
        assert_error(error, "suno_browser_worker_output_too_large");
    }
}
#[tokio::test]
async fn suno_worker_errors_abort_generation_replay() {
    let error = browser_worker::run(launch("error"), Duration::from_secs(5))
        .await
        .unwrap_err();
    assert_error(error, "suno_fixture_error");
}

#[tokio::test]
async fn suno_worker_success_preserves_clip_identity() {
    let result = browser_worker::run(launch("success"), Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(result.clips[0].id, "fixture");
    assert!(result.completed);
}
#[tokio::test]
async fn suno_worker_invalid_json_omits_raw_output() {
    let error = browser_worker::run(launch("invalid"), Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(!error.message.contains("private invalid"));
    assert_error(error, "suno_browser_worker_output_parse_failed");
}
#[tokio::test]
async fn suno_worker_checks_input_before_spawn() {
    let mut input = launch("success");
    input.node_bin = "must-not-launch-fixture".into();
    input.stdin_json = vec![b'x'; 1024 * 1024 + 1];
    assert_error(
        browser_worker::run(input, Duration::from_secs(5))
            .await
            .unwrap_err(),
        "suno_browser_worker_input_too_large",
    );
}
#[tokio::test]
async fn suno_worker_spawn_failure_has_only_suno_identity() {
    let mut input = launch("success");
    input.node_bin = "must-not-launch-fixture".into();
    assert_error(
        browser_worker::run(input, Duration::from_secs(5))
            .await
            .unwrap_err(),
        "suno_browser_worker_spawn_failed",
    );
}

async fn tree_fixture(
    timeout: Duration,
) -> (
    tokio::task::JoinHandle<
        Result<
            crate::upstream::browser_worker_types::SunoBrowserWorkerSuccess,
            crate::error::GatewayError,
        >,
    >,
    tokio::net::TcpStream,
) {
    use tokio::io::AsyncReadExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut input = launch("tree");
    input.stdin_json = serde_json::to_vec(&json!({
        "mode":"tree", "probe":{"port":listener.local_addr().unwrap().port()}
    }))
    .unwrap();
    let worker = tokio::spawn(browser_worker::run(input, timeout));
    let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut ready = [0; 6];
    stream.read_exact(&mut ready).await.unwrap();
    assert_eq!(&ready, b"ready\n");
    (worker, stream)
}
async fn assert_tree_gone(mut stream: tokio::net::TcpStream) {
    use tokio::io::AsyncReadExt;
    let mut byte = [0];
    let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut byte))
        .await
        .expect("owned descendant must exit after worker cancellation");
    match read {
        Ok(0) => {}
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        other => panic!("descendant did not close its probe: {other:?}"),
    }
}
#[tokio::test]
async fn suno_worker_timeout_reaps_owned_descendant() {
    let (worker, stream) = tree_fixture(Duration::from_secs(3)).await;
    let error = worker.await.unwrap().unwrap_err();
    assert_eq!(error.http_status, Some(504));
    assert_error(error, "suno_browser_worker_timeout");
    assert_tree_gone(stream).await;
}
#[tokio::test]
async fn suno_worker_caller_drop_reaps_owned_descendant() {
    let (worker, stream) = tree_fixture(Duration::from_secs(30)).await;
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    assert_tree_gone(stream).await;
}
