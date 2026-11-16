//! No credentials or live service: real code exchange uses a bounded synthetic loopback server.
use super::*;
use crate::{
    protocol::grok_oauth::{GrantAssurance, OAuthClient},
    state::GatewayShutdownHandle,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) struct Server {
    pub(super) client: OAuthClient,
    ready: Option<tokio::sync::oneshot::Receiver<()>>,
    task: Option<tokio::task::JoinHandle<usize>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
impl Server {
    pub(super) async fn new(stall: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = client().test_issuer(&format!("http://{}", listener.local_addr().unwrap()));
        let (sender, ready) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut bytes = Vec::new();
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let mut buf = [0; 2048];
                    let n = socket.read(&mut buf).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buf[..n]);
                    assert!(bytes.len() <= 65536);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = std::str::from_utf8(&bytes[..end]).unwrap();
                        assert!(header.starts_with("POST /oauth2/token HTTP/1.1"));
                        let length = header
                            .lines()
                            .filter_map(|l| l.split_once(':'))
                            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                            .unwrap()
                            .1
                            .trim()
                            .parse::<usize>()
                            .unwrap();
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
            })
            .await
            .unwrap();
            if stall {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\n{\r\n")
                    .await
                    .unwrap();
                sender.send(()).unwrap();
                let mut buf = [0; 1024];
                return tokio::time::timeout(Duration::from_secs(3), socket.read(&mut buf))
                    .await
                    .unwrap()
                    .unwrap();
            }
            let token = format!(
                "{}.{}.s",
                URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256","typ":"JWT"}"#),
                URL_SAFE_NO_PAD
                    .encode(r#"{"principal_type":"Team","principal_id":"private-team"}"#)
            );
            let body=serde_json::json!({"access_token":token,"refresh_token":"private-refresh","expires_in":60}).to_string();
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
            let _ = sender.send(());
            1
        });
        Self {
            client,
            ready: Some(ready),
            task: Some(task),
        }
    }
    async fn ready(&mut self) {
        self.ready.take().unwrap().await.unwrap();
    }
    pub(super) async fn finish(&mut self) -> usize {
        self.task.take().unwrap().await.unwrap()
    }
}
fn client() -> OAuthClient {
    OAuthClient::new("synthetic-client", &["openid", "offline_access"]).unwrap()
}
fn binding() -> Binding {
    Binding::new("operator-host-ip", "grok-oauth-fixture", "saved-revision").unwrap()
}
pub(super) fn http() -> rquest::Client {
    rquest::Client::builder().no_proxy().build().unwrap()
}
pub(super) fn callback(session: &Session) -> String {
    let url = url::Url::parse(&session.view().authorization_url.unwrap()).unwrap();
    let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    format!(
        "{}?code=synthetic-code&state={}",
        pairs["redirect_uri"], pairs["state"]
    )
}
fn create(registry: &Registry, client: OAuthClient) -> Arc<Session> {
    registry
        .create(binding(), client, "http://127.0.0.1:34567/callback", None)
        .unwrap()
}

#[tokio::test]
async fn registry_is_instance_owned_bounded_and_binds_operator_provider_and_revision() {
    let registry = Registry::default();
    let other = Registry::default();
    let first = create(&registry, client());
    for (owner, provider, revision) in [
        ("other", "grok-oauth-fixture", "saved-revision"),
        ("operator-host-ip", "other", "saved-revision"),
        ("operator-host-ip", "grok-oauth-fixture", "new-revision"),
    ] {
        assert!(registry
            .get(
                first.id(),
                &Binding::new(owner, provider, revision).unwrap()
            )
            .is_err());
    }
    assert!(other.get(first.id(), &binding()).is_err());
    for _ in 1..16 {
        create(&registry, client());
    }
    assert!(matches!(
        registry.create(binding(), client(), "http://127.0.0.1:34567/callback", None),
        Err(LoginError::Capacity)
    ));
    first.cancel();
    create(&registry, client());
    assert_eq!(registry.count(), 16);
    registry.join().await;
    assert_eq!(registry.count(), 0);
    assert!(matches!(
        registry.create(binding(), client(), "http://127.0.0.1:34567/callback", None),
        Err(LoginError::Stopped)
    ));
}

