//! Real local TCP only; raw peers share the owner future and fixture server tasks are joined.
use super::{
    authorization_tests::{attempt, callback},
    material_tests::tokens,
    transport_fixture::{http, Mock, Reply},
    transport_test_support::{test_budget, test_endpoint},
    *,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn grant() -> RefreshGrant {
    RefreshGrant::new(
        OAuthClient::new("explicit-client", None).unwrap(),
        "refresh-private",
        None,
    )
    .unwrap()
}
async fn raw_response(response: Vec<u8>) -> OAuthError {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/grant", listener.local_addr().unwrap());
    let peer = async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        loop {
            let mut chunk = [0; 1024];
            let count = socket.read(&mut chunk).await.unwrap();
            assert!(count > 0 && request.len() + count <= 16_384);
            request.extend_from_slice(&chunk[..count]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = std::str::from_utf8(&request[..end]).unwrap();
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        socket.write_all(&response).await.unwrap();
        socket.shutdown().await.unwrap();
    };
    let request = async {
        test_endpoint(endpoint, grant().execute(&http()))
            .await
            .unwrap_err()
    };
    let (_, error) = tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(peer, request)
    })
    .await
    .unwrap();
    error
}

#[tokio::test]
async fn vertex_oauth_transport_size_is_cumulative_decoded_not_compressed_or_declared() {
    // Same synthetic stdlib gzip fixture as existing Accio and Kiro transport contracts.
    let compressed = hex::decode("1f8b080000000000020aedc1010100000082a0dbceaf2140010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00d81d82a9c01000100").unwrap();
    let mut response = format!("HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", compressed.len()).into_bytes();
    response.extend(compressed);
    assert_eq!(raw_response(response).await, OAuthError::ResponseTooLarge);
    let mut response =
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_vec();
    for size in [40_000, 25_537] {
        response.extend(format!("{size:x}\r\n").as_bytes());
        response.extend(vec![b'x'; size]);
        response.extend(b"\r\n");
    }
    response.extend(b"0\r\n\r\n");
    assert_eq!(raw_response(response).await, OAuthError::ResponseTooLarge);
    assert_eq!(
        raw_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 65537\r\nConnection: close\r\n\r\n".to_vec()
        )
        .await,
        OAuthError::ResponseTooLarge
    );
    assert_eq!(raw_response(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\nprivate-incomplete".to_vec()).await, OAuthError::Transport);
}

#[tokio::test]
async fn vertex_oauth_transport_total_budget_covers_headers_plus_body() {
    for (headers, body) in [(300, 0), (0, 300), (120, 120)] {
        let mock = Mock::new(Reply {
            header_delay: Duration::from_millis(headers),
            body_delay: Duration::from_millis(body),
            body: tokens().to_string(),
            ..Reply::default()
        })
        .await;
        let error = test_budget(
            Duration::from_millis(200),
            test_endpoint(mock.endpoint.clone(), grant().execute(&http())),
        )
        .await
        .unwrap_err();
        assert_eq!(error, OAuthError::Timeout);
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn vertex_oauth_transport_expired_owned_grants_never_send() {
    let mock = Mock::new(Reply {
        body: tokens().to_string(),
        ..Reply::default()
    })
    .await;
    let auth = attempt();
    let callback = callback(&auth);
    let mut code = auth.finish(&callback).unwrap();
    code.deadline = std::time::Instant::now();
    assert_eq!(
        test_endpoint(mock.endpoint.clone(), code.exchange(&http()))
            .await
            .unwrap_err(),
        OAuthError::Expired
    );
    let mut refresh = grant();
    refresh.refresh_expires_at = Some(time::OffsetDateTime::now_utc());
    assert_eq!(
        test_endpoint(mock.endpoint.clone(), refresh.execute(&http()))
            .await
            .unwrap_err(),
        OAuthError::Expired
    );
    assert_eq!(mock.count(), 0);
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_transport_code_requires_new_refresh_and_rotation_can_only_shorten_deadline() {
    let mut value = tokens();
    value.as_object_mut().unwrap().remove("refresh_token");
    let mock = Mock::new(Reply {
        body: value.to_string(),
        ..Reply::default()
    })
    .await;
    let auth = attempt();
    let callback = callback(&auth);
    let code = auth.finish(&callback).unwrap();
    assert_eq!(
        test_endpoint(mock.endpoint.clone(), code.exchange(&http()))
            .await
            .unwrap_err(),
        OAuthError::MissingRefreshToken
    );
    mock.stop().await;
    for seconds in [300, 9000] {
        let mut value = tokens();
        value["refresh_token_expires_in"] = serde_json::json!(seconds);
        value["refresh_token"] = serde_json::json!("rotated-private");
        let mock = Mock::new(Reply {
            body: value.to_string(),
            ..Reply::default()
        })
        .await;
        let old = time::OffsetDateTime::now_utc() + time::Duration::seconds(700);
        let mut refresh = grant();
        refresh.refresh_expires_at = Some(old);
        let material = test_endpoint(mock.endpoint.clone(), refresh.execute(&http()))
            .await
            .unwrap();
        assert_eq!(material.refresh_token(), Some("rotated-private"));
        let expiry = material.refresh_expires_at().unwrap();
        if seconds == 300 {
            assert!(expiry < old);
        } else {
            assert_eq!(expiry, old);
        }
        mock.stop().await;
    }
}
