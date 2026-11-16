//! Full no-credential scheduler rotation, Local persistence and generation ownership.
use super::{accio_fixture::*, *};
use crate::console::refresh_grants::GrantPhase;

#[tokio::test]
async fn accio_scheduler_rotates_preserves_or_first_sets_cookie_and_absolute_expiry() {
    for (previous, returned, expected) in [
        (None, None, None),
        (None, Some("first-cookie"), Some("first-cookie")),
        (Some("old-cookie"), None, Some("old-cookie")),
        (
            Some("old-cookie"),
            Some("rotated-cookie"),
            Some("rotated-cookie"),
        ),
    ] {
        let mut fixture = fixture(previous).await;
        let original = request(&fixture).1;
        let expiry = "2099-01-01T00:00:00.12Z";
        let mock = MockOAuth::new(200, &response(expiry, returned), 0).await;
        let mut pending = Pending::new();
        refresh(&fixture, &mock, &mut pending).await;
        assert!(pending.is_empty());
        assert_eq!(mock.count(), 1);
        let saved = fixture.saved();
        assert_eq!(saved.api_key.as_deref(), Some("new-access"));
        assert_eq!(saved.refresh_token.as_deref(), Some("new-refresh"));
        assert_eq!(saved.accio_sgcookie.as_deref(), expected);
        assert_eq!(saved.expires_at.as_deref(), Some(expiry));
        assert_eq!(saved.headers["utdid"], "synthetic-device");
        assert_eq!(load(&fixture, &original).phase(), GrantPhase::Published);
        fixture.runtime = RouteConfigRuntime::new_local(&fixture.config).unwrap();
        fixture.runtime.recover_startup().await.unwrap();
        assert_eq!(fixture.saved().accio_sgcookie.as_deref(), expected);
        assert_eq!(fixture.saved().expires_at.as_deref(), Some(expiry));
        let candidates = fixture
            .runtime
            .route_config()
            .resolve_candidates(Some("test-model"));
        assert_eq!(candidates[0].payload.api_key, "new-access");
        let payload = serde_json::to_string(&candidates[0].payload).unwrap();
        assert!(!payload.contains("sgcookie"));
        refresh(&fixture, &mock, &mut pending).await;
        assert_eq!(
            mock.count(),
            1,
            "restart does not grant a new expiry or exchange"
        );
    }
}

#[tokio::test]
async fn accio_pending_cookie_waits_for_durable_result_then_retries_disk_only() {
    let fixture = fixture(Some("old-cookie")).await;
    let mock = MockOAuth::new(
        200,
        &response("2099-01-01T00:00:00Z", Some("new-cookie")),
        100,
    )
    .await;
    let mut pending = Pending::new();
    let (_, guard) = tokio::join!(refresh(&fixture, &mock, &mut pending), async {
        mock.wait_called().await;
        fixture.writer_lock()
    });
    assert_eq!(pending.len(), 1);
    assert_eq!(
        fixture.saved().accio_sgcookie.as_deref(),
        Some("old-cookie")
    );
    assert_eq!(
        fixture
            .runtime
            .route_config()
            .resolve_candidates(Some("test-model"))[0]
            .payload
            .api_key,
        "old-access"
    );
    refresh(&fixture, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
    drop(guard);
    refresh(&fixture, &mock, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(
        fixture.saved().accio_sgcookie.as_deref(),
        Some("new-cookie")
    );
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn accio_cookie_only_reauthorization_revokes_the_old_generation_and_preserves_late_result() {
    let fixture = fixture(Some("old-cookie")).await;
    let original = request(&fixture).1;
    let owner = fixture
        .runtime
        .route_config()
        .snapshot()
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap();
    let mock = MockOAuth::new(
        200,
        &response("2099-01-01T00:00:00Z", Some("late-cookie")),
        100,
    )
    .await;
    let mut pending = Pending::new();
    tokio::join!(refresh(&fixture, &mock, &mut pending), async {
        mock.wait_called().await;
        let snapshot = fixture.runtime.route_config().snapshot();
        let mut document = snapshot.document().clone();
        document.providers[0].credentials[0].accio_sgcookie = Some("reauthorized-cookie".into());
        fixture
            .runtime
            .commit_document(snapshot.revision().id(), document, None)
            .await
            .unwrap();
    });
    assert!(pending.is_empty());
    assert!(!owner.is_active());
    assert_eq!(fixture.saved().api_key.as_deref(), Some("old-access"));
    assert_eq!(
        fixture.saved().accio_sgcookie.as_deref(),
        Some("reauthorized-cookie")
    );
    assert_eq!(load(&fixture, &original).phase(), GrantPhase::Result);
    refresh(&fixture, &mock, &mut pending).await;
    assert_eq!(
        mock.count(),
        1,
        "a new SG must not bypass the same refresh-family fence"
    );
}
