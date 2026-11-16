//! Offline sockets exercise the production listener and management admission; no live issuer calls.
use super::*;
use axum::http::{Method, StatusCode};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::json;
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
const ROOT: &str = "/v1/internal/gateway/console/grok-auth-sessions";

fn fixture() -> support::ConsoleStateFixture {
    support::ConsoleStateFixture::new(
        serde_json::from_value(json!({"providers":[{
        "id":"build","preset":"grok-oauth","base_url":"https://cli-chat-proxy.grok.com/v1",
        "explicit_credentials_only":true,"credentials":[]}]}))
        .unwrap(),
        true,
    )
}
async fn free_address() -> std::net::SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
}
async fn create(state: &Arc<AppState>, redirect: &str, grant: Option<&str>) -> Response {
    support::send_console_json(
        state,
        Method::POST,
        ROOT,
        json!({
            "providerId":"build","expectedRevision":state.route_config.snapshot().revision().id(),
            "clientId":"permitted-client","scopes":["offline_access"],
            "authorizationFlow":"loopback","redirectUri":redirect
        }),
        grant,
        None,
    )
    .await
}
async fn send(address: std::net::SocketAddr, target: &str) -> String {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut socket = TcpStream::connect(address).await.unwrap();
        socket
            .write_all(format!("GET {target} HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).await.unwrap();
        response
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn loopback_admission_requires_grant_and_bound_explicit_numeric_port() {
    let fixture = fixture();
    let state = &fixture.state;
    let occupied = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect = format!("http://{}/callback", occupied.local_addr().unwrap());
    assert_eq!(
        create(state, &redirect, None).await.status(),
        StatusCode::FORBIDDEN
    );
    let grant = support::grant_console_secret_access(state).await;
    for bad in [
        &redirect,
        "http://localhost:34567/callback",
        "http://127.0.0.1:0/callback",
        "http://0.0.0.0:34567/callback",
    ] {
        assert_eq!(
            create(state, bad, Some(&grant)).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(state.lifecycle.grok_oauth.count(), 0);
    state.lifecycle.grok_oauth.join().await;
}

#[tokio::test]
async fn loopback_wrong_state_keeps_listener_and_denial_releases_without_saving() {
    let fixture = fixture();
    let state = &fixture.state;
    let address = free_address().await;
    let grant = support::grant_console_secret_access(state).await;
    let response = create(state, &format!("http://{address}/callback"), Some(&grant)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::parse_json(response).await;
    assert_eq!(body["session"]["automaticCallback"], true);
    let auth = url::Url::parse(body["session"]["authorizationUrl"].as_str().unwrap()).unwrap();
    let nonce = auth
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    for target in ["/wrong?state=x", "/callback?code=unused&state=wrong"] {
        let response = send(address, target).await;
        assert!(response.starts_with("HTTP/1.1 400"));
        assert!(response.contains("Cache-Control: no-store"));
        assert!(!response.contains("unused"));
    }
    assert_eq!(state.lifecycle.grok_oauth.count(), 1);
    let response = send(
        address,
        &format!("/callback?error=access_denied&state={nonce}"),
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 400"));
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    state.lifecycle.grok_oauth.join().await;
    assert!(TcpListener::bind(address).await.is_ok());
}

#[tokio::test]
async fn loopback_shutdown_joins_pending_header_and_releases_port() {
    let fixture = fixture();
    let state = &fixture.state;
    let address = free_address().await;
    let grant = support::grant_console_secret_access(state).await;
    assert_eq!(
        create(state, &format!("http://{address}/callback"), Some(&grant))
            .await
            .status(),
        StatusCode::OK
    );
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket
        .write_all(b"GET /callback?code=incomplete")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), state.lifecycle.grok_oauth.join())
        .await
        .unwrap();
    assert!(TcpListener::bind(address).await.is_ok());
    assert_eq!(
        create(state, &format!("http://{address}/callback"), Some(&grant))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn loopback_exchange_survives_browser_disconnect_and_stops_at_ready() {
    let fixture = fixture();
    let state = &fixture.state;
    let address = free_address().await;
    let issuer = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = OAuthClient::new("permitted-client", &["offline_access"])
        .unwrap()
        .test_issuer(&format!("http://{}", issuer.local_addr().unwrap()));
    let (sent, received) = tokio::sync::oneshot::channel();
    let sent = Arc::new(parking_lot::Mutex::new(Some(sent)));
    let release = Arc::new(tokio::sync::Notify::new());
    let gate = release.clone();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let tracker = calls.clone();
    // Code grants without a verified user ID token require an explicit Team principal hint.
    let token = format!(
        "{}.{}.s",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(r#"{"principal_type":"Team","principal_id":"synthetic-team"}"#)
    );
    let app = axum::Router::new().route("/oauth2/token", axum::routing::post(move || {
        let sent = sent.clone();
        let gate = gate.clone();
        let token = token.clone();
        tracker.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        async move {
            if let Some(sent) = sent.lock().take() { let _ = sent.send(()); }
            gate.notified().await;
            Json(json!({"access_token":token,"refresh_token":"synthetic-refresh","expires_in":120,"scope":"offline_access"}))
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(issuer, app).await.unwrap();
    });
    let session = state
        .lifecycle
        .grok_oauth
        .create_loopback(
            state,
            Binding::new(
                "synthetic-owner",
                "build",
                state.route_config.snapshot().revision().id(),
            )
            .unwrap(),
            client,
            &format!("http://{address}/callback"),
            None,
            rquest::Client::builder().no_proxy().build().unwrap(),
        )
        .unwrap();
    let auth = url::Url::parse(&session.view().authorization_url.unwrap()).unwrap();
    let nonce = auth
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket
        .write_all(
            format!(
                "GET /callback?code=synthetic&state={nonce} HTTP/1.1\r\nHost: {address}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    drop(socket);
    tokio::time::timeout(Duration::from_secs(3), received)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(session.view().status, grok_oauth::SessionStatus::Exchanging);
    assert!(session
        .complete(
            &rquest::Client::builder().no_proxy().build().unwrap(),
            &format!("http://{address}/callback?code=synthetic&state={nonce}")
        )
        .await
        .is_err());
    release.notify_one();
    tokio::time::timeout(Duration::from_secs(3), async {
        while session.view().status == grok_oauth::SessionStatus::Exchanging {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(session.view().status, grok_oauth::SessionStatus::Ready);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    assert!(!serde_json::to_string(&session.view())
        .unwrap()
        .contains("synthetic-refresh"));
    state.lifecycle.grok_oauth.join().await;
    task.abort();
    let _ = task.await;
    assert!(TcpListener::bind(address).await.is_ok());
}
