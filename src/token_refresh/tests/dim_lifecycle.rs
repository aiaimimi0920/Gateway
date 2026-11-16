//! Exercise one-time consumption, durable rotation, late supersession and restart without live accounts.
use super::{dim_fixture::*, *};
use crate::console::refresh_grants::*;
use serde_json::json;

#[tokio::test]
async fn dim_scheduler_singleflight_persists_scopes_and_absolute_expiry_across_restart() {
    let mut f = fixture().await;
    // Routing requires an account-bound declaration; a refresh token alone is not model discovery.
    let snapshot = f.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let credential = &mut document.providers[0].credentials[0];
    credential.dim_catalog = Some(
        crate::protocol::dim_oauth::ModelCatalog::new(
            credential.dim_oauth.as_ref().unwrap(),
            crate::protocol::dim_oauth::parse_catalog(
                br#"{"data":[{"id":"test-model","dim":{"endpoint_type":"openai"}}]}"#,
            )
            .unwrap(),
        )
        .unwrap(),
    );
    f.runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    let original = request(&f).1;
    let snapshot = f.runtime.route_config().snapshot();
    let owner = snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap();
    let mock = MockOAuth::new(200, &response(), 20).await;
    let (mut a, mut b) = (Pending::new(), Pending::new());
    tokio::join!(refresh(&f, &mock, &mut a), refresh(&f, &mock, &mut b));
    assert_eq!(mock.count(), 1);
    assert!(a.is_empty() && b.is_empty());
    let saved = f.saved();
    assert_eq!(saved.api_key.as_deref(), Some("new-dim-access"));
    assert_eq!(saved.refresh_token.as_deref(), Some("new-dim-refresh"));
    let value = serde_json::to_value(saved.dim_oauth.as_ref()).unwrap();
    assert_eq!(value["granted_scopes"], json!(["openid", "profile"]));
    assert_eq!(load(&f, &original).phase(), GrantPhase::Published);
    let current = f.runtime.route_config().snapshot();
    assert!(Arc::ptr_eq(
        &owner,
        current
            .oauth_refresh_target("oauth", "account")
            .unwrap()
            .1
            .refresh_config
            .as_ref()
            .unwrap()
    ));
    f.runtime = RouteConfigRuntime::new_local(&f.config).unwrap();
    f.runtime.recover_startup().await.unwrap();
    assert_eq!(f.saved().dim_oauth, saved.dim_oauth);
    assert_eq!(f.saved().expires_at, saved.expires_at);
    let candidates = f
        .runtime
        .route_config()
        .resolve_candidates(Some("test-model"));
    assert_eq!(candidates[0].payload.api_key, "new-dim-access");
    let wire = serde_json::to_string(&candidates[0].payload).unwrap();
    assert!(!wire.contains("new-dim-refresh") && !wire.contains("synthetic-nonce"));
    refresh(&f, &mock, &mut a).await;
    assert_eq!(mock.count(), 1);
}
#[tokio::test]
async fn dim_rotation_unacknowledged_storage_retries_disk_only() {
    let f = fixture().await;
    let mock = MockOAuth::new(200, &response(), 150).await;
    let mut pending = Pending::new();
    let (_, guard) = tokio::join!(refresh(&f, &mock, &mut pending), async {
        mock.wait_called().await;
        f.writer_lock()
    });
    assert_eq!(pending.len(), 1);
    assert_eq!(f.saved().api_key.as_deref(), Some("old-access"));
    refresh(&f, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
    drop(guard);
    refresh(&f, &mock, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 1);
    assert_eq!(f.saved().refresh_token.as_deref(), Some("new-dim-refresh"));
}
#[tokio::test]
async fn dim_late_result_does_not_replace_superseding_subject_or_replay_token_family() {
    let f = fixture().await;
    let original = request(&f).1;
    let mock = MockOAuth::new(200, &response(), 150).await;
    let mut pending = Pending::new();
    tokio::join!(refresh(&f, &mock, &mut pending), async {
        mock.wait_called().await;
        let s = f.runtime.route_config().snapshot();
        let mut value = serde_json::to_value(s.document()).unwrap();
        value["providers"][0]["credentials"][0]["dim_oauth"]["subject"] =
            json!("superseding-subject");
        f.runtime
            .commit_document(
                s.revision().id(),
                serde_json::from_value(value).unwrap(),
                None,
            )
            .await
            .unwrap();
    });
    assert!(pending.is_empty());
    assert_eq!(f.saved().api_key.as_deref(), Some("old-access"));
    assert_eq!(load(&f, &original).phase(), GrantPhase::Result);
    refresh(&f, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
}
#[tokio::test]
async fn dim_durable_result_replays_narrowed_context_without_second_exchange() {
    let mut f = fixture().await;
    let (revision, request) = request(&f);
    let p = persistence(&f);
    let guard = p.try_writer_lock().unwrap();
    let GrantReservation::Reserved(record) = p
        .reserve_refresh_grant_locked(&guard, &revision, &request)
        .unwrap()
    else {
        panic!("reserve")
    };
    let mut next = context();
    next["granted_scopes"] = json!(["openid"]);
    let result = request
        .result_with_dim(
            "replayed-access",
            Some("replayed-refresh"),
            3600,
            "2090-01-01T00:00:00.12Z",
            None,
            None,
            None,
            Some(serde_json::from_value(next).unwrap()),
        )
        .unwrap();
    p.store_refresh_grant_result_locked(&guard, &record, &result)
        .unwrap();
    drop(guard);
    f.runtime = RouteConfigRuntime::new_local(&f.config).unwrap();
    f.runtime.recover_startup().await.unwrap();
    let mock = MockOAuth::new(500, "unexpected network", 0).await;
    refresh(&f, &mock, &mut Pending::new()).await;
    assert_eq!(mock.count(), 0);
    assert_eq!(f.saved().dim_oauth, result.material.dim_oauth);
    assert_eq!(
        f.saved().expires_at.as_deref(),
        Some("2090-01-01T00:00:00.12Z")
    );
    assert_eq!(load(&f, &request).phase(), GrantPhase::Published);
}
