//! Real local publication/recovery retains mutable Vertex scope/deadline without another token exchange.
use super::{vertex_fixture::*, *};
use crate::console::refresh_grants::*;
use serde_json::json;

#[tokio::test]
async fn vertex_oauth_scheduler_single_flight_publishes_mutable_context_and_restarts() {
    let mut f = fixture().await;
    let original = request(&f).1;
    let snapshot = f.runtime.route_config().snapshot();
    let old = snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap();
    let mock = MockOAuth::new(200, &response(), 20).await;
    let (mut a, mut b) = (Pending::new(), Pending::new());
    tokio::join!(refresh(&f, &mock, &mut a), refresh(&f, &mock, &mut b));
    assert!(a.is_empty() && b.is_empty());
    assert_eq!(mock.count(), 1);
    let saved = f.saved();
    assert_eq!(saved.api_key.as_deref(), Some("new-vertex-access"));
    assert_eq!(saved.refresh_token.as_deref(), Some("new-vertex-refresh"));
    let context = saved.vertex_oauth.as_ref().unwrap();
    assert!(context.scope_reported());
    assert!(
        context.refresh_expires_at().unwrap().unwrap()
            < time::OffsetDateTime::now_utc() + time::Duration::seconds(7201)
    );
    assert_eq!(load(&f, &original).phase(), GrantPhase::Published);
    let current = f.runtime.route_config().snapshot();
    assert!(Arc::ptr_eq(
        &old,
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
    assert_eq!(f.saved().vertex_oauth, saved.vertex_oauth);
    assert_eq!(f.saved().expires_at, saved.expires_at);
    let candidates = f
        .runtime
        .route_config()
        .resolve_candidates(Some("test-model"));
    let payload = &candidates[0].payload;
    assert_eq!(payload.api_key, "new-vertex-access");
    assert_eq!(payload.headers["x-goog-user-project"], "quota-a");
    let wire = serde_json::to_string(payload).unwrap();
    assert!(!wire.contains("private-vertex-client") && !wire.contains("new-vertex-refresh"));
    refresh(&f, &mock, &mut a).await;
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn vertex_oauth_unacknowledged_result_retries_disk_only() {
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
    assert!(f.saved().vertex_oauth.unwrap().scope_reported());
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn vertex_oauth_target_change_retires_late_result_but_not_consumption_evidence() {
    let f = fixture().await;
    let original = request(&f).1;
    let mock = MockOAuth::new(200, &response(), 150).await;
    let mut pending = Pending::new();
    tokio::join!(refresh(&f, &mock, &mut pending), async {
        mock.wait_called().await;
        let snapshot = f.runtime.route_config().snapshot();
        let mut value = serde_json::to_value(snapshot.document()).unwrap();
        value["providers"][0]["base_url"] = json!("https://aiplatform.googleapis.com/v1/projects/project-b/locations/global/publishers/google");
        value["providers"][0]["credentials"][0]["vertex_oauth"]["target"]["project"] =
            json!("project-b");
        f.runtime
            .commit_document(
                snapshot.revision().id(),
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
    assert_eq!(
        mock.count(),
        1,
        "editing the target cannot replay an already consumed RT family"
    );
}

#[tokio::test]
async fn vertex_oauth_durable_result_replays_exact_mutable_context_without_network() {
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
    next["scope_reported"] = json!(true);
    next["refresh_expires_at"] = json!("2080-01-01T00:00:00.123456789Z");
    let next = serde_json::from_value(next).unwrap();
    let result = request
        .result_with_vertex(
            "replayed-access",
            Some("replayed-refresh"),
            1,
            "2090-01-01T00:00:00.12Z",
            None,
            None,
            Some(next),
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
    let saved = f.saved();
    assert_eq!(
        serde_json::to_value(saved.vertex_oauth).unwrap(),
        serde_json::to_value(result.material.vertex_oauth).unwrap()
    );
    assert_eq!(saved.expires_at.as_deref(), Some("2090-01-01T00:00:00.12Z"));
    assert_eq!(load(&f, &request).phase(), GrantPhase::Published);
}
