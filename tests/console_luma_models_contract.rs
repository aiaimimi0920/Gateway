//! Management model discovery rejects unsafe requests before any external call.
use axum::http::{Method, StatusCode};
use serde_json::json;
#[path = "fixtures/luma_mcp.rs"]
mod fixture;
#[path = "console_contract_support/mod.rs"]
mod support;
const PATH: &str = "/v1/internal/gateway/console/luma-mcp-models";

#[tokio::test]
async fn luma_models_require_consent_and_disallow_generic_tools() {
    let f = support::ConsoleStateFixture::new(fixture::document(fixture::provider()), true);
    let body = json!({"providerId":"luma-mcp","credentialId":"saved-mcp",
        "expectedRevision":f.state.route_config.snapshot().revision().id(),"action":"list"});
    let response =
        support::send_console_json(&f.state, Method::POST, PATH, body.clone(), None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let grant = support::grant_console_secret_access(&f.state).await;
    for (key, value) in [
        ("toolName", "luma_generate_image"),
        ("baseUrl", "https://invalid"),
        ("session_id", "other"),
        ("action", "generate"),
        ("model", "unexpected"),
    ] {
        let mut input = body.clone();
        input[key] = json!(value);
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
async fn luma_models_reject_revision_and_expired_account_before_network() {
    let mut provider = fixture::provider();
    provider["credentials"][0]["luma_oauth"]["expires_at"] = json!("2020-01-01T00:00:00Z");
    let f = support::ConsoleStateFixture::new(fixture::document(provider), true);
    let grant = support::grant_console_secret_access(&f.state).await;
    for (revision, expected) in [
        ("stale".to_owned(), StatusCode::CONFLICT),
        (
            f.state.route_config.snapshot().revision().id().to_owned(),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = support::send_console_json(&f.state, Method::POST, PATH,
            json!({"providerId":"luma-mcp","credentialId":"saved-mcp","expectedRevision":revision,"action":"list"}), Some(&grant), None).await;
        assert_eq!(response.status(), expected);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}
