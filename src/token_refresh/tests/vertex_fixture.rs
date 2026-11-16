//! Vertex synthetic grants use the existing local authority and bounded loopback token transport.
use super::*;
use crate::console::{refresh_grants::*, RouteConfigRedisRevision};
use serde_json::{json, Value};

pub(super) fn context() -> Value {
    json!({"grant_type":"desktop_authorization_code","client_id":"explicit-vertex-client",
        "client_secret":"private-vertex-client", "requested_scope":"https://www.googleapis.com/auth/cloud-platform",
        "scope_reported":false, "target":{"project":"project-a","location":"global","quota_project":"quota-a"},
        "refresh_expires_at":"2099-01-01T00:00:00.123456789Z"})
}
pub(super) async fn fixture() -> LocalFixture {
    let fixture = LocalFixture::new(crate::protocol::vertex_oauth::TOKEN_ENDPOINT).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let provider = &mut document.providers[0];
    provider.preset = Some("google-agent-platform-official-api".into());
    provider.auth_mode = Some(crate::protocol::vertex_oauth::business::AUTH_MODE.into());
    provider.base_url = "https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google".into();
    provider.explicit_credentials_only = true;
    let credential = &mut provider.credentials[0];
    credential.headers.clear();
    credential.refresh_endpoint = None;
    credential.refresh_client_id = None;
    credential.vertex_oauth = Some(serde_json::from_value(context()).unwrap());
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    fixture
}
pub(super) fn request(fixture: &LocalFixture) -> (RouteConfigRedisRevision, GrantRequest) {
    let snapshot = fixture.runtime.route_config().snapshot();
    let revision =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let request = GrantRequest::from_revision(&revision, "oauth", "account").unwrap();
    (revision, request)
}
pub(super) fn persistence(f: &LocalFixture) -> RouteConfigPersistence {
    RouteConfigPersistence::new(&f.config.state_dir, &f.config.routes_file).unwrap()
}
pub(super) fn load(f: &LocalFixture, request: &GrantRequest) -> GrantRecord {
    let path = persistence(f)
        .state_root()
        .join("refresh-grants")
        .join(request.family_id())
        .join(format!("{}.json", request.material_id()));
    GrantRecord::from_json(&std::fs::read_to_string(path).unwrap()).unwrap()
}
pub(super) fn response() -> String {
    json!({"access_token":"new-vertex-access","refresh_token":"new-vertex-refresh","token_type":"Bearer",
        "expires_in":3600, "refresh_token_expires_in":7200,
        "scope":"https://www.googleapis.com/auth/cloud-platform"}).to_string()
}
pub(super) async fn refresh(f: &LocalFixture, mock: &MockOAuth, pending: &mut Pending) {
    crate::protocol::vertex_oauth::transport_test_support::test_endpoint(
        mock.url.clone(),
        f.refresh(pending),
    )
    .await;
}
