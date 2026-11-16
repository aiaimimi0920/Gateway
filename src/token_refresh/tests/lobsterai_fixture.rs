//! Real local journal and synthetic loopback refresh; TEMP is controlled by the test runner.
use super::*;
pub(super) use crate::console::refresh_grants::*;
pub(super) use crate::protocol::lobsterai::{
    self as protocol,
    test_support::{Peer, Reply},
    ReceivedTokens,
};
pub(super) use serde_json::json;
pub(super) async fn fixture() -> LocalFixture {
    let f = LocalFixture::new(protocol::TOKEN_ENDPOINT).await;
    let snapshot = f.runtime.route_config().snapshot();
    let mut doc = snapshot.document().clone();
    let p = &mut doc.providers[0];
    p.base_url = crate::routing::config::lobsterai::BASE.into();
    p.adapter = Some("openai_compatible".into());
    p.auth_mode = Some(protocol::AUTH_MODE.into());
    p.explicit_credentials_only = true;
    let tokens = ReceivedTokens::from_json(
        crate::protocol::lobsterai::test_support::tokens()
            .to_string()
            .as_bytes(),
    )
    .unwrap()
    .admit_initial(uuid::Uuid::new_v4())
    .unwrap();
    let c = &mut p.credentials[0];
    c.api_key = Some(tokens.access_token);
    c.refresh_token = Some(tokens.refresh_token);
    c.lobsterai = Some(tokens.context);
    c.headers.clear();
    c.expires_at = None;
    c.token_expires_in_secs = None;
    c.refresh_endpoint = None;
    c.refresh_client_id = None;
    f.runtime
        .commit_document(snapshot.revision().id(), doc, None)
        .await
        .unwrap();
    f
}
pub(super) fn owner(f: &LocalFixture) -> Arc<crate::routing::config::TokenRefreshState> {
    f.runtime
        .route_config()
        .snapshot()
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap()
}
pub(super) async fn refresh(f: &LocalFixture, peer: &Peer, pending: &mut Pending) {
    tokio::time::timeout(
        Duration::from_secs(5),
        protocol::transport::TEST_ORIGIN.scope(peer.origin.clone(), f.refresh(pending)),
    )
    .await
    .unwrap();
}
pub(super) async fn ready(f: &LocalFixture, peer: &Peer) {
    refresh(f, peer, &mut Pending::new()).await;
    owner(f).dispatch_readiness().unwrap();
}
pub(super) fn reject(f: &LocalFixture) {
    let owner = owner(f);
    assert!(owner.reject_lobster_access(
        owner.lobster_dispatch_generation().unwrap(),
        f.saved().api_key.as_deref().unwrap(),
        401
    ));
}
pub(super) fn reply() -> serde_json::Value {
    json!({"code":0,"data":{"accessToken":"lobster-new-access"}})
}
pub(super) fn request(f: &LocalFixture) -> GrantRequest {
    super::nous_fixture::request(f)
}
pub(super) fn load(f: &LocalFixture, request: &GrantRequest) -> GrantRecord {
    super::nous_fixture::load(f, request)
}
pub(super) async fn reserve(f: &LocalFixture, request: &GrantRequest) -> GrantRecord {
    let snapshot = f.runtime.route_config().snapshot();
    let revision = crate::console::RouteConfigRedisRevision::new(
        snapshot.revision().clone(),
        snapshot.document().clone(),
    )
    .unwrap();
    let GrantReservation::Reserved(record) = f
        .runtime
        .reserve_refresh_grant(&revision, request)
        .await
        .unwrap()
    else {
        panic!("reserve")
    };
    record
}
pub(super) async fn restart(f: &mut LocalFixture) {
    f.runtime = RouteConfigRuntime::new_local(&f.config).unwrap();
    f.runtime.recover_startup().await.unwrap();
}
