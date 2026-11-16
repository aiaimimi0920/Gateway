//! Inbound sharing is a live, persisted policy based on the transport peer, not proxy headers.
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use neuro_gateway::{console::RouteConfigPersistence, http::router::build_router};
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;
const SETTINGS: &str = "/v1/internal/gateway/console/lan-sharing";

struct Fixture {
    inner: ConsoleStateFixture,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let mut inner =
            ConsoleStateFixture::new(document("managed", "model", "provider-secret"), false);
        let root = std::env::temp_dir().join(format!("gateway-sharing-{}", uuid::Uuid::new_v4()));
        let state = Arc::get_mut(&mut inner.state).unwrap();
        state.config.console.state_dir = root.clone();
        state.config.console.routes_file = root.join("routes.yaml");
        Self { inner, root }
    }
    fn router(&self) -> axum::Router {
        build_router(Arc::clone(&self.inner.state))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
        if self.root.exists() {
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }
}
fn request(
    method: &str,
    path: &str,
    peer: Option<&str>,
    token: Option<&str>,
    body: serde_json::Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("x-forwarded-for", "127.0.0.1")
        .header("x-gateway-client-ip", "127.0.0.1");
    if let Some(token) = token {
        builder = builder.header("x-management-token", token);
    }
    let mut request = builder.body(Body::from(body.to_string())).unwrap();
    if let Some(peer) = peer {
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<std::net::SocketAddr>().unwrap()));
    }
    request
}
fn get(path: &str, peer: Option<&str>) -> Request<Body> {
    request(
        "GET",
        path,
        peer,
        Some(MANAGEMENT_TOKEN),
        serde_json::Value::Null,
    )
}
fn save(enabled: bool) -> Request<Body> {
    request(
        "PUT",
        SETTINGS,
        Some("127.0.0.1:1234"),
        Some(MANAGEMENT_TOKEN),
        serde_json::json!({"enabled":enabled}),
    )
}

#[tokio::test]
async fn defaults_on_without_weakening_authentication_or_creating_a_policy_file() {
    let fixture = Fixture::new();
    let router = fixture.router();
    let response = router
        .clone()
        .oneshot(get(SETTINGS, Some("127.0.0.1:1234")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(parse_json(response).await["enabled"], true);
    assert!(!fixture.root.exists());
    assert_eq!(
        router
            .clone()
            .oneshot(get("/healthz", Some("192.0.2.10:1234")))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    for method in ["GET", "PUT"] {
        let response = router
            .clone()
            .oneshot(request(
                method,
                SETTINGS,
                Some("127.0.0.1:1234"),
                None,
                serde_json::json!({"enabled":false}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        router
            .oneshot(get(SETTINGS, Some("192.0.2.10:1234")))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn disables_other_devices_immediately_and_retains_local_access_across_restart() {
    let fixture = Fixture::new();
    let router = fixture.router();
    assert_eq!(
        router.clone().oneshot(save(false)).await.unwrap().status(),
        StatusCode::OK
    );
    for path in ["/healthz", "/readyz", "/v1/models", "/ui/", SETTINGS] {
        let response = router
            .clone()
            .oneshot(get(path, Some("192.0.2.10:1234")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
        assert!(parse_json(response)
            .await
            .to_string()
            .contains("gateway_lan_sharing_disabled"));
    }
    for path in ["/v1/chat/completions", "/v1/responses"] {
        assert_eq!(
            router
                .clone()
                .oneshot(request(
                    "POST",
                    path,
                    Some("192.0.2.10:1234"),
                    Some(MANAGEMENT_TOKEN),
                    serde_json::json!({})
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    for peer in ["127.0.0.1:1234", "[::1]:1234", "[::ffff:127.0.0.1]:1234"] {
        assert_eq!(
            router
                .clone()
                .oneshot(get("/healthz", Some(peer)))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        router
            .oneshot(get("/healthz", None))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let restarted = fixture.router();
    assert_eq!(
        parse_json(
            restarted
                .clone()
                .oneshot(get(SETTINGS, Some("127.0.0.1:1234")))
                .await
                .unwrap()
        )
        .await["enabled"],
        false
    );
    assert_eq!(
        restarted
            .clone()
            .oneshot(save(true))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        restarted
            .oneshot(get("/healthz", Some("192.0.2.10:1234")))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_local_adapter_address_is_not_mistaken_for_another_device() {
    // UDP connect only selects a route; no datagram is sent to this documentation address.
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.connect("192.0.2.1:9").unwrap();
    let source = socket.local_addr().unwrap().to_string();
    let fixture = Fixture::new();
    let router = fixture.router();
    assert_eq!(
        router.clone().oneshot(save(false)).await.unwrap().status(),
        StatusCode::OK
    );
    assert_eq!(
        router
            .oneshot(get("/healthz", Some(&source)))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn unsuccessful_writes_do_not_change_active_policy_and_corruption_is_not_default_on() {
    let fixture = Fixture::new();
    let router = fixture.router();
    let store =
        RouteConfigPersistence::new(&fixture.root, &fixture.root.join("routes.yaml")).unwrap();
    let guard = store.try_writer_lock().unwrap();
    assert_eq!(
        router.clone().oneshot(save(false)).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        router
            .oneshot(get("/healthz", Some("192.0.2.10:1234")))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    drop(guard);
    std::fs::write(fixture.root.join("console/lan-sharing.json"), b"invalid").unwrap();
    let corrupted = fixture.router();
    assert_eq!(
        corrupted
            .clone()
            .oneshot(get("/healthz", Some("192.0.2.10:1234")))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        corrupted
            .clone()
            .oneshot(get("/healthz", Some("127.0.0.1:1234")))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        corrupted
            .clone()
            .oneshot(save(true))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        corrupted
            .oneshot(get("/healthz", Some("192.0.2.10:1234")))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}
