//! Deadline, decoded-byte admission, redirect and cancellation operate on real loopback HTTP.
use super::{transport, transport_fixture::*, *};
use std::time::Duration;

#[tokio::test]
async fn accio_grants_never_follow_redirects_to_another_origin() {
    let destination = Mock::new(Reply::default()).await;
    for status in [301, 302, 303, 307, 308] {
        let source = Mock::new(Reply {
            status,
            location: Some(destination.endpoint.clone()),
            ..Default::default()
        })
        .await;
        let grant = RefreshGrant::new("private-access", "private-refresh", None).unwrap();
        assert_eq!(
            transport::test_refresh(grant, &http(), &source.endpoint)
                .await
                .unwrap_err(),
            OAuthError::RedirectRejected
        );
        assert_eq!(source.count(), 1);
        assert_eq!(destination.count(), 0);
        source.stop().await;
    }
    destination.stop().await;
}

#[tokio::test]
async fn accio_decoded_response_budget_rejects_chunked_oversize_before_parsing() {
    let mock = Mock::new(Reply {
        body: "x".repeat(65_537),
        ..Default::default()
    })
    .await;
    let grant = RefreshGrant::new("access", "refresh", None).unwrap();
    assert_eq!(
        transport::test_refresh(grant, &http(), &mock.endpoint)
            .await
            .unwrap_err(),
        OAuthError::ResponseTooLarge
    );
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn accio_compressed_response_budget_applies_after_gzip_decoding() {
    // Python stdlib gzip.compress(b'x' * 65_537, mtime=0); only 97 bytes on the wire.
    let compressed = hex::decode(concat!(
        "1f8b080000000000020aedc1010100000082a0dbceaf2140010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00d81d82a9c01000100"
    )).unwrap();
    assert!(compressed.len() < 128);
    let mut response = format!("HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", compressed.len()).into_bytes();
    response.extend(compressed);
    assert_eq!(raw_response(response).await, OAuthError::ResponseTooLarge);
}

#[tokio::test]
async fn accio_declared_oversize_and_truncated_body_are_not_successful_material() {
    let oversize = b"HTTP/1.1 200 OK\r\nContent-Length: 65537\r\nConnection: close\r\n\r\n";
    assert_eq!(
        raw_response(oversize.to_vec()).await,
        OAuthError::ResponseTooLarge
    );
    let truncated =
        b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\nprivate-incomplete";
    assert_eq!(
        raw_response(truncated.to_vec()).await,
        OAuthError::Transport
    );
}

async fn raw_response(response: Vec<u8>) -> OAuthError {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/grant", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        socket.write_all(&response).await.unwrap();
        socket.shutdown().await.unwrap();
    });
    let grant = RefreshGrant::new("synthetic-access", "synthetic-refresh", None).unwrap();
    let error = transport::test_refresh(grant, &http(), &endpoint)
        .await
        .unwrap_err();
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    error
}

#[tokio::test]
async fn accio_total_deadline_covers_both_sending_and_reading_without_retry() {
    for (header_delay, body_delay) in [
        (Duration::from_millis(400), Duration::ZERO),
        (Duration::ZERO, Duration::from_millis(400)),
    ] {
        let mock = Mock::new(Reply {
            header_delay,
            body_delay,
            ..Default::default()
        })
        .await;
        let start = std::time::Instant::now();
        let error = transport::test_post(&http(), &mock.endpoint, Duration::from_millis(100))
            .await
            .unwrap_err();
        assert!(matches!(error, OAuthError::Timeout | OAuthError::Transport));
        assert!(start.elapsed() < Duration::from_secs(3));
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn accio_cancel_drops_inflight_body_connection_and_does_not_spawn_a_retrier() {
    let mock = Mock::new(Reply {
        body_delay: Duration::from_secs(300),
        ..Default::default()
    })
    .await;
    let endpoint = mock.endpoint.clone();
    let grant = RefreshGrant::new("access", "refresh", None).unwrap();
    let task =
        tokio::spawn(async move { transport::test_refresh(grant, &http(), &endpoint).await });
    mock.wait_called().await;
    // Let the response stream be handed to the client before cancelling the owner future.
    tokio::time::sleep(Duration::from_millis(50)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    mock.wait_body_dropped().await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}
