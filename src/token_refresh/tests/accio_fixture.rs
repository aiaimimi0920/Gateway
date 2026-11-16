//! Reuse the Local authority and shared scheduler; only the compiled test wire origin is replaced.
use super::*;
use crate::console::{refresh_grants::*, RouteConfigRedisRevision};
use crate::protocol::accio_oauth::{GATEWAY, REFRESH_ENDPOINT, TEST_REFRESH_ENDPOINT};
use serde_json::{json, Value};

pub(super) async fn fixture(cookie: Option<&str>) -> LocalFixture {
    let fixture = LocalFixture::new(REFRESH_ENDPOINT).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let provider = &mut document.providers[0];
    provider.preset = Some("accio".into());
    provider.base_url = GATEWAY.into();
    let credential = &mut provider.credentials[0];
    credential.headers.clear();
    credential
        .headers
        .insert("utdid".into(), "synthetic-device".into());
    credential.accio_sgcookie = cookie.map(str::to_owned);
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    fixture
}

pub(super) fn response(expiry: &str, cookie: Option<&str>) -> String {
    let mut value = json!({"success":true,"data":{"accessToken":"new-access",
        "refreshToken":"new-refresh","expiresAt":expiry}});
    if let Some(cookie) = cookie {
        value["data"]["sgcookie"] = json!(cookie);
    }
    value.to_string()
}

pub(super) async fn refresh(fixture: &LocalFixture, mock: &MockOAuth, pending: &mut Pending) {
    TEST_REFRESH_ENDPOINT
        .scope(mock.url.clone(), fixture.refresh(pending))
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

pub(super) fn cache(cookie: Option<&str>) -> Value {
    let mut value = json!({"id":"account","kind":"account_credential","provider":"accio",
        "project_id":"synthetic-project","user_id":"synthetic-user",
        "api_key":"old-access","api_base_url":GATEWAY,
        "created_at":"2020-01-01T00:00:00Z","updated_at":"2020-01-01T00:00:00Z",
        "expires_at":"2020-01-01T00:00:00Z","quota_remaining_tokens":42,"unknown":"retained",
        "account_payload":{"preset":"accio","refresh_token":"old-refresh",
        "refresh_endpoint":REFRESH_ENDPOINT,"refresh_client_id":"fixture-client",
        "expires_at":"2020-01-01T00:00:00Z"}});
    if let Some(cookie) = cookie {
        value["account_payload"]["accio_sgcookie"] = json!(cookie);
    }
    value
}
