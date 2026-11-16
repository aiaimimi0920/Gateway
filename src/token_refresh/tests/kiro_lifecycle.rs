//! Actual scheduler uses one AWS JSON grant and the existing durable publication/restart owners.
use super::{kiro_fixture::*, *};
use crate::console::refresh_grants::*;
use serde_json::{json, Value};

#[tokio::test]
async fn kiro_scheduler_single_flight_persists_context_and_absolute_expiry_through_restart() {
    for rotate in [false, true] {
        let mut fixture = fixture().await;
        let original = request(&fixture).1;
        let initial = fixture.runtime.route_config().snapshot();
        let initial_headers = crate::upstream::headers::build_upstream_headers(
            &initial
                .oauth_refresh_target("oauth", "account")
                .unwrap()
                .1
                .payload,
        );
        let mock = Mock::new(vec![response(rotate)]).await;
        let (mut a, mut b) = (Pending::new(), Pending::new());
        let before = time::OffsetDateTime::now_utc();
        tokio::join!(
            refresh(&fixture, &mock, &mut a),
            refresh(&fixture, &mock, &mut b)
        );
        assert!(a.is_empty() && b.is_empty());
        assert_eq!(mock.count(), 1);
        let saved = fixture.saved();
        assert_eq!(saved.api_key.as_deref(), Some("new-access"));
        assert_eq!(
            saved.refresh_token.as_deref(),
            Some(if rotate { "new-refresh" } else { "old-refresh" })
        );
        assert_eq!(serde_json::to_value(&saved.kiro_oauth).unwrap(), context());
        let expiry = time::OffsetDateTime::parse(
            saved.expires_at.as_deref().unwrap(),
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap();
        assert!(expiry >= before + time::Duration::seconds(3599));
        assert!(expiry <= time::OffsetDateTime::now_utc() + time::Duration::seconds(3600));
        {
            let calls = mock.calls.lock().unwrap();
            let (path, headers, body) = &calls[0];
            assert_eq!(path, "/token");
            assert_eq!(headers["content-type"], "application/json");
            assert!(!headers.contains_key("authorization"));
            assert_eq!(
                serde_json::from_slice::<Value>(body).unwrap(),
                json!({"clientId":"synthetic-client",
                "clientSecret":"synthetic-registered-secret","grantType":"refresh_token","refreshToken":"old-refresh"})
            );
        }
        assert_eq!(load(&fixture, &original).phase(), GrantPhase::Published);
        fixture.runtime = RouteConfigRuntime::new_local(&fixture.config).unwrap();
        fixture.runtime.recover_startup().await.unwrap();
        assert_eq!(fixture.saved().kiro_oauth, saved.kiro_oauth);
        assert_eq!(fixture.saved().expires_at, saved.expires_at);
        let candidates = fixture
            .runtime
            .route_config()
            .resolve_candidates(Some("test-model"));
        assert_eq!(candidates[0].payload.api_key, "new-access");
        let headers = crate::upstream::headers::build_upstream_headers(&candidates[0].payload);
        assert_eq!(headers["authorization"], "Bearer new-access");
        assert_eq!(headers["user-agent"], initial_headers["user-agent"]);
        assert_eq!(
            headers["x-amz-user-agent"],
            initial_headers["x-amz-user-agent"]
        );
        assert!(headers["user-agent"]
            .to_str()
            .unwrap()
            .contains(&"a".repeat(64)));
        assert_eq!(
            candidates[0].payload.extra_body.as_ref().unwrap()["kiroMachineId"],
            "a".repeat(64)
        );
        let business = serde_json::to_string(&candidates[0].payload).unwrap();
        assert!(
            !business.contains("synthetic-registered-secret") && !business.contains("new-refresh")
        );
        refresh(&fixture, &mock, &mut a).await;
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn kiro_unacknowledged_result_retries_disk_only_without_applying_or_exchanging_again() {
    let fixture = fixture().await;
    let mut reply = response(true);
    reply.header_delay = Duration::from_millis(150);
    let mock = Mock::new(vec![reply]).await;
    let mut pending = Pending::new();
    let (_, guard) = tokio::join!(refresh(&fixture, &mock, &mut pending), async {
        mock.wait_count(1).await;
        fixture.writer_lock()
    });
    assert_eq!(pending.len(), 1);
    assert_eq!(fixture.saved().api_key.as_deref(), Some("old-access"));
    refresh(&fixture, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
    drop(guard);
    refresh(&fixture, &mock, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(fixture.saved().api_key.as_deref(), Some("new-access"));
    assert_eq!(
        serde_json::to_value(fixture.saved().kiro_oauth).unwrap(),
        context()
    );
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn kiro_policy_edit_reuses_owner_but_registration_change_retires_late_result() {
    let fixture = fixture().await;
    let original = request(&fixture).1;
    let old = owner(&fixture);
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    document.providers[0].label = Some("policy only".into());
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&old, &owner(&fixture)));
    let mut reply = response(true);
    reply.header_delay = Duration::from_millis(150);
    let mock = Mock::new(vec![reply]).await;
    let mut pending = Pending::new();
    tokio::join!(refresh(&fixture, &mock, &mut pending), async {
        mock.wait_count(1).await;
        let snapshot = fixture.runtime.route_config().snapshot();
        let mut value = serde_json::to_value(snapshot.document()).unwrap();
        value["providers"][0]["credentials"][0]["kiro_oauth"]["scopes"] =
            json!(["codewhisperer:analysis"]);
        fixture
            .runtime
            .commit_document(
                snapshot.revision().id(),
                serde_json::from_value(value).unwrap(),
                None,
            )
            .await
            .unwrap();
    });
    assert!(!old.is_active());
    assert!(pending.is_empty());
    assert_eq!(fixture.saved().api_key.as_deref(), Some("old-access"));
    assert_eq!(load(&fixture, &original).phase(), GrantPhase::Result);
    refresh(&fixture, &mock, &mut pending).await;
    assert_eq!(
        mock.count(),
        1,
        "scope edit cannot bypass the consumed RT family"
    );
    mock.stop().await;
}

#[tokio::test]
async fn kiro_sending_fence_survives_registration_metadata_changes() {
    let fixture = fixture().await;
    let (revision, original) = request(&fixture);
    let persistence = persistence(&fixture);
    let guard = persistence.try_writer_lock().unwrap();
    assert!(matches!(
        persistence
            .reserve_refresh_grant_locked(&guard, &revision, &original)
            .unwrap(),
        GrantReservation::Reserved(_)
    ));
    drop(guard);
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut value = serde_json::to_value(snapshot.document()).unwrap();
    value["providers"][0]["credentials"][0]["kiro_oauth"]["client_secret_expires_at"] =
        json!(1893456001i64);
    fixture
        .runtime
        .commit_document(
            snapshot.revision().id(),
            serde_json::from_value(value).unwrap(),
            None,
        )
        .await
        .unwrap();
    let (revision, next) = request(&fixture);
    assert_eq!(original.family_id(), next.family_id());
    assert_ne!(original.material_id(), next.material_id());
    let guard = persistence.try_writer_lock().unwrap();
    assert!(matches!(
        persistence
            .reserve_refresh_grant_locked(&guard, &revision, &next)
            .unwrap(),
        GrantReservation::Blocked
    ));
}

#[tokio::test]
async fn kiro_saved_result_replays_exact_context_and_expiry_without_another_exchange() {
    for expiry in ["2099-01-01T00:00:00.12Z", "2021-01-01T00:00:00Z"] {
        let mut fixture = fixture().await;
        let (revision, request) = request(&fixture);
        let persistence = persistence(&fixture);
        let guard = persistence.try_writer_lock().unwrap();
        let GrantReservation::Reserved(record) = persistence
            .reserve_refresh_grant_locked(&guard, &revision, &request)
            .unwrap()
        else {
            panic!("reserve")
        };
        let result = request
            .result("replayed-access", Some("replayed-refresh"), 1, expiry, None)
            .unwrap();
        persistence
            .store_refresh_grant_result_locked(&guard, &record, &result)
            .unwrap();
        drop(guard);
        fixture.runtime = RouteConfigRuntime::new_local(&fixture.config).unwrap();
        fixture.runtime.recover_startup().await.unwrap();
        let mock = Mock::new(vec![]).await;
        refresh(&fixture, &mock, &mut Pending::new()).await;
        assert_eq!(mock.count(), 0);
        assert_eq!(fixture.saved().api_key.as_deref(), Some("replayed-access"));
        assert_eq!(
            serde_json::to_value(fixture.saved().kiro_oauth).unwrap(),
            context()
        );
        assert_eq!(fixture.saved().expires_at.as_deref(), Some(expiry));
        assert_eq!(load(&fixture, &request).phase(), GrantPhase::Published);
        assert_eq!(
            owner(&fixture).dispatch_readiness().is_ok(),
            expiry.starts_with("2099")
        );
        mock.stop().await;
    }
}

#[tokio::test]
async fn kiro_missing_access_expiry_is_not_published_or_retried_as_generic_form() {
    let fixture = fixture().await;
    let original = request(&fixture).1;
    let mock = Mock::new(vec![ResponseSpec::json(
        200,
        json!({"accessToken":"unknown-expiry","tokenType":"Bearer"}),
    )])
    .await;
    let mut pending = Pending::new();
    refresh(&fixture, &mock, &mut pending).await;
    assert_eq!(fixture.saved().api_key.as_deref(), Some("old-access"));
    assert_eq!(load(&fixture, &original).phase(), GrantPhase::Sending);
    refresh(&fixture, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}
