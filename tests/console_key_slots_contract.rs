//! Two-slot persistence, passwordless console sessions and trusted transport boundaries.
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use neuro_gateway::{console::ConsoleRequestContext, http::router::build_router};
use std::sync::Arc;
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

fn request(path: &str, token: &str, peer: Option<&str>) -> Request<Body> {
    let mut request = Request::get(path)
        .header("x-management-token", token)
        .header("x-gateway-client-ip", "127.0.0.1")
        .body(Body::empty())
        .unwrap();
    if let Some(peer) = peer {
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<std::net::SocketAddr>().unwrap()));
    }
    request
}

#[tokio::test]
async fn disabled_keys_persist_and_passwordless_sessions_stop_when_a_key_is_enabled() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "provider-secret"), false);
    let auth = &fixture.state.console_auth;
    let local = ConsoleRequestContext::loopback();
    assert!(auth.passwordless_token().unwrap().is_none());
    auth.set_management_key_slot(
        &local,
        MANAGEMENT_TOKEN,
        1,
        true,
        Some("second-management-key"),
    )
    .unwrap();
    auth.set_management_key_slot(&local, MANAGEMENT_TOKEN, 0, false, None)
        .unwrap();
    assert!(auth.verify_management_token(MANAGEMENT_TOKEN).is_err());
    assert!(auth.passwordless_token().unwrap().is_none());
    auth.set_management_key_slot(&local, "second-management-key", 1, false, None)
        .unwrap();
    let marker = auth.passwordless_token().unwrap().unwrap();
    assert!(auth.verify_management_token(&marker).is_err()); // Not a trusted forwarding credential.
    assert!(auth.verify_management_token("").is_err());
    auth.verify_session(&local, &marker, None).unwrap();
    let grant = auth
        .confirm_secret_access(&local, &marker, &marker)
        .unwrap();
    let actor = auth.authenticate_management_token(&local, &marker).unwrap();
    auth.verify_secret_grant(&local, &actor, &grant.grant)
        .unwrap();
    let keys = serde_json::to_value(auth.list_management_keys(&local, &marker).unwrap()).unwrap();
    assert_eq!(keys.as_array().unwrap().len(), 2);
    assert_eq!(keys[0]["enabled"], false);
    assert_eq!(keys[1]["enabled"], false);
    let mut config = fixture.state.config.console.clone();
    config.state_dir = auth
        .admin_path()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let restarted =
        neuro_gateway::console::ConsoleAuthRuntime::new(&config, Some(MANAGEMENT_TOKEN.into()))
            .unwrap();
    assert_eq!(
        restarted.passwordless_token().unwrap(),
        Some(marker.clone())
    );
    restarted
        .set_management_key_slot(&local, &marker, 0, true, None)
        .unwrap();
    assert!(auth.passwordless_token().unwrap().is_none());
    assert!(auth.verify_session(&local, &marker, None).is_err());
    assert!(auth.verify_management_token(MANAGEMENT_TOKEN).is_ok());
    assert!(auth
        .verify_management_token("second-management-key")
        .is_err());
}

#[tokio::test]
async fn marker_keeps_remote_policy_and_cannot_forge_transport_context() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "provider-secret"), false);
    fixture
        .state
        .console_auth
        .set_management_key_slot(
            &ConsoleRequestContext::loopback(),
            MANAGEMENT_TOKEN,
            0,
            false,
            None,
        )
        .unwrap();
    let marker = fixture
        .state
        .console_auth
        .passwordless_token()
        .unwrap()
        .unwrap();
    let router = build_router(Arc::clone(&fixture.state));
    let response = router
        .clone()
        .oneshot(request(
            "/v1/internal/gateway/console/management-keys",
            &marker,
            Some("127.0.0.1:1234"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = router
        .clone()
        .oneshot(request(
            "/v1/internal/gateway/console/management-keys",
            &marker,
            Some("192.0.2.1:1234"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    // This generic-management route has no explicit peer extractor. Its task-local guard must isolate peers.
    let (local, remote) = tokio::join!(
        router.clone().oneshot(request(
            "/v1/internal/gateway/costs",
            &marker,
            Some("127.0.0.1:1234")
        )),
        router.clone().oneshot(request(
            "/v1/internal/gateway/costs",
            &marker,
            Some("192.0.2.1:1234")
        ))
    );
    assert_eq!(remote.unwrap().status(), StatusCode::FORBIDDEN);
    let local = local.unwrap();
    // The fixture has no PostgreSQL: reaching its normal dependency error proves admission, not business success.
    assert_eq!(local.status(), StatusCode::SERVICE_UNAVAILABLE);
    let missing_peer = router
        .oneshot(request("/v1/internal/gateway/costs", &marker, None))
        .await
        .unwrap();
    assert_eq!(missing_peer.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn set_slot_http_contract_and_old_ring_defaults_preserve_credentials() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "provider-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    let make = |token: &str, slot: usize| {
        Request::post("/v1/internal/gateway/console/management-keys")
            .header("x-management-token", token)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({"action":"setSlot","slot":slot,"enabled":false}).to_string(),
            ))
            .unwrap()
    };
    assert_eq!(
        router
            .clone()
            .oneshot(make("invalid", 0))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        router
            .clone()
            .oneshot(make(MANAGEMENT_TOKEN, 2))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        router
            .oneshot(make(MANAGEMENT_TOKEN, 0))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let path = fixture.state.console_auth.admin_path();
    let mut stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    stored["keys"][0].as_object_mut().unwrap().remove("enabled");
    std::fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
    assert!(fixture
        .state
        .console_auth
        .passwordless_token()
        .unwrap()
        .is_none());
    assert!(fixture
        .state
        .console_auth
        .verify_management_token(MANAGEMENT_TOKEN)
        .is_ok());
}
