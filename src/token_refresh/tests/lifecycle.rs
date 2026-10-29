use super::*;

#[tokio::test]
async fn policy_change_retires_old_routing_but_preserves_rotated_material() {
    let mock = MockOAuth::new(200, ROTATED, 100).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let initial = fixture.runtime.route_config().snapshot();
    let old_owner = initial
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap();
    let mut pending = Pending::new();
    let edit = async {
        tokio::time::timeout(Duration::from_secs(2), async {
            while mock.count() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let snapshot = fixture.runtime.route_config().snapshot();
        let mut document = snapshot.document().clone();
        document.providers[0].credential_pool_auth_policy =
            crate::routing::config::CredentialPoolAuthPolicy::OAuthOnly;
        fixture
            .runtime
            .commit_document(snapshot.revision().id(), document, None)
            .await
            .unwrap();
        fixture.refresh(&mut Pending::new()).await;
    };
    tokio::join!(fixture.refresh(&mut pending), edit);
    assert!(!old_owner.is_active());
    assert_eq!(
        mock.count(),
        1,
        "routing replacement must share the in-flight authorization lock"
    );
    assert_eq!(
        fixture.saved().refresh_token.as_deref(),
        Some("new-refresh")
    );
    assert_eq!(
        fixture
            .runtime
            .route_config()
            .snapshot()
            .document()
            .providers[0]
            .credential_pool_auth_policy,
        crate::routing::config::CredentialPoolAuthPolicy::OAuthOnly
    );
}

#[tokio::test]
async fn server_cache_failure_retains_original_expected_material_for_retry() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let pool = deadpool_redis::Config::from_url("redis://127.0.0.1:1")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    pool.close(); // deterministic outage; no connection to any existing service.
    let mut pending = Pending::new();
    for _ in 0..2 {
        refresh_once(
            fixture.runtime.route_config(),
            &client(),
            None,
            Some(&pool),
            &mut pending,
            Duration::from_secs(300),
            None,
        )
        .await;
    }
    assert_eq!(mock.count(), 1);
    let saved = pending.values().next().unwrap();
    assert_eq!(saved.previous_access, "old-access");
    assert_eq!(saved.previous_refresh, "old-refresh");
    assert_eq!(saved.token.refresh_token.as_deref(), Some("new-refresh"));
}

#[tokio::test]
async fn stale_http_result_cannot_overwrite_reauthorization() {
    let mock = MockOAuth::new(200, ROTATED, 100).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let mut pending = Pending::new();
    let refresh = fixture.refresh(&mut pending);
    let edit = async {
        tokio::time::timeout(Duration::from_secs(2), async {
            while mock.count() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let snapshot = fixture.runtime.route_config().snapshot();
        let mut document = snapshot.document().clone();
        document.providers[0].credentials[0].api_key = Some("reauthorized-access".into());
        document.providers[0].credentials[0].refresh_token = Some("reauthorized-refresh".into());
        fixture
            .runtime
            .commit_document(snapshot.revision().id(), document, None)
            .await
            .unwrap();
    };
    tokio::join!(refresh, edit);
    assert!(pending.is_empty());
    assert_eq!(
        fixture.saved().api_key.as_deref(),
        Some("reauthorized-access")
    );
    assert_eq!(
        fixture.saved().refresh_token.as_deref(),
        Some("reauthorized-refresh")
    );
}

#[tokio::test]
async fn unrelated_provider_edit_during_exchange_preserves_rotated_material() {
    let mock = MockOAuth::new(200, ROTATED, 100).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let mut pending = Pending::new();
    let edit = async {
        tokio::time::timeout(Duration::from_secs(2), async {
            while mock.count() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let snapshot = fixture.runtime.route_config().snapshot();
        let mut document = snapshot.document().clone();
        document.providers[0].label = Some("Renamed while refreshing".into());
        fixture
            .runtime
            .commit_document(snapshot.revision().id(), document, None)
            .await
            .unwrap();
    };
    tokio::join!(fixture.refresh(&mut pending), edit);
    assert_eq!(
        fixture.saved().refresh_token.as_deref(),
        Some("new-refresh")
    );
    assert!(pending.is_empty());
    assert_eq!(
        fixture
            .runtime
            .route_config()
            .snapshot()
            .document()
            .providers[0]
            .label
            .as_deref(),
        Some("Renamed while refreshing")
    );
}

#[tokio::test]
async fn pending_runtime_override_survives_other_provider_setting_commit() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let persistence =
        RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file)
            .unwrap();
    let guard = persistence.try_writer_lock().unwrap();
    let mut pending = Pending::new();
    fixture.refresh(&mut pending).await;
    drop(guard);
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    document.providers[0].label = Some("Changed while persistence pending".into());
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    let candidates = fixture
        .runtime
        .route_config()
        .resolve_candidates(Some("test-model"));
    assert_eq!(candidates[0].payload.api_key, "new-access");
    fixture.refresh(&mut pending).await;
    assert_eq!(mock.count(), 1);
    assert!(pending.is_empty());
    assert_eq!(
        fixture.saved().refresh_token.as_deref(),
        Some("new-refresh")
    );
}

#[tokio::test]
async fn concurrent_cycles_make_only_one_refresh_request() {
    let mock = MockOAuth::new(200, ROTATED, 50).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let (mut a, mut b) = (Pending::new(), Pending::new());
    tokio::join!(fixture.refresh(&mut a), fixture.refresh(&mut b));
    assert_eq!(mock.count(), 1);
    assert_eq!(fixture.saved().api_key.as_deref(), Some("new-access"));
}

#[tokio::test]
async fn transient_and_terminal_failures_do_not_spin_or_change_material() {
    for (status, body) in [
        (503, "temporary failure"),
        (400, r#"{"error":"invalid_grant"}"#),
    ] {
        let mock = MockOAuth::new(status, body, 0).await;
        let fixture = LocalFixture::new(&mock.url).await;
        let mut pending = Pending::new();
        fixture.refresh(&mut pending).await;
        fixture.refresh(&mut pending).await;
        assert_eq!(mock.count(), 1);
        assert_eq!(fixture.saved().api_key.as_deref(), Some("old-access"));
        assert_eq!(
            fixture.saved().refresh_token.as_deref(),
            Some("old-refresh")
        );
    }
}

#[tokio::test]
async fn no_rotation_preserves_refresh_token_and_second_account_is_not_lost() {
    let mock = MockOAuth::new(200, r#"{"access_token":"new-access","expires_in":3600}"#, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let mut second = document.providers[0].credentials[0].clone();
    second.id = Some("second-account".into());
    document.providers[0].credentials.push(second);
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(mock.count(), 2);
    let snapshot = fixture.runtime.route_config().snapshot();
    for credential in &snapshot.document().providers[0].credentials {
        assert_eq!(credential.api_key.as_deref(), Some("new-access"));
        assert_eq!(credential.refresh_token.as_deref(), Some("old-refresh"));
    }
}

#[tokio::test]
async fn disabled_missing_refresh_and_not_due_accounts_are_skipped() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    for mode in ["disabled", "manual", "future"] {
        let fixture = LocalFixture::new(&mock.url).await;
        let snapshot = fixture.runtime.route_config().snapshot();
        let mut document = snapshot.document().clone();
        let credential = &mut document.providers[0].credentials[0];
        match mode {
            "disabled" => credential.enabled = Some(false),
            "manual" => credential.refresh_token = None,
            _ => credential.expires_at = Some("2099-01-01T00:00:00Z".into()),
        }
        fixture
            .runtime
            .commit_document(snapshot.revision().id(), document, None)
            .await
            .unwrap();
        fixture.refresh(&mut Pending::new()).await;
    }
    assert_eq!(mock.count(), 0);
}
