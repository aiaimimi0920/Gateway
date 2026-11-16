//! Real management router and loopback listener; no credentials or Google requests.
use crate::{routing::config::RouteConfigYaml, state::AppState};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::sync::Arc;
mod competition;
mod failures;
mod fixture;
mod flow;
mod safety;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
#[path = "../../../../protocol/accio_oauth/transport_fixture.rs"]
mod transport_fixture;
const ROOT: &str = "/v1/internal/gateway/console/vertex-auth-sessions";
fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"vertex", "preset":"google-agent-platform-official-api",
        "auth_mode":"vertex_user_oauth","explicit_credentials_only":true,
        "base_url":"https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google",
        "credentials":[]}, {"id":"unrelated","base_url":"https://example.invalid/v1","api_key":"synthetic-other-secret"}]})).unwrap()
}
fn create_body(state: &AppState) -> Value {
    json!({"providerId":"vertex","expectedRevision":state.route_config.snapshot().revision().id(),
        "clientId":"explicit-vertex-client", "clientSecret":"synthetic-private-client",
        "target":{"project":"project-a","location":"global","quota_project":"quota-a"}})
}
async fn call(
    state: &Arc<AppState>,
    path: &str,
    body: Value,
    grant: Option<&str>,
) -> axum::response::Response {
    support::send_console_json(state, Method::POST, path, body, grant, None).await
}
#[tokio::test]
async fn vertex_oauth_management_create_binds_actual_listener_and_cancel_releases_port() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let response = call(state, ROOT, create_body(state), Some(&grant)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let value = support::parse_json(response).await;
    let view = &value["session"];
    assert_eq!(view["status"], "waiting_user");
    assert!(!value.to_string().contains("synthetic-private-client"));
    let redirect = url::Url::parse(view["redirectUri"].as_str().unwrap()).unwrap();
    let address = format!("127.0.0.1:{}", redirect.port().unwrap());
    assert!(tokio::net::TcpStream::connect(&address).await.is_ok());
    let path = format!("{ROOT}/{}/cancel", view["id"].as_str().unwrap());
    let result = call(
        state,
        &path,
        json!({"expectedRevision":view["expectedRevision"]}),
        Some(&grant),
    )
    .await;
    assert_eq!(
        support::parse_json(result).await["session"]["status"],
        "cancelled"
    );
    // Windows refused loopback connects themselves take about two seconds; this is
    // not the listener shutdown budget (covered separately through Registry::join).
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while tokio::net::TcpStream::connect(&address).await.is_ok() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
}
#[tokio::test]
async fn vertex_oauth_management_rejections_require_secret_grant_and_disable_caching() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let response = call(&fixture.state, ROOT, create_body(&fixture.state), None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let mut body = create_body(&fixture.state);
    body["tokenEndpoint"] = json!("https://untrusted.invalid");
    let response = call(&fixture.state, ROOT, body, Some(&grant)).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.headers()["cache-control"], "no-store");
}
