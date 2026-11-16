use super::*;

#[tokio::test]
async fn normal_freebuff_router_paces_status_and_persists_server_material_once_without_http_waiter()
{
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![
        code(),
        Spec::json(401, json!({"error":"not-yet"})),
        user(),
    ])
    .await;
    let (view, session) = create(state, &grant, create_body(state), &mock).await;
    assert_eq!(
        session.view().login_url.as_deref(),
        Some("https://freebuff.com/login?code=synthetic-browser-code")
    );
    let response = TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            action(state, &view, Some(&grant), "poll"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    wait(&session, SessionStatus::WaitingUser).await;
    assert_eq!(session.view().poll_interval_secs, Some(5));
    let response = TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            action(state, &view, Some(&grant), "poll"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(session.view().status, SessionStatus::Polling);
    assert_eq!(
        action(state, &view, Some(&grant), "poll").await.status(),
        StatusCode::OK
    );
    wait(&session, SessionStatus::Ready).await;
    assert!(session.view().login_url.is_none());
    let public = serde_json::to_string(&session.view()).unwrap();
    for secret in [
        "synthetic-token",
        FINGERPRINT,
        "synthetic-hash",
        "synthetic@example.invalid",
    ] {
        assert!(!public.contains(secret));
    }
    let lock = state.lifecycle.health_control.publication.lock().await;
    let response = action(state, &view, Some(&grant), "import").await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    drop(response);
    let target = session.view().target_credential_id.unwrap();
    for action_name in ["import", "cancel"] {
        assert_eq!(
            action(state, &view, Some(&grant), action_name)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(session.view().status, SessionStatus::Importing);
    }
    drop(lock);
    wait(&session, SessionStatus::Succeeded).await;
    let response = action(state, &view, Some(&grant), "import").await;
    assert_eq!(response.status(), StatusCode::OK);
    let read = support::send_console_json(
        state,
        Method::GET,
        &format!("{ROOT}/{}", session.id()),
        json!(null),
        None,
        None,
    )
    .await;
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(
        support::parse_json(read).await["session"]["credentialId"],
        target
    );
    state.lifecycle.freebuff_login.join().await;
    let snapshot = state.route_config.snapshot();
    let saved = &snapshot.document().providers[0].credentials[0];
    assert_eq!(snapshot.document().providers[0].credentials.len(), 1);
    assert_eq!(saved.id.as_deref(), Some(target.as_str()));
    assert_eq!(saved.api_key.as_deref(), Some("synthetic-token"));
    assert!(
        saved.auth_token.is_none() && saved.refresh_token.is_none() && saved.expires_at.is_none()
    );
    let context = saved.freebuff_session.as_ref().unwrap();
    assert_eq!(context.fingerprint_id(), Some(FINGERPRINT));
    assert_eq!(context.fingerprint_hash(), Some("synthetic-hash+/="));
    assert!(snapshot.get_providers()[0].credential_pool[0]
        .refresh_config
        .is_none());
    assert_eq!(
        session.view().saved_revision.as_deref(),
        Some(snapshot.revision().id())
    );
    let archive = state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .coordinator()
        .unwrap()
        .load_revision(snapshot.revision().id())
        .unwrap()
        .unwrap();
    assert_eq!(
        archive.document().providers[0].credentials[0]
            .freebuff_session
            .as_ref(),
        Some(context)
    );
    assert_eq!(
        snapshot.document().providers[1].api_key,
        "synthetic-other-secret"
    );
    let redacted = serde_json::to_string(
        &crate::console::secrets::redact_route_document(snapshot.document()).unwrap(),
    )
    .unwrap();
    for secret in [
        "synthetic-token",
        FINGERPRINT,
        "synthetic-hash",
        "synthetic-other-secret",
    ] {
        assert!(!redacted.contains(secret));
    }
    let calls = mock.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].0, Method::POST);
    assert_eq!(calls[1].0, Method::GET);
    for call in calls {
        assert!(call.2.get("authorization").is_none() && call.2.get("cookie").is_none());
    }
    mock.stop().await;
}

#[tokio::test]
async fn freebuff_commit_revision_race_is_unconfirmed_and_does_not_replay_prepared_save() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![code(), user()]).await;
    let (view, session) = create(state, &grant, create_body(state), &mock).await;
    make_ready(state, &view, &session, &grant, &mock).await;
    let lock = state.lifecycle.health_control.publication.lock().await;
    assert_eq!(
        action(state, &view, Some(&grant), "import").await.status(),
        StatusCode::OK
    );
    let mut changed = state.route_config.snapshot().document().clone();
    changed.providers[0].label = Some("concurrent-edit".into());
    fixture
        .commit_route_document(changed, Some("concurrent edit"))
        .await;
    drop(lock);
    wait(&session, SessionStatus::SaveUnconfirmed).await;
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert!(session.view().credential_id.is_none());
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    assert_eq!(
        action(state, &view, Some(&grant), "import").await.status(),
        StatusCode::OK
    );
    state.lifecycle.freebuff_login.join().await;
    assert_eq!(mock.count(), 2);
    mock.stop().await;
}
