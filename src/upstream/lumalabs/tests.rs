//! Explicit launch fixtures avoid changing process-wide env or reading real sessions.
use super::browser_worker;
use crate::{
    error::{FallbackHint, GatewayError},
    upstream::lumalabs_runtime_helpers::PreparedLumalabsBrowserWorkerLaunch,
};
use serde_json::json;
use std::{path::PathBuf, time::Duration};

fn launch(mode: &str) -> PreparedLumalabsBrowserWorkerLaunch {
    PreparedLumalabsBrowserWorkerLaunch {
        node_bin: "node".into(),
        script_path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/tests/fixtures/lumalabs-worker-process.mjs"),
        stdin_json: serde_json::to_vec(&json!({"mode":mode})).unwrap(),
    }
}
pub(super) async fn run(mode: &str) -> Result<super::task_result::TaskResult, GatewayError> {
    browser_worker::run(launch(mode), Duration::from_secs(5), "lumalabs_compatible").await
}
pub(super) fn assert_error(error: GatewayError, code: &str) {
    assert_eq!(error.code.as_deref(), Some(code));
    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    assert!(!error.message.contains("synthetic-private"));
}
#[tokio::test]
async fn luma_web_red_worker_nonzero_success_is_not_success() {
    assert_error(
        run("nonzero").await.unwrap_err(),
        "lumalabs_browser_worker_nonzero_exit",
    );
}
#[tokio::test]
async fn luma_web_red_worker_output_budgets_are_metadata_sized() {
    for mode in ["stdout", "stderr"] {
        assert_error(
            run(mode).await.unwrap_err(),
            "lumalabs_browser_worker_output_too_large",
        );
    }
}
#[tokio::test]
async fn luma_web_red_worker_invalid_json_does_not_echo_output() {
    assert_error(
        run("invalid").await.unwrap_err(),
        "lumalabs_browser_worker_output_parse_failed",
    );
    assert_error(
        run("utf8").await.unwrap_err(),
        "lumalabs_browser_worker_output_parse_failed",
    );
}
#[tokio::test]
async fn luma_web_red_worker_business_failure_cannot_replay() {
    assert_error(run("error").await.unwrap_err(), "lumalabs_fixture_error");
}
#[tokio::test]
async fn luma_web_worker_success_preserves_signed_url() {
    assert_eq!(
        run("success").await.unwrap().signed_url,
        "https://asset.fixture.invalid/image.png"
    );
}
#[tokio::test]
async fn luma_web_worker_input_is_bounded_before_spawn_and_spawn_errors_are_projected() {
    let mut input = launch("success");
    input.node_bin = "nonexistent-luma-fixture".into();
    input.stdin_json = vec![b'x'; 1024 * 1024 + 1];
    assert_error(
        browser_worker::run(input, Duration::from_secs(5), "lumalabs_compatible")
            .await
            .unwrap_err(),
        "lumalabs_browser_worker_input_too_large",
    );
    let mut input = launch("success");
    input.node_bin = "nonexistent-luma-fixture".into();
    assert_error(
        browser_worker::run(input, Duration::from_secs(5), "lumalabs_compatible")
            .await
            .unwrap_err(),
        "lumalabs_browser_worker_spawn_failed",
    );
}

async fn tree_fixture(
    timeout: Duration,
) -> (
    tokio::task::JoinHandle<Result<super::task_result::TaskResult, GatewayError>>,
    tokio::net::TcpStream,
) {
    use tokio::io::AsyncReadExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut input = launch("tree");
    input.stdin_json = serde_json::to_vec(
        &json!({"mode":"tree", "probe":{"port":listener.local_addr().unwrap().port()}}),
    )
    .unwrap();
    let worker = tokio::spawn(browser_worker::run(input, timeout, "lumalabs_compatible"));
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
        .expect("owned descendant must terminate");
    match read {
        Ok(0) => {}
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        other => panic!("descendant still owns its TCP probe: {other:?}"),
    }
}
#[tokio::test]
async fn luma_web_worker_timeout_reaps_owned_descendant() {
    let (worker, stream) = tree_fixture(Duration::from_secs(3)).await;
    let error = worker.await.unwrap().unwrap_err();
    assert_eq!(error.http_status, Some(504));
    assert_error(error, "lumalabs_browser_worker_timeout");
    assert_tree_gone(stream).await;
}
#[tokio::test]
async fn luma_web_worker_caller_drop_reaps_owned_descendant() {
    let (worker, stream) = tree_fixture(Duration::from_secs(30)).await;
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    assert_tree_gone(stream).await;
}
