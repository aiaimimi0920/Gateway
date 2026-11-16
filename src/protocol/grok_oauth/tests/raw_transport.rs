//! Raw HTTP framing and resource lifetime contracts, independent of the OAuth state fixtures.
use super::*;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn read_request(socket: &mut tokio::net::TcpStream) {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut bytes = Vec::new();
        let mut chunk = [0; 4096];
        loop {
            let n = socket.read(&mut chunk).await.unwrap();
            assert_ne!(n, 0);
            bytes.extend_from_slice(&chunk[..n]);
            assert!(bytes.len() <= 65_536);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = std::str::from_utf8(&bytes[..end]).unwrap();
                let length = head
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
}
async fn raw_reply(reply: Vec<u8>) -> (OAuthClient, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = client().test_issuer(&format!("http://{}", listener.local_addr().unwrap()));
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_request(&mut socket).await;
        // A receiver intentionally rejecting oversized data may close before this write completes.
        let _ = socket.write_all(&reply).await;
    });
    (client, task)
}
#[tokio::test]
async fn redirects_never_deliver_material_to_location() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let reply=format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",target.local_addr().unwrap());
    let (client, task) = raw_reply(reply.into_bytes()).await;
    assert_eq!(
        client
            .refresh(&wire::http(), "private-refresh")
            .await
            .unwrap_err(),
        OAuthError::RedirectRejected
    );
    task.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), target.accept())
            .await
            .is_err()
    );
}
#[tokio::test]
async fn response_parser_rejects_duplicate_keys_bad_utf8_truncation_and_chunked_overflow() {
    for body in [
        b"{\"access_token\":\"first\",\"access_token\":\"last\"}".to_vec(),
        b"{\"access_token\":\"\xff\"}".to_vec(),
        b"{\"access_token\":".to_vec(),
        b"{} {}".to_vec(),
    ] {
        let mut reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        reply.extend(body);
        let (client, task) = raw_reply(reply).await;
        assert_eq!(
            client
                .refresh(&wire::http(), "private-refresh")
                .await
                .unwrap_err(),
            OAuthError::InvalidResponse
        );
        task.await.unwrap();
    }
    let mut reply =
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n10001\r\n"
            .to_vec();
    reply.extend(vec![b'x'; 65537]);
    reply.extend(b"\r\n0\r\n\r\n");
    let (client, task) = raw_reply(reply).await;
    assert_eq!(
        client
            .refresh(&wire::http(), "private-refresh")
            .await
            .unwrap_err(),
        OAuthError::ResponseTooLarge
    );
    task.await.unwrap();
}
async fn stalled_body() -> (
    OAuthClient,
    tokio::sync::oneshot::Receiver<()>,
    tokio::task::JoinHandle<usize>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = client().test_issuer(&format!("http://{}", listener.local_addr().unwrap()));
    let (sent, ready) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_request(&mut socket).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\n{\r\n")
            .await
            .unwrap();
        sent.send(()).unwrap();
        let mut bytes = [0; 4096];
        tokio::time::timeout(Duration::from_secs(20), socket.read(&mut bytes))
            .await
            .unwrap()
            .unwrap()
    });
    (client, ready, server)
}
#[tokio::test]
async fn dropping_inflight_exchange_closes_the_owned_connection() {
    let (client, ready, server) = stalled_body().await;
    let task = tokio::spawn(async move { client.refresh(&wire::http(), "private-refresh").await });
    ready.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(server.await.unwrap(), 0);
}
#[tokio::test]
async fn own_deadline_includes_a_stalled_response_body() {
    let (client, ready, server) = stalled_body().await;
    let started = Instant::now();
    let task = tokio::spawn(async move { client.refresh(&wire::http(), "private-refresh").await });
    ready.await.unwrap();
    let error = task.await.unwrap().unwrap_err();
    assert!(matches!(error, OAuthError::Timeout | OAuthError::Transport));
    assert!(started.elapsed() >= Duration::from_secs(14));
    assert!(started.elapsed() < Duration::from_secs(20));
    assert_eq!(server.await.unwrap(), 0);
}
