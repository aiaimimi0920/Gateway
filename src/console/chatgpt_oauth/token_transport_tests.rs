use super::send_token_request;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn fixture(reply: Option<&'static [u8]>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = [0; 4096];
        let length = stream.read(&mut buffer).await.unwrap();
        assert!(String::from_utf8_lossy(&buffer[..length]).starts_with("POST /"));
        if let Some(reply) = reply {
            stream.write_all(reply).await.unwrap();
        }
        stream.shutdown().await.unwrap();
    });
    (url, task)
}

#[tokio::test]
async fn retries_connect_failure_before_delivering_the_code() {
    let client = rquest::Client::builder().no_proxy().build().unwrap();
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed_url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let (url, server) = fixture(Some(
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
    ))
    .await;
    let attempts = AtomicUsize::new(0);
    let response = send_token_request(|| {
        let target = if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            &closed_url
        } else {
            &url
        };
        client.post(target).body("test-code").send()
    })
    .await
    .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    server.await.unwrap();
}

#[tokio::test]
async fn never_replays_a_code_after_http_rejection_or_connection_close() {
    let client = rquest::Client::builder().no_proxy().build().unwrap();
    for reply in [
        Some(
            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .as_slice(),
        ),
        None,
    ] {
        let (url, server) = fixture(reply).await;
        let attempts = AtomicUsize::new(0);
        let result = send_token_request(|| {
            attempts.fetch_add(1, Ordering::SeqCst);
            client.post(&url).body("test-code").send()
        })
        .await;
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        match reply {
            Some(_) => assert_eq!(result.unwrap().status(), 400),
            None => assert_eq!(
                result.err().unwrap().code.as_deref(),
                Some("chatgpt_oauth_transport_failed")
            ),
        }
        server.await.unwrap();
    }
}

#[tokio::test]
async fn stops_after_two_connect_failures_without_exposing_request_material() {
    let client = rquest::Client::builder().no_proxy().build().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let attempts = AtomicUsize::new(0);
    let error = send_token_request(|| {
        attempts.fetch_add(1, Ordering::SeqCst);
        client.post(&url).body("private-code").send()
    })
    .await
    .err()
    .unwrap();
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert!(!error.message.contains("private-code"));
    assert_eq!(
        error.code.as_deref(),
        Some("chatgpt_oauth_transport_failed")
    );
}
