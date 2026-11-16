//! Reuse the actual local authority, disk journal and scheduler with synthetic DIM material.
use super::*;
use crate::console::{refresh_grants::*, RouteConfigRedisRevision};
use serde_json::{json, Value};

pub(super) fn context() -> Value {
    crate::protocol::dim_oauth::test_support::context()
}
pub(super) async fn fixture() -> LocalFixture {
    let f = LocalFixture::new("https://dimagent.cn/token").await;
    let snapshot = f.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let p = &mut document.providers[0];
    p.auth_mode = Some(crate::protocol::dim_oauth::business::AUTH_MODE.into());
    p.base_url = "https://dimagent.cn/v1".into();
    p.explicit_credentials_only = true;
    let c = &mut p.credentials[0];
    c.headers.clear();
    c.refresh_endpoint = None;
    c.refresh_client_id = None;
    c.dim_oauth = Some(serde_json::from_value(context()).unwrap());
    f.runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    f
}
pub(super) fn request(f: &LocalFixture) -> (RouteConfigRedisRevision, GrantRequest) {
    let s = f.runtime.route_config().snapshot();
    let revision =
        RouteConfigRedisRevision::new(s.revision().clone(), s.document().clone()).unwrap();
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
    json!({"access_token":"new-dim-access","refresh_token":"new-dim-refresh","token_type":"Bearer",
        "expires_in":3600,"scope":"openid profile"})
    .to_string()
}
pub(super) async fn refresh(f: &LocalFixture, mock: &MockOAuth, pending: &mut Pending) {
    crate::protocol::dim_oauth::test_support::origin(mock.url.clone(), f.refresh(pending)).await
}
