//! Management rejection tests never perform a real Luma request.
use axum::http::{Method, StatusCode};
use serde_json::json;
#[path = "fixtures/luma_mcp.rs"]
mod fixture;
#[path = "console_contract_support/mod.rs"]
mod support;
const PATH: &str = "/v1/internal/gateway/console/luma-mcp-tools";

fn state() -> support::ConsoleStateFixture {
    support::ConsoleStateFixture::new(fixture::document(fixture::provider()), true)
}

#[tokio::test]
async fn luma_mcp_catalog_requires_secret_consent_and_forbids_request_overrides() {
    let f = state();
    let body = json!({"providerId":"luma-mcp","credentialId":"saved-mcp",
        "expectedRevision":f.state.route_config.snapshot().revision().id()});
    let response =
        support::send_console_json(&f.state, Method::POST, PATH, body.clone(), None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let grant = support::grant_console_secret_access(&f.state).await;
    for key in ["apiKey", "baseUrl", "clientId", "toolName", "arguments"] {
        let mut input = body.clone();
        input[key] = json!("caller-override");
        let response =
            support::send_console_json(&f.state, Method::POST, PATH, input, Some(&grant), None)
                .await;
        assert!(matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ));
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}

#[tokio::test]
async fn luma_mcp_catalog_rejects_stale_revision_wrong_account_and_expired_material() {
    let mut provider = fixture::provider();
    provider["credentials"][0]["luma_oauth"]["expires_at"] = json!("2020-01-01T00:00:00Z");
    let f = support::ConsoleStateFixture::new(fixture::document(provider), true);
    let grant = support::grant_console_secret_access(&f.state).await;
    for (provider, credential, revision, status) in [
        (
            "luma-mcp",
            "saved-mcp",
            "stale".to_string(),
            StatusCode::CONFLICT,
        ),
        (
            "other",
            "saved-mcp",
            f.state.route_config.snapshot().revision().id().to_string(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "luma-mcp",
            "missing",
            f.state.route_config.snapshot().revision().id().to_string(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "luma-mcp",
            "saved-mcp",
            f.state.route_config.snapshot().revision().id().to_string(),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = support::send_console_json(
            &f.state,
            Method::POST,
            PATH,
            json!({"providerId":provider,"credentialId":credential,"expectedRevision":revision}),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}

#[tokio::test]
async fn luma_mcp_normal_secret_commit_retains_context_and_uses_revision_cas() {
    let mut empty = fixture::provider();
    empty["credentials"] = json!([]);
    let f = support::ConsoleStateFixture::new(fixture::document(empty), true);
    let grant = support::grant_console_secret_access(&f.state).await;
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let mut next = fixture::provider();
    next["credentials"][0]["api_key"] = serde_json::Value::Null;
    let body = json!({"expectedRevision":revision,"document":fixture::document(next),
        "secretPatches":[{"path":"/providers/0/credentials/0/api_key","operation":"replace","value":"synthetic-access"}]});
    let response = support::send_console_json(
        &f.state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    let status = response.status();
    let result = support::parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(!result.to_string().contains("synthetic-access"));
    let providers = f.state.route_config.get_providers();
    let saved = &providers[0].credential_pool[0];
    assert_eq!(saved.payload.api_key, "synthetic-access");
    assert_eq!(saved.id, "saved-mcp");
    assert!(!saved.enabled);
    assert!(f
        .state
        .route_config
        .snapshot()
        .resolve_candidates(Some("any-model"))
        .is_empty());
    let response = support::send_console_json(
        &f.state,
        Method::GET,
        "/v1/internal/gateway/console/route-config",
        serde_json::Value::Null,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let loaded = support::parse_json(response).await;
    assert!(!loaded.to_string().contains("synthetic-access"));
    assert_eq!(
        loaded["routeConfig"]["document"]["providers"][0]["credentials"][0]["luma_oauth"]
            ["client_id"],
        "registered-client"
    );
    let response = support::send_console_json(
        &f.state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        body,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}
