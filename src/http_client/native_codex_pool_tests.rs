//! Pool ownership and retirement proofs use only bounded loopback transports.
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn populated(connection: &NativeCodexConnection) -> usize {
    connection.0.lock().clients.iter().flatten().count()
}

async fn transport_failure(client: &reqwest::Client) -> GatewayError {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        // EOF after request delivery: rebuilding must not turn this into replay permission.
    });
    let error = client.post(url).body("fixture").send().await.unwrap_err();
    server.await.unwrap();
    crate::error::classify_native_network_error(error, "fixture")
}

#[tokio::test]
async fn native_codex_pool_is_lazy_shared_and_policy_isolated() {
    let connection = NativeCodexConnection::default();
    let shared = connection.clone();
    let independent = NativeCodexConnection::default();
    assert!(Arc::ptr_eq(&connection.0, &shared.0));
    assert!(!Arc::ptr_eq(&connection.0, &independent.0));
    assert_eq!(populated(&connection), 0);
    connection.client(Duration::from_secs(2));
    shared.client(Duration::from_secs(2));
    assert_eq!(populated(&connection), 1);
    connection.session_client(Duration::from_secs(2));
    crate::request_wait::transport::within(true, async {
        connection.client(Duration::from_secs(2));
        connection.session_client(Duration::from_secs(2));
    })
    .await;
    assert_eq!(populated(&connection), 4);
    assert_eq!(populated(&independent), 0);
    let weak = Arc::downgrade(&connection.0);
    drop(connection);
    assert!(weak.upgrade().is_some());
    drop(shared);
    assert!(
        weak.upgrade().is_none(),
        "pool outlived its accepted connection owner"
    );
}

#[tokio::test]
async fn native_codex_pool_retires_only_matching_trusted_failure_generation() {
    let connection = NativeCodexConnection::default();
    let (client, generation) = connection.client(Duration::from_secs(2));
    let error = transport_failure(&client).await;
    assert!(error.native_transport.is_some());
    assert!(!error.retryable);
    assert!(!error.native_connect_retry_allowed());
    assert!(connection.recover_after_send(generation, &error));
    assert_eq!(populated(&connection), 0);
    let (_, next) = connection.client(Duration::from_secs(2));
    assert_eq!(next, generation + 1);
    assert!(!connection.recover_after_send(generation, &error));
    assert_eq!(populated(&connection), 1);
    let mut spoofed = GatewayError::service_unavailable(error.message.clone());
    spoofed.code = error.code.clone();
    assert!(!connection.recover_after_send(next, &spoofed));
    assert_eq!(connection.generation(), next);
    assert_eq!(populated(&connection), 1);
}

#[tokio::test]
async fn native_codex_pool_retirement_does_not_cancel_inflight_response() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let connection = NativeCodexConnection::default();
        let (client, generation) = connection.client(Duration::from_secs(3));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let release = Arc::new(tokio::sync::Notify::new());
        let finish = release.clone();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            socket.read(&mut request).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            finish.notified().await;
            socket.write_all(b"x").await.unwrap();
        });
        let response = client.get(url).send().await.unwrap();
        let error = transport_failure(&client).await;
        assert!(connection.recover_after_send(generation, &error));
        let (_, next) = connection.client(Duration::from_secs(3));
        assert_eq!(next, generation + 1);
        release.notify_one();
        assert_eq!(response.bytes().await.unwrap().as_ref(), b"x");
        server.await.unwrap();
    })
    .await
    .unwrap();
}
