//! 真实 loopback 传输错误必须保留 Gateway 决策和脱敏边界；不调用提供方。
use futures::TryStreamExt;
use neuro_gateway::error::{classify_network_error, ErrorKind, FallbackHint};
use rquest::Client;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const DEADLINE: Duration = Duration::from_secs(3);
const SECRET: &str = "classification-fixture-secret";

fn client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(DEADLINE)
        .build()
        .unwrap()
}

async fn headers(socket: &mut TcpStream) {
    let mut data = Vec::new();
    while !data.ends_with(b"\r\n\r\n") {
        assert!(data.len() < 8192, "fixture headers must remain bounded");
        let mut byte = [0];
        socket.read_exact(&mut byte).await.unwrap();
        data.push(byte[0]);
    }
}

fn classified(error: &rquest::Error, kind: ErrorKind) {
    let result = classify_network_error(error, Some("fixture-provider"));
    assert_eq!(result.kind, kind);
    assert_eq!(result.provider_name.as_deref(), Some("fixture-provider"));
    assert!(!result.message.contains(SECRET));
    assert!(result.message.chars().count() <= 512);
    match kind {
        ErrorKind::Network => {
            assert!(result.retryable);
            assert!(matches!(
                result.fallback_hint,
                FallbackHint::FallbackProvider { .. }
            ));
        }
        ErrorKind::Timeout => {
            assert!(result.retryable);
            assert_eq!(result.http_status, Some(504));
            assert!(matches!(
                result.fallback_hint,
                FallbackHint::Retry { delay_ms: 1000, .. }
            ));
        }
        ErrorKind::Unknown => assert!(!result.retryable),
        _ => panic!("unexpected transport classification"),
    }
}

#[tokio::test]
async fn refused_connection_preserves_network_fallback_and_query_redaction() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let error = client()
        .get(format!("http://{address}/?access_token={SECRET}"))
        .send()
        .await
        .unwrap_err();
    assert!(error.is_connect());
    assert!(!error.is_timeout());
    classified(&error, ErrorKind::Network);
}

#[tokio::test]
async fn response_header_deadline_remains_timeout_not_connect_retry() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        headers(&mut socket).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
    };
    let request = async {
        let error = client()
            .get(format!("http://{address}/?access_token={SECRET}"))
            .timeout(Duration::from_millis(150))
            .send()
            .await
            .unwrap_err();
        assert!(error.is_timeout());
        assert!(!error.is_connect());
        classified(&error, ErrorKind::Timeout);
    };
    tokio::time::timeout(DEADLINE, async { tokio::join!(server, request) })
        .await
        .unwrap();
}

async fn body_failure(stalled: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        headers(&mut socket).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nx")
            .await
            .unwrap();
        if stalled {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    };
    let request = async {
        let response = client()
            .get(format!("http://{address}/?access_token={SECRET}"))
            .timeout(Duration::from_millis(150))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        let error = response
            .bytes_stream()
            .try_collect::<Vec<_>>()
            .await
            .unwrap_err();
        // 已收到响应的 body 错误不满足 OAuth 的 connect-only 重试条件。
        assert!(!error.is_connect());
        assert_eq!(error.is_timeout(), stalled);
        classified(
            &error,
            if stalled {
                ErrorKind::Timeout
            } else {
                ErrorKind::Unknown
            },
        );
    };
    tokio::time::timeout(DEADLINE, async { tokio::join!(server, request) })
        .await
        .unwrap();
}

#[tokio::test]
async fn truncated_body_preserves_non_connect_classification() {
    body_failure(false).await;
}

#[tokio::test]
async fn stalled_body_preserves_timeout_and_non_connect_classification() {
    body_failure(true).await;
}
