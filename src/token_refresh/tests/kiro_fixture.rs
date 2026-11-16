//! Synthetic Kiro registration reuses the Local authority and protocol's joined loopback peer.
use super::*;
use crate::console::{refresh_grants::*, RouteConfigRedisRevision};
pub(super) use crate::protocol::kiro_oauth::fixture::{Mock, ResponseSpec};
use crate::protocol::kiro_oauth::TEST_ORIGIN;
use serde_json::{json, Value};

pub(super) const ENDPOINT: &str = "https://oidc.us-east-1.amazonaws.com/token";
pub(super) const BASE: &str = "https://q.us-east-1.amazonaws.com";
pub(super) fn context() -> Value {
    json!({"auth_method":"builder_id","directory_region":"us-east-1",
        "start_url":"https://view.awsapps.com/start","client_id":"synthetic-client",
        "client_secret":"synthetic-registered-secret","client_secret_expires_at":1893456000i64,
        "scopes":["codewhisperer:conversations"],"machine_id":"a".repeat(64)})
}
pub(super) async fn fixture() -> LocalFixture {
    let fixture = LocalFixture::new(ENDPOINT).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let provider = &mut document.providers[0];
    provider.preset = Some("kiro".into());
    provider.base_url = BASE.into();
    provider.explicit_credentials_only = true;
    let credential = &mut provider.credentials[0];
    credential.headers.clear();
    credential.refresh_endpoint = None;
    credential.refresh_client_id = None;
    credential.kiro_oauth = Some(serde_json::from_value(context()).unwrap());
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    fixture
}
pub(super) fn response(rotate: bool) -> ResponseSpec {
    let mut body = json!({"accessToken":"new-access","tokenType":"Bearer","expiresIn":3600});
    if rotate {
        body["refreshToken"] = json!("new-refresh");
    }
    ResponseSpec::json(200, body)
}
pub(super) async fn refresh(fixture: &LocalFixture, mock: &Mock, pending: &mut Pending) {
    TEST_ORIGIN
        .scope(mock.origin.clone(), fixture.refresh(pending))
        .await;
}
pub(super) fn request(fixture: &LocalFixture) -> (RouteConfigRedisRevision, GrantRequest) {
    let snapshot = fixture.runtime.route_config().snapshot();
    let revision =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let request = GrantRequest::from_revision(&revision, "oauth", "account").unwrap();
    (revision, request)
}
pub(super) fn persistence(fixture: &LocalFixture) -> RouteConfigPersistence {
    RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file).unwrap()
}
pub(super) fn load(fixture: &LocalFixture, request: &GrantRequest) -> GrantRecord {
    let path = persistence(fixture)
        .state_root()
        .join("refresh-grants")
        .join(request.family_id())
        .join(format!("{}.json", request.material_id()));
    GrantRecord::from_json(&std::fs::read_to_string(path).unwrap()).unwrap()
}
pub(super) fn owner(fixture: &LocalFixture) -> Arc<crate::routing::config::TokenRefreshState> {
    fixture
        .runtime
        .route_config()
        .snapshot()
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap()
}
pub(super) fn cache() -> Value {
    json!({"id":"account","kind":"account_credential","provider":"kiro",
        "project_id":"synthetic-project","user_id":"synthetic-user",
        "api_key":"old-access","api_base_url":BASE,
        "created_at":"2020-01-01T00:00:00Z","updated_at":"2020-01-01T00:00:00Z",
        "expires_at":"2020-01-01T00:00:00Z","quota_remaining_tokens":42,"unknown":"retained",
        "account_payload":{"preset":"kiro","refresh_token":"old-refresh",
        "kiro_oauth":context(),"expires_at":"2020-01-01T00:00:00Z"}})
}
