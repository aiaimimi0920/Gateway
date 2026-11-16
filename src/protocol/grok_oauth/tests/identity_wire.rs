//! Exercises the public verifier with real loopback HTTP, signed tokens and joined server tasks.
use super::{identity_support::*, *};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Fixture {
    client: OAuthClient,
    requests: tokio::sync::mpsc::Receiver<String>,
    server: Option<tokio::task::JoinHandle<usize>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(task) = &self.server {
            task.abort();
        }
    }
}
impl Fixture {
    async fn start(replies: Vec<Vec<u8>>, stall: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = client().test_issuer(&format!("http://{}", listener.local_addr().unwrap()));
        let (sender, requests) = tokio::sync::mpsc::channel(4);
        let server = tokio::spawn(async move {
            let mut count = 0;
            for reply in replies {
                let (mut socket, _) =
                    tokio::time::timeout(Duration::from_secs(3), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut header = Vec::new();
                tokio::time::timeout(Duration::from_secs(3), async {
                    loop {
                        let mut buf = [0; 1024];
                        let n = socket.read(&mut buf).await.unwrap();
                        assert!(n > 0);
                        header.extend_from_slice(&buf[..n]);
                        assert!(header.len() <= 8192);
                        if header.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                })
                .await
                .unwrap();
                let _ = socket.write_all(&reply).await;
                sender
                    .send(String::from_utf8(header).unwrap())
                    .await
                    .unwrap();
                if stall {
                    let mut buf = [0; 1024];
                    return tokio::time::timeout(Duration::from_secs(15), socket.read(&mut buf))
                        .await
                        .unwrap()
                        .unwrap();
                }
                count += 1;
            }
            count
        });
        Self {
            client,
            requests,
            server: Some(server),
        }
    }
    async fn request(&mut self) -> String {
        tokio::time::timeout(Duration::from_secs(3), self.requests.recv())
            .await
            .unwrap()
            .unwrap()
    }
    async fn finish(&mut self) -> usize {
        self.server.take().unwrap().await.unwrap()
    }
}
fn reply(body: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    bytes.extend_from_slice(body);
    bytes
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[tokio::test]
async fn public_verifier_fetches_only_fixed_jwks_without_shared_auth_and_observes_rotation() {
    let first = Signer::new();
    let second = Signer::new();
    let mut fixture =
        Fixture::start(vec![reply(&first.jwks()), reply(&second.jwks())], false).await;
    let auth = authorization(&fixture.client);
    for signer in [first, second] {
        let material = tokens(Some(signer.sign(&claims(&fixture.client, &auth, now()))));
        let identity = fixture
            .client
            .verify_code_identity(&wire::http(), &auth, &material)
            .await
            .unwrap();
        assert_eq!(identity.subject(), "private-synthetic-subject");
        let request = fixture.request().await;
        assert!(request.starts_with("GET /.well-known/jwks.json HTTP/1.1\r\n"));
        let lower = request.to_ascii_lowercase();
        for forbidden in [
            "authorization:",
            "cookie:",
            "x-xai-token-auth:",
            "synthetic-access",
            "private-synthetic-subject",
        ] {
            assert!(!lower.contains(forbidden));
        }
    }
    assert_eq!(fixture.finish().await, 2);
}

#[tokio::test]
async fn public_verifier_rejects_context_rebinding_and_missing_id_before_network() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = client().test_issuer(&format!("http://{}", listener.local_addr().unwrap()));
    let auth = authorization(&client);
    assert_eq!(
        client
            .verify_code_identity(&wire::http(), &auth, &tokens(None))
            .await
            .unwrap_err(),
        OAuthError::InvalidIdentity
    );
    let material = tokens(Some(Signer::new().sign(&claims(&client, &auth, now()))));
    let changed = client
        .clone()
        .with_principal(Principal::new("Team", "other").unwrap());
    assert_eq!(
        changed
            .verify_code_identity(&wire::http(), &auth, &material)
            .await
            .unwrap_err(),
        OAuthError::InvalidInput
    );
    let material = tokens(Some("none.invalid.token".into()));
    assert_eq!(
        client
            .verify_code_identity(&wire::http(), &auth, &material)
            .await
            .unwrap_err(),
        OAuthError::InvalidIdentity
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn jwks_redirect_is_not_followed_and_does_not_forward_any_token() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let response = format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",target.local_addr().unwrap());
    let mut fixture = Fixture::start(vec![response.into_bytes()], false).await;
    let auth = authorization(&fixture.client);
    let material = tokens(Some(Signer::new().sign(&claims(
        &fixture.client,
        &auth,
        now(),
    ))));
    assert_eq!(
        fixture
            .client
            .verify_code_identity(&wire::http(), &auth, &material)
            .await
            .unwrap_err(),
        OAuthError::RedirectRejected
    );
    fixture.request().await;
    assert_eq!(fixture.finish().await, 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), target.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn malformed_and_chunked_oversized_key_sets_are_bounded() {
    let mut responses = vec![
        reply(b"{\"keys\":[],\"keys\":[]}"),
        reply(b"{\"keys\":\xff}"),
        reply(b"{} {}"),
    ];
    let mut large =
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n10001\r\n"
            .to_vec();
    large.extend(vec![b'x'; 65537]);
    large.extend_from_slice(b"\r\n0\r\n\r\n");
    responses.push(large);
    for (index, response) in responses.into_iter().enumerate() {
        let mut fixture = Fixture::start(vec![response], false).await;
        let auth = authorization(&fixture.client);
        let material = tokens(Some(Signer::new().sign(&claims(
            &fixture.client,
            &auth,
            now(),
        ))));
        let error = fixture
            .client
            .verify_code_identity(&wire::http(), &auth, &material)
            .await
            .unwrap_err();
        assert_eq!(
            error,
            if index == 3 {
                OAuthError::ResponseTooLarge
            } else {
                OAuthError::InvalidIdentity
            }
        );
        fixture.request().await;
        assert_eq!(fixture.finish().await, 1);
    }
}

#[tokio::test]
async fn dropping_key_fetch_closes_connection_and_no_task_outlives_the_verifier() {
    let mut fixture = Fixture::start(
        vec![b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\n{\r\n".to_vec()],
        true,
    )
    .await;
    let client = fixture.client.clone();
    let auth = authorization(&client);
    let material = tokens(Some(Signer::new().sign(&claims(&client, &auth, now()))));
    let task = tokio::spawn(async move {
        client
            .verify_code_identity(&wire::http(), &auth, &material)
            .await
    });
    fixture.request().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(fixture.finish().await, 0);
}

#[tokio::test]
async fn key_fetch_own_deadline_includes_stalled_body() {
    let mut fixture = Fixture::start(
        vec![b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\n{\r\n".to_vec()],
        true,
    )
    .await;
    let client = fixture.client.clone();
    let auth = authorization(&client);
    let material = tokens(Some(Signer::new().sign(&claims(&client, &auth, now()))));
    let started = std::time::Instant::now();
    let task = tokio::spawn(async move {
        client
            .verify_code_identity(&wire::http(), &auth, &material)
            .await
    });
    fixture.request().await;
    let error = task.await.unwrap().unwrap_err();
    assert!(matches!(error, OAuthError::Timeout | OAuthError::Transport));
    assert!(started.elapsed() >= Duration::from_secs(9));
    assert!(started.elapsed() < Duration::from_secs(15));
    assert_eq!(fixture.finish().await, 0);
}
