//! Revision races and failed commits never become a successful credential receipt.
use super::*;

#[tokio::test]
async fn stale_revision_before_import_does_not_consume_receipt_or_rebase() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let issuer = Issuer::new(true).await;
    let session = create(&fixture.state, issuer.client.clone(), None);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    complete(&fixture.state, &session, &grant).await;
    let mut changed = document();
    changed.providers[1].api_key = "synthetic-concurrent-key".into();
    let snapshot = fixture.commit_route_document(changed, None).await;
    let response = build_router(fixture.state.clone())
        .oneshot(request(
            session.id(),
            "import",
            session.binding().revision(),
            &grant,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        snapshot.revision().id()
    );
    assert!(snapshot.document().providers[0].credentials.is_empty());
    assert!(session.claim_receipt(session.binding()).is_ok());
    fixture.state.lifecycle.grok_oauth.join().await;
}

#[tokio::test]
async fn concurrent_revision_after_admission_fails_cas_without_retry_or_duplicate() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let issuer = Issuer::new(true).await;
    let session = create(&fixture.state, issuer.client.clone(), None);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    complete(&fixture.state, &session, &grant).await;
    let lock = fixture
        .state
        .lifecycle
        .health_control
        .publication
        .lock()
        .await;
    // Lose the entire HTTP response; the registry still owns the accepted save.
    let response = build_router(fixture.state.clone())
        .oneshot(request(
            session.id(),
            "import",
            session.binding().revision(),
            &grant,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    drop(response);
    let mut changed = document();
    changed.providers[1].api_key = "synthetic-winner".into();
    let winner = fixture.commit_route_document(changed, None).await;
    drop(lock);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while session.view().status == SessionStatus::Importing {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let body = import(&fixture.state, &session, &grant).await;
    assert_eq!(body["session"]["status"], "save_unconfirmed");
    assert_eq!(body["session"]["errorCode"], "console_revision_conflict");
    assert!(body["session"]["credentialId"].is_null());
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        winner.revision().id()
    );
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.state.lifecycle.grok_oauth.join().await;
}

#[tokio::test]
async fn commit_failure_is_unconfirmed_and_status_remains_pollable_without_new_secret_grant() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let issuer = Issuer::new(true).await;
    let session = create(&fixture.state, issuer.client.clone(), None);
    let revision = session.binding().revision().to_owned();
    let grant = support::grant_console_secret_access(&fixture.state).await;
    complete(&fixture.state, &session, &grant).await;
    fixture
        .fail_next_activation
        .store(true, std::sync::atomic::Ordering::SeqCst);
    import(&fixture.state, &session, &grant).await;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while session.view().status == SessionStatus::Importing {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(fixture
        .activation_failure_seen
        .load(std::sync::atomic::Ordering::SeqCst));
    let app = build_router(fixture.state.clone());
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("{ROOT}/{}", session.id()))
                .header("x-management-token", support::MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::parse_json(response).await;
    assert_eq!(body["session"]["status"], "save_unconfirmed");
    assert!(body["session"]["savedRevision"].is_null());
    let repeat = import(&fixture.state, &session, &grant).await;
    assert_eq!(repeat["session"]["status"], "save_unconfirmed");
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.state.lifecycle.grok_oauth.join().await;
}

#[tokio::test]
async fn expired_or_wrong_grant_and_wrong_action_revision_cannot_claim_ready_material() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let issuer = Issuer::new(true).await;
    let session = create(&fixture.state, issuer.client.clone(), None);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    complete(&fixture.state, &session, &grant).await;
    let app = build_router(fixture.state.clone());
    for action in ["import", "complete", "cancel"] {
        let response = app
            .clone()
            .oneshot(request(
                session.id(),
                action,
                session.binding().revision(),
                "wrong-grant",
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = app
            .clone()
            .oneshot(request(
                session.id(),
                action,
                "wrong-revision",
                &grant,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert!(session.claim_receipt(session.binding()).is_ok());
    fixture.state.lifecycle.grok_oauth.join().await;
}

#[tokio::test]
async fn save_deadline_is_bounded_and_does_not_cancel_into_a_fake_success() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let issuer = Issuer::new(true).await;
    let session = create(&fixture.state, issuer.client.clone(), None);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    complete(&fixture.state, &session, &grant).await;
    tokio::time::pause();
    let lock = fixture
        .state
        .lifecycle
        .health_control
        .publication
        .lock()
        .await;
    import(&fixture.state, &session, &grant).await;
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(21)).await;
    for _ in 0..100 {
        if session.view().status != SessionStatus::Importing {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert_eq!(session.view().error_code, Some("grok_login_save_timeout"));
    assert!(session.view().target_credential_id.is_some());
    drop(lock);
    fixture.state.lifecycle.grok_oauth.join().await;
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
}

#[tokio::test]
async fn expiry_while_waiting_for_publication_cannot_renew_or_save_a_dead_receipt() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let issuer = Issuer::with_lifetime(true, 1).await;
    let session = create(&fixture.state, issuer.client.clone(), None);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    complete(&fixture.state, &session, &grant).await;
    tokio::time::pause();
    let lock = fixture
        .state
        .lifecycle
        .health_control
        .publication
        .lock()
        .await;
    import(&fixture.state, &session, &grant).await;
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(2)).await;
    drop(lock);
    fixture.state.lifecycle.grok_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert_eq!(session.view().error_code, Some("grok_oauth_expired"));
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
}

#[tokio::test]
async fn shutdown_before_admission_retains_ready_material_but_rejects_a_new_save() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let issuer = Issuer::new(true).await;
    let session = create(&fixture.state, issuer.client.clone(), None);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    complete(&fixture.state, &session, &grant).await;
    fixture.state.shutdown.request("synthetic_shutdown");
    let response = build_router(fixture.state.clone())
        .oneshot(request(
            session.id(),
            "import",
            session.binding().revision(),
            &grant,
            None,
        ))
        .await
        .unwrap();
    assert!(!response.status().is_success());
    assert_eq!(session.view().status, SessionStatus::Ready);
    fixture.state.lifecycle.grok_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Cancelled);
}
