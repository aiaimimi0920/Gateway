//! Real fixed sockets prove discovery admission and shutdown ownership without external traffic.
use super::*;
use crate::{
    protocol::dim_oauth::{test_support::origin, Site},
    routing::config::RouteConfigYaml,
};
use axum::{routing::get, Json, Router};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::AsyncWriteExt;
#[path = "../../../../tests/console_contract_support/mod.rs"]
mod support;

struct Peer {
    origin: String,
    hits: Arc<AtomicUsize>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Peer {
    async fn new(stall: bool) -> Self {
        let hits = Arc::new(AtomicUsize::new(0));
        let capture = hits.clone();
        let app = Router::new().route("/.well-known/openid-configuration",get(move || {
            let hits = capture.clone(); async move {
                hits.fetch_add(1,Ordering::SeqCst);
                if stall { std::future::pending::<()>().await; }
                Json(json!({"issuer":"https://dimagent.cn","authorization_endpoint":"https://dimagent.cn/oauth/authorize",
                    "token_endpoint":"https://dimagent.cn/oauth/token","userinfo_endpoint":"https://dimagent.cn/oauth/userinfo",
                    "jwks_uri":"https://dimagent.cn/oauth/jwks.json","response_types_supported":["code"],
                    "code_challenge_methods_supported":["S256"],"token_endpoint_auth_methods_supported":["none"],
                    "id_token_signing_alg_values_supported":["RS256"]}))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }));
        Self { origin, hits, task }
    }
    async fn stop(mut self) {
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
    async fn wait_hits(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(4), async {
            while self.hits.load(Ordering::SeqCst) < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
fn document() -> RouteConfigYaml {
    serde_json::from_value(
        json!({"providers":[{"id":"dim","auth_mode":"dim_user_oauth",
        "explicit_credentials_only":true,"base_url":"https://dimagent.cn/v1","credentials":[]}]}),
    )
    .unwrap()
}
fn redirect() -> (u16, String) {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    (port, format!("http://127.0.0.1:{port}/oauth/callback"))
}
async fn create(
    state: &Arc<AppState>,
    redirect: &str,
    ttl: Duration,
) -> Result<Arc<Session>, crate::error::GatewayError> {
    state
        .lifecycle
        .dim_oauth
        .create_with_ttl(
            state,
            Binding::new(
                "resource-owner",
                "dim",
                state.route_config.snapshot().revision().id(),
                Site::China,
            )
            .unwrap(),
            OAuthClient::new(Site::China, "permitted-gateway", &["openid"]).unwrap(),
            redirect,
            rquest::Client::builder().no_proxy().build().unwrap(),
            ttl,
        )
        .await
}

#[tokio::test]
async fn dim_oauth_management_discovery_is_bounded_and_shutdown_joins_and_releases_all_fixed_ports()
{
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let registry = &state.lifecycle.dim_oauth;
    let peer = Peer::new(true).await;
    let mut tasks = JoinSet::new();
    let mut ports = Vec::new();
    for _ in 0..16 {
        let (port, redirect) = redirect();
        ports.push(port);
        let state = state.clone();
        let peer_origin = peer.origin.clone();
        tasks.spawn(async move {
            origin(
                peer_origin,
                create(&state, &redirect, Duration::from_secs(300)),
            )
            .await
        });
    }
    peer.wait_hits(16).await;
    let (_, uri) = redirect();
    let error = origin(
        peer.origin.clone(),
        create(state, &uri, Duration::from_secs(300)),
    )
    .await
    .unwrap_err();
    assert!(error.message.contains("capacity"));
    assert_eq!(peer.hits.load(Ordering::SeqCst), 16);
    tokio::time::timeout(Duration::from_secs(2), async {
        registry.shutdown();
        registry.join().await;
        while let Some(result) = tasks.join_next().await {
            assert!(result.unwrap().unwrap_err().message.contains("stopped"));
        }
    })
    .await
    .unwrap();
    for port in ports {
        assert!(std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).is_ok());
    }
    assert!(registry.operations.lock().is_empty());
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_occupied_registered_port_never_falls_back_or_fetches_discovery() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let peer = Peer::new(false).await;
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    let error = origin(
        peer.origin.clone(),
        create(
            &fixture.state,
            &format!("http://127.0.0.1:{port}/oauth/callback"),
            Duration::from_secs(300),
        ),
    )
    .await
    .unwrap_err();
    assert!(error.message.contains("listener_unavailable"));
    assert_eq!(peer.hits.load(Ordering::SeqCst), 0);
    fixture.state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_expiry_and_shutdown_interrupt_partial_callback_headers() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let peer = Peer::new(false).await;
    let (port, uri) = redirect();
    let session = origin(
        peer.origin.clone(),
        create(state, &uri, Duration::from_millis(50)),
    )
    .await
    .unwrap();
    let mut stream = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    stream
        .write_all(b"GET /oauth/callback?code=c HTTP/1.1\r\nHost: ")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(session.view().status, super::super::SessionStatus::Expired);
    let (other, uri) = redirect();
    let waiting = origin(
        peer.origin.clone(),
        create(state, &uri, Duration::from_secs(300)),
    )
    .await
    .unwrap();
    let mut stream = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, other))
        .await
        .unwrap();
    stream
        .write_all(b"GET /oauth/callback?code=c HTTP/1.1\r\nHost: ")
        .await
        .unwrap();
    state.lifecycle.dim_oauth.spawn(state.shutdown.clone());
    state.shutdown.request("synthetic_shutdown");
    tokio::time::timeout(Duration::from_secs(2), state.lifecycle.dim_oauth.join())
        .await
        .unwrap();
    assert_eq!(
        waiting.view().status,
        super::super::SessionStatus::Cancelled
    );
    for port in [port, other] {
        assert!(
            tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .is_err()
        );
    }
    peer.stop().await;
}
