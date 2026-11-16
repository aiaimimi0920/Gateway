//! Kiro management endpoints must use ordinary management/Secret Grant admission, not device codes.
mod console_contract_support;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use console_contract_support as support;
use neuro_gateway::{http::router::build_router, routing::config::RouteConfigYaml};
use serde_json::json;
use tower::ServiceExt;

const ROOT: &str = "/v1/internal/gateway/console/kiro-auth-sessions";
fn document() -> RouteConfigYaml {
    // A compiled-out preset cannot be loaded. The HTTP feature gate must still reject
    // a well-formed create before target lookup, so use a valid empty console there.
    if !cfg!(feature = "line-kiro-official-vendor-api") {
        return serde_json::from_value(json!({"providers":[]})).unwrap();
    }
    serde_json::from_value(json!({"providers":[{"id":"kiro-pool","preset":"kiro",
        "base_url":"https://q.us-east-1.amazonaws.com","explicit_credentials_only":true,"credentials":[]}]})).unwrap()
}

#[tokio::test]
async fn kiro_management_routes_require_management_identity_and_secret_grant() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let response = build_router(fixture.state.clone())
        .oneshot(
            Request::get(format!("{ROOT}/unknown"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = json!({"providerId":"kiro-pool","expectedRevision":fixture.state.route_config.snapshot().revision().id(),
        "directory":{"authMethod":"builder_id"},"scopes":["codewhisperer:conversations"],"machineId":"a".repeat(64)});
    let response =
        support::send_console_json(&fixture.state, Method::POST, ROOT, body, None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn kiro_create_rejects_missing_explicit_machine_or_disabled_line_without_network() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let body = json!({"providerId":"kiro-pool","expectedRevision":fixture.state.route_config.snapshot().revision().id(),
        "directory":{"authMethod":"builder_id"},"scopes":["codewhisperer:conversations"]});
    let response =
        support::send_console_json(&fixture.state, Method::POST, ROOT, body, Some(&grant), None)
            .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["cache-control"], "no-store");
}

#[tokio::test]
async fn kiro_management_body_limits_apply_before_json_or_session_admission() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    for (endpoint, size) in [
        (ROOT.to_owned(), 17 * 1024),
        (format!("{ROOT}/unknown/poll"), 2048),
    ] {
        let response = support::send_console_json(
            &fixture.state,
            Method::POST,
            &endpoint,
            json!({"padding":"x".repeat(size)}),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}

#[tokio::test]
async fn kiro_builder_directory_unknown_fields_are_rejected_before_feature_or_network_admission() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let body = json!({"providerId":"kiro-pool","expectedRevision":fixture.state.route_config.snapshot().revision().id(),
        "directory":{"authMethod":"builder_id","startUrl":"https://attacker.invalid/start"},
        "scopes":["codewhisperer:conversations"],"machineId":"a".repeat(64)});
    let response =
        support::send_console_json(&fixture.state, Method::POST, ROOT, body, Some(&grant), None)
            .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.headers()["cache-control"], "no-store");
}

#[cfg(not(feature = "line-kiro-official-vendor-api"))]
#[tokio::test]
async fn compiled_out_kiro_management_rejects_valid_create_without_network() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let body = json!({"providerId":"kiro-pool","expectedRevision":fixture.state.route_config.snapshot().revision().id(),
        "directory":{"authMethod":"builder_id"},"scopes":["codewhisperer:conversations"],"machineId":"a".repeat(64)});
    let response =
        support::send_console_json(&fixture.state, Method::POST, ROOT, body, Some(&grant), None)
            .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(support::parse_json(response)
        .await
        .to_string()
        .contains("kiro_oauth_feature_disabled"));
}
