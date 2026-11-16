//! Recovery preserves absolute expiry and revocation, even when result persistence was interrupted.
use super::*;
use crate::console::{refresh_grants::*, RouteConfigRedisRevision};

fn request(fixture: &LocalFixture) -> (RouteConfigRedisRevision, GrantRequest) {
    let snapshot = fixture.runtime.route_config().snapshot();
    let revision =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let request = GrantRequest::from_revision(&revision, "oauth", "account").unwrap();
    (revision, request)
}

fn load(fixture: &LocalFixture, request: &GrantRequest) -> GrantRecord {
    let persistence =
        RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file)
            .unwrap();
    let path = persistence
        .state_root()
        .join("refresh-grants")
        .join(request.family_id())
        .join(format!("{}.json", request.material_id()));
    GrantRecord::from_json(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[tokio::test]
async fn cancellation_after_send_cannot_authorize_another_exchange_after_restart() {
    let mock = MockOAuth::new(200, ROTATED, 500).await;
    let mut fixture = LocalFixture::new(&mock.url).await;
    let mut pending = Pending::new();
    {
        let refresh = fixture.refresh(&mut pending);
        tokio::pin!(refresh);
        tokio::select! {
            _ = mock.wait_called() => {},
            _ = &mut refresh => panic!("exchange must still be in flight"),
        }
    }
    fixture.runtime = RouteConfigRuntime::new_local(&fixture.config).unwrap();
    fixture.runtime.recover_startup().await.unwrap();
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(
        load(&fixture, &request(&fixture).1).phase(),
        GrantPhase::Sending
    );
}

#[tokio::test]
async fn expired_saved_result_does_not_receive_a_new_runtime_lifetime() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let (revision, request) = request(&fixture);
    let persistence =
        RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file)
            .unwrap();
    let guard = persistence.try_writer_lock().unwrap();
    let GrantReservation::Reserved(record) = persistence
        .reserve_refresh_grant_locked(&guard, &revision, &request)
        .unwrap()
    else {
        panic!("reserve")
    };
    let expiry = "2021-01-01T00:00:00Z";
    let result = request
        .result(
            "expired-access",
            Some("expired-refresh"),
            3600,
            expiry,
            None,
        )
        .unwrap();
    persistence
        .store_refresh_grant_result_locked(&guard, &record, &result)
        .unwrap();
    drop(guard);
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(mock.count(), 0);
    assert_eq!(fixture.saved().expires_at.as_deref(), Some(expiry));
    let snapshot = fixture.runtime.route_config().snapshot();
    let owner = snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .as_ref()
        .unwrap();
    assert!(
        owner.refresh_snapshot(Duration::ZERO).is_some(),
        "replayed expired access remains due"
    );
    assert_eq!(fixture.saved().token_expires_in_secs, Some(3600));
}

#[tokio::test]
async fn late_revoked_result_is_still_saved_before_pending_is_discarded() {
    let mock = MockOAuth::new(200, ROTATED, 100).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let original = request(&fixture).1;
    let mut pending = Pending::new();
    let (_, guard) = tokio::join!(fixture.refresh(&mut pending), async {
        mock.wait_called().await;
        let snapshot = fixture.runtime.route_config().snapshot();
        let mut document = snapshot.document().clone();
        let credential = &mut document.providers[0].credentials[0];
        credential.api_key = Some("reauthorized-access".into());
        credential.refresh_token = Some("reauthorized-refresh".into());
        credential.expires_at = Some("2099-01-01T00:00:00Z".into());
        fixture
            .runtime
            .commit_document(snapshot.revision().id(), document, None)
            .await
            .unwrap();
        fixture.writer_lock()
    });
    assert_eq!(
        pending.len(),
        1,
        "revocation must not erase the Result storage obligation"
    );
    assert_eq!(load(&fixture, &original).phase(), GrantPhase::Sending);
    drop(guard);
    fixture.refresh(&mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(load(&fixture, &original).phase(), GrantPhase::Result);
    assert_eq!(
        fixture.saved().api_key.as_deref(),
        Some("reauthorized-access")
    );
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn same_refresh_token_in_two_credential_ids_is_not_two_grant_budgets() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let mut second = document.providers[0].credentials[0].clone();
    second.id = Some("copied-account".into());
    document.providers[0].credentials.push(second);
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(
        fixture
            .runtime
            .route_config()
            .snapshot()
            .document()
            .providers[0]
            .credentials[1]
            .api_key
            .as_deref(),
        Some("old-access")
    );
}

#[tokio::test]
async fn inherited_or_empty_generic_access_and_unknown_expiry_remain_refreshable() {
    for mode in ["inherited", "empty"] {
        let mock = MockOAuth::new(200, ROTATED, 0).await;
        let fixture = LocalFixture::new(&mock.url).await;
        let snapshot = fixture.runtime.route_config().snapshot();
        let mut document = snapshot.document().clone();
        document.providers[0].api_key = "provider-access".into();
        let credential = &mut document.providers[0].credentials[0];
        credential.api_key = (mode == "empty").then(String::new);
        credential.expires_at = Some("legacy-invalid-expiry".into());
        fixture
            .runtime
            .commit_document(snapshot.revision().id(), document, None)
            .await
            .unwrap();
        fixture.refresh(&mut Pending::new()).await;
        assert_eq!(
            mock.count(),
            1,
            "preserve pre-gate refresh material inheritance"
        );
        assert_eq!(fixture.saved().api_key.as_deref(), Some("new-access"));
    }
}

#[tokio::test]
async fn metadata_edits_cannot_unlock_an_unknown_refresh_family() {
    let mock = MockOAuth::new(503, "unknown", 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    fixture.refresh(&mut Pending::new()).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    document.providers[0].credentials[0].api_key = Some("changed-access".into());
    document.providers[0].credentials[0].expires_at = Some("2022-01-01T00:00:00Z".into());
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(fixture.saved().api_key.as_deref(), Some("changed-access"));
}

#[tokio::test]
async fn fractional_expiry_spelling_does_not_block_publication_or_same_token_successor() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let (revision, original) = request(&fixture);
    let persistence =
        RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file)
            .unwrap();
    let guard = persistence.try_writer_lock().unwrap();
    let GrantReservation::Reserved(record) = persistence
        .reserve_refresh_grant_locked(&guard, &revision, &original)
        .unwrap()
    else {
        panic!("reserve")
    };
    let result = original
        .result(
            "recovered-access",
            None,
            3600,
            "2099-01-01T00:00:00.120Z",
            None,
        )
        .unwrap();
    persistence
        .store_refresh_grant_result_locked(&guard, &record, &result)
        .unwrap();
    drop(guard);
    let mut pending = Pending::new();
    fixture.refresh(&mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 0);
    assert_eq!(load(&fixture, &original).phase(), GrantPhase::Published);
    let snapshot = fixture.runtime.route_config().snapshot();
    snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .as_ref()
        .unwrap()
        .force_due();
    fixture.refresh(&mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(
        mock.count(),
        1,
        "same instant and unchanged RT can send the next grant"
    );
    assert_eq!(
        fixture.saved().refresh_token.as_deref(),
        Some("new-refresh")
    );
}
