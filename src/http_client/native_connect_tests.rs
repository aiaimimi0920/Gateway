//! Real loopback exchanges distinguish connector failures from uncertain delivery.
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn failed_connect(redirect_evidence: bool) -> crate::error::GatewayError {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let error = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .post(format!(
            "http://{address}/?api_key=secret-query-certificate407"
        ))
        .send()
        .await
        .unwrap_err();
    crate::error::classify_native_send_error(error, "fixture", redirect_evidence)
}

#[tokio::test]
async fn connect_evidence_is_trusted_and_diagnostics_do_not_leak_urls() {
    let error = failed_connect(true).await;
    assert!(error.native_connect_retry_allowed());
    assert_eq!(
        error.native_transport.unwrap().category(),
        "connection_refused"
    );
    assert!(!error.message.contains("secret-query"));
    assert!(!error.message.contains("127.0.0.1"));
    let error = failed_connect(false).await;
    assert!(!error.native_connect_retry_allowed());
    assert!(!error.retryable);
}

#[tokio::test]
async fn redirect_then_connector_failure_never_grants_another_origin_send() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0; 4096];
        let size = socket.read(&mut request).await.unwrap();
        assert!(String::from_utf8_lossy(&request[..size]).starts_with("POST /start"));
        // Close the listening port before returning a same-origin redirect.
        drop(listener);
        socket.write_all(b"HTTP/1.1 307 Temporary Redirect\r\nLocation: /finish\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(policy())
        .build()
        .unwrap();
    let error = send(
        client
            .post(format!("http://{address}/start"))
            .body("original"),
        "fixture",
    )
    .await
    .unwrap_err();
    task.await.unwrap();
    assert!(!error.native_connect_retry_allowed());
    assert!(error.message.contains("delivery_unknown"));
}

#[tokio::test]
async fn headers_timeout_and_sent_request_disconnect_are_not_replayed() {
    for timeout in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            if timeout {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_millis(100))
            .build()
            .unwrap();
        let error = send(
            client
                .post(format!("http://{address}/"))
                .body("tool request"),
            "fixture",
        )
        .await
        .unwrap_err();
        assert!(!error.native_connect_retry_allowed());
        task.abort();
        let _ = task.await;
    }
}

#[tokio::test]
async fn proxy_authentication_failure_is_diagnostic_not_a_transient_retry() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        socket.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .proxy(
            reqwest::Proxy::https(format!("http://user:fixture-proxy-secret@{address}")).unwrap(),
        )
        .build()
        .unwrap();
    let error = send(
        client.post("https://origin.invalid/?api_key=fixture-query-secret"),
        "fixture",
    )
    .await
    .unwrap_err();
    task.await.unwrap();
    assert_eq!(
        error.native_transport.as_ref().unwrap().category(),
        "proxy_authentication"
    );
    assert!(!error.native_connect_retry_allowed());
    assert!(!error.message.contains("fixture-proxy-secret"));
    assert!(!error.message.contains("fixture-query-secret"));
}
