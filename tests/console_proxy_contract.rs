//! Default egress settings obey the same management boundary as the console.
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use neuro_gateway::http::router::build_router;
use std::sync::Arc;
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

const SETTINGS: &str = "/v1/internal/gateway/console/proxy/settings";
const TEST: &str = "/v1/internal/gateway/console/proxy/test";

fn request(
    method: &str,
    path: &str,
    token: Option<&str>,
    body: serde_json::Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("x-management-token", token);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn proxy_settings_require_management_auth_and_remote_access_permission() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "provider-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    for (method, path) in [("GET", SETTINGS), ("PUT", SETTINGS), ("POST", TEST)] {
        for token in [None, Some("wrong-token")] {
            let response = router
                .clone()
                .oneshot(request(
                    method,
                    path,
                    token,
                    serde_json::json!({"mode":"off"}),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }
    let mut remote = request(
        "GET",
        SETTINGS,
        Some(MANAGEMENT_TOKEN),
        serde_json::json!(null),
    );
    remote.extensions_mut().insert(ConnectInfo(
        "192.0.2.1:1234".parse::<std::net::SocketAddr>().unwrap(),
    ));
    assert_eq!(
        router.oneshot(remote).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn saves_pending_settings_and_rejects_invalid_drafts_without_network_requests() {
    let directory =
        std::env::temp_dir().join(format!("gateway-proxy-contract-{}", uuid::Uuid::new_v4()));
    let mut fixture =
        ConsoleStateFixture::new(document("managed", "model", "provider-secret"), false);
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .config
        .console
        .state_dir = directory.clone();
    let router = build_router(Arc::clone(&fixture.state));
    for mode in ["off", "auto"] {
        let response = router
            .clone()
            .oneshot(request(
                "PUT",
                SETTINGS,
                Some(MANAGEMENT_TOKEN),
                serde_json::json!({"mode":mode}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(parse_json(response).await["mode"], mode);
    }
    for (method, path) in [("PUT", SETTINGS), ("POST", TEST)] {
        let response = router
            .clone()
            .oneshot(request(
                method,
                path,
                Some(MANAGEMENT_TOKEN),
                serde_json::json!({"mode":"custom","url":"http://user:secret@localhost:7890"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = router
        .oneshot(request(
            "GET",
            SETTINGS,
            Some(MANAGEMENT_TOKEN),
            serde_json::json!(null),
        ))
        .await
        .unwrap();
    let saved = parse_json(response).await;
    assert_eq!(saved["mode"], "auto");
    assert_eq!(saved["restartRequired"], false);
    std::fs::remove_dir_all(directory).unwrap();
}
