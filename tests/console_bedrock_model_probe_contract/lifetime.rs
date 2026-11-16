//! Native metadata is not transport EOF. Cancellation drops the actual source, not a proxy task.
use super::{fixture, probe, support::*};
use axum::http::{Method, StatusCode};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Notify,
};

async fn source(hold: bool) -> (String, Arc<Notify>, tokio::task::JoinHandle<bool>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let entered = Arc::new(Notify::new());
    let sent = entered.clone();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0; 2048];
            let n = socket.read(&mut chunk).await.unwrap();
            assert!(n > 0 && bytes.len() + n <= 32768);
            bytes.extend_from_slice(&chunk[..n]);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                let length = header
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if bytes.len() == end + 4 + length {
                    break;
                }
            }
        }
        let body = fixture::sequence("OK", "end_turn").concat();
        // Advertise one more byte. A clean-looking terminal metadata cannot hide a broken body.
        let header=format!("HTTP/1.1 200 OK\r\nContent-Type: application/vnd.amazon.eventstream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()+1);
        socket.write_all(header.as_bytes()).await.unwrap();
        socket.write_all(&body).await.unwrap();
        socket.flush().await.unwrap();
        sent.notify_one();
        if !hold {
            return true;
        }
        let mut byte = [0];
        match tokio::time::timeout(Duration::from_secs(4), socket.read(&mut byte)).await {
            Ok(Ok(0)) | Ok(Err(_)) => true,
            _ => false,
        }
    });
    (base, entered, task)
}

async fn fixture(base: &str) -> ConsoleStateFixture {
    let unused = fixture::Upstream::start().await;
    let mut document = unused.document(&["good"]);
    document.providers[0].base_url = base.into();
    unused.finish().await;
    ConsoleStateFixture::new(document, false)
}

#[tokio::test]
async fn bedrock_probe_rejects_transport_error_after_complete_metadata() {
    let (base, _, server) = source(false).await;
    let f = fixture(&base).await;
    let grant = grant_console_secret_access(&f.state).await;
    let response = tokio::time::timeout(Duration::from_secs(5), probe(&f, Some(&grant), "good"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(body["result"]["results"][0]["status"], "failed", "{body}");
    assert!(body["result"]["results"][0]["message"]
        .as_str()
        .unwrap()
        .contains("interrupted"));
    assert!(server.await.unwrap());
}

#[tokio::test]
async fn dropping_bedrock_probe_closes_held_native_response() {
    let (base, entered, server) = source(true).await;
    let f = fixture(&base).await;
    let grant = grant_console_secret_access(&f.state).await;
    let state = f.state.clone();
    let call = tokio::spawn(async move {
        send_console_json(
            &state,
            Method::POST,
            "/v1/internal/gateway/console/providers/bedrock-test/probe",
            json!({"prompt":"Reply OK","model":"probe-alias","credentialIds":["good"]}),
            Some(&grant),
            None,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    assert!(tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap());
    // This proves connection cleanup, not durable audit cancellation finalization.
    assert_eq!(
        f.state
            .concurrency_registry
            .get_or_create("bedrock-test")
            .snapshot()
            .active_count,
        0
    );
}
