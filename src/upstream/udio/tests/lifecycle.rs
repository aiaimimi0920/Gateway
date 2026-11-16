//! Real Node descendant probe, independent of browser availability.
use super::*;
use tokio::io::AsyncReadExt;

async fn tree_fixture(
    timeout: Duration,
) -> (
    tokio::task::JoinHandle<
        Result<crate::upstream::browser_worker_types::UdioBrowserWorkerSuccess, GatewayError>,
    >,
    tokio::net::TcpStream,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut input = launch("tree");
    input.stdin_json = serde_json::to_vec(&json!({"mode":"tree",
        "probe":{"port":listener.local_addr().unwrap().port()}}))
    .unwrap();
    let worker = tokio::spawn(browser_worker::run(input, timeout));
    let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut ready = [0; 6];
    tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut ready))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&ready, b"ready\n");
    (worker, stream)
}
async fn assert_tree_gone(mut stream: tokio::net::TcpStream) {
    let mut byte = [0];
    let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut byte))
        .await
        .expect("owned descendant must release its probe");
    assert!(
        matches!(read, Ok(0))
            || read.is_err_and(|e| e.kind() == std::io::ErrorKind::ConnectionReset)
    );
}
#[tokio::test]
async fn udio_worker_timeout_reaps_owned_descendant() {
    let (worker, stream) = tree_fixture(Duration::from_secs(3)).await;
    let error = worker.await.unwrap().unwrap_err();
    assert_eq!(error.http_status, Some(504));
    assert_error(error, "udio_browser_worker_timeout");
    assert_tree_gone(stream).await;
}
#[tokio::test]
async fn udio_worker_caller_drop_reaps_owned_descendant() {
    let (worker, stream) = tree_fixture(Duration::from_secs(30)).await;
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    assert_tree_gone(stream).await;
}
