//! Saved Accio Result replays the original expiry and final effective SG without another grant.
use super::{accio_fixture::*, *};
use crate::console::refresh_grants::*;

#[tokio::test]
async fn accio_sg_only_edit_cannot_bypass_an_existing_sending_family() {
    let fixture = fixture(Some("old-cookie")).await;
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
    let mut document = snapshot.document().clone();
    document.providers[0].credentials[0].accio_sgcookie = Some("different-cookie".into());
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
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
async fn accio_saved_result_replays_cookie_and_absolute_expiry_after_restart() {
    for expiry in ["2099-01-01T00:00:00.12Z", "2021-01-01T00:00:00Z"] {
        let mut fixture = fixture(Some("old-cookie")).await;
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
            .result_with_accio(
                "replayed-access",
                Some("replayed-refresh"),
                1,
                expiry,
                None,
                Some("replayed-cookie".into()),
            )
            .unwrap();
        persistence
            .store_refresh_grant_result_locked(&guard, &record, &result)
            .unwrap();
        drop(guard);
        fixture.runtime = RouteConfigRuntime::new_local(&fixture.config).unwrap();
        fixture.runtime.recover_startup().await.unwrap();
        let mock = MockOAuth::new(200, &response("2099-01-01T00:00:00Z", None), 0).await;
        let mut pending = Pending::new();
        refresh(&fixture, &mock, &mut pending).await;
        assert!(pending.is_empty());
        assert_eq!(mock.count(), 0);
        assert_eq!(fixture.saved().api_key.as_deref(), Some("replayed-access"));
        assert_eq!(
            fixture.saved().accio_sgcookie.as_deref(),
            Some("replayed-cookie")
        );
        assert_eq!(fixture.saved().expires_at.as_deref(), Some(expiry));
        assert_eq!(load(&fixture, &request).phase(), GrantPhase::Published);
        let snapshot = fixture.runtime.route_config().snapshot();
        let owner = snapshot
            .oauth_refresh_target("oauth", "account")
            .unwrap()
            .1
            .refresh_config
            .as_ref()
            .unwrap();
        assert_eq!(
            owner.dispatch_readiness().is_ok(),
            expiry.starts_with("2099")
        );
    }
}

#[tokio::test]
async fn accio_fresh_but_expired_rotation_is_saved_without_extending_authorization() {
    let mut fixture = fixture(Some("old-cookie")).await;
    let original = request(&fixture).1;
    let expiry = "2021-01-01T00:00:00.123456789Z";
    let mock = MockOAuth::new(200, &response(expiry, Some("expired-cookie")), 0).await;
    let mut pending = Pending::new();
    refresh(&fixture, &mock, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(fixture.saved().expires_at.as_deref(), Some(expiry));
    assert_eq!(
        fixture.saved().accio_sgcookie.as_deref(),
        Some("expired-cookie")
    );
    assert_eq!(fixture.saved().token_expires_in_secs, Some(1));
    assert_eq!(load(&fixture, &original).phase(), GrantPhase::Published);
    fixture.runtime = RouteConfigRuntime::new_local(&fixture.config).unwrap();
    fixture.runtime.recover_startup().await.unwrap();
    let snapshot = fixture.runtime.route_config().snapshot();
    let owner = snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .as_ref()
        .unwrap();
    assert!(owner.dispatch_readiness().is_err());
    assert!(owner.refresh_snapshot(Duration::ZERO).is_some());
    assert_eq!(mock.count(), 1);
}