#[tokio::test]
async fn callback_and_material_claim_are_once_only_and_wrong_revision_does_not_consume() {
    let mut server = Server::new(false).await;
    let registry = Registry::default();
    let session = create(&registry, server.client.clone());
    let callback = callback(&session);
    session.complete(&http(), &callback).await.unwrap();
    assert_eq!(server.finish().await, 1);
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert!(session.view().authorization_url.is_none());
    assert!(!session.view().identity_verified);
    assert!(session.complete(&http(), &callback).await.is_err());
    let wrong = Binding::new("operator-host-ip", "grok-oauth-fixture", "new-revision").unwrap();
    assert!(session.claim_receipt(&wrong).is_err());
    let receipt = session.claim_receipt(&binding()).unwrap();
    assert_eq!(receipt.assurance(), GrantAssurance::TokenEndpointOnly);
    assert_eq!(
        receipt.tokens().refresh_token.as_deref(),
        Some("private-refresh")
    );
    assert!(session.claim_receipt(&binding()).is_err());
    assert_eq!(session.view().status, SessionStatus::Claimed);
    let view = serde_json::to_string(&session.view()).unwrap();
    assert!(!view.contains("private-"));
    assert!(!format!("{session:?} {receipt:?}").contains("private-"));
    registry.join().await;
}

#[tokio::test]
async fn wrong_state_does_not_consume_a_valid_login() {
    let mut server = Server::new(false).await;
    let registry = Registry::default();
    let session = create(&registry, server.client.clone());
    assert!(session
        .complete(
            &http(),
            "http://127.0.0.1:34567/callback?code=x&state=wrong"
        )
        .await
        .is_err());
    assert_eq!(session.view().status, SessionStatus::WaitingUser);
    session
        .complete(&http(), &callback(&session))
        .await
        .unwrap();
    assert_eq!(server.finish().await, 1);
    registry.join().await;
}

#[tokio::test]
async fn cancel_and_future_drop_both_release_connection_without_stuck_exchange() {
    for cancel in [true, false] {
        let mut server = Server::new(true).await;
        let registry = Registry::default();
        let session = create(&registry, server.client.clone());
        let callback = callback(&session);
        let active = session.clone();
        let task = tokio::spawn(async move { active.complete(&http(), &callback).await });
        server.ready().await;
        assert_eq!(session.view().status, SessionStatus::Exchanging);
        if cancel {
            session.cancel();
            assert!(task.await.unwrap().is_err());
            assert_eq!(session.view().status, SessionStatus::Cancelled);
        } else {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            assert_eq!(session.view().status, SessionStatus::Failed);
        }
        assert_eq!(server.finish().await, 0);
        assert!(session.view().authorization_url.is_none());
        assert!(session.claim_receipt(&binding()).is_err());
        registry.join().await;
    }
}

#[tokio::test]
async fn concurrent_callback_is_rejected_while_the_first_exchange_owns_the_code() {
    let mut server = Server::new(true).await;
    let registry = Registry::default();
    let session = create(&registry, server.client.clone());
    let callback = callback(&session);
    let active = session.clone();
    let copy = callback.clone();
    let task = tokio::spawn(async move { active.complete(&http(), &copy).await });
    server.ready().await;
    assert!(session.complete(&http(), &callback).await.is_err());
    session.cancel();
    assert!(task.await.unwrap().is_err());
    assert_eq!(server.finish().await, 0);
    registry.join().await;
}

#[tokio::test]
async fn session_deadline_cancels_a_stalled_exchange_before_transport_deadline() {
    let mut server = Server::new(true).await;
    let registry = Registry::default();
    let session =
        registry.short_session(binding(), server.client.clone(), Duration::from_millis(100));
    let callback = callback(&session);
    let active = session.clone();
    let task = tokio::spawn(async move { active.complete(&http(), &callback).await });
    server.ready().await;
    assert!(task.await.unwrap().is_err());
    assert_eq!(session.view().status, SessionStatus::Expired);
    assert_eq!(server.finish().await, 0);
    registry.join().await;
}

#[tokio::test]
async fn global_shutdown_cancels_inflight_login_and_joins_one_registry_worker() {
    let mut server = Server::new(true).await;
    let registry = Arc::new(Registry::default());
    let shutdown = GatewayShutdownHandle::default();
    registry.spawn(shutdown.clone());
    registry.spawn(shutdown.clone());
    let session = create(&registry, server.client.clone());
    let callback = callback(&session);
    let active = session.clone();
    let task = tokio::spawn(async move { active.complete(&http(), &callback).await });
    server.ready().await;
    shutdown.request("synthetic_shutdown");
    assert!(task.await.unwrap().is_err());
    registry.join().await;
    registry.join().await;
    assert_eq!(registry.count(), 0);
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert_eq!(server.finish().await, 0);
}

#[tokio::test]
async fn expiry_worker_clears_unused_sessions_without_status_requests() {
    let registry = Arc::new(Registry::default());
    registry.spawn(GatewayShutdownHandle::default());
    let session = registry.short_session(binding(), client(), Duration::from_millis(20));
    tokio::time::timeout(Duration::from_secs(3), async {
        while registry.count() != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(session.view().status, SessionStatus::Expired);
    assert!(session.view().authorization_url.is_none());
    registry.join().await;
}
