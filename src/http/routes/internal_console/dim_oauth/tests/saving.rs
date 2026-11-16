use super::{fixture::*, *};
use crate::console::dim_oauth::SessionStatus;
use std::time::Duration;

#[tokio::test]
async fn dim_oauth_management_directory_failure_never_partially_saves_the_account() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::ZERO).await;
    let view = create(state, &grant, create_body(state), &peer).await;
    let session = session(state, &view);
    complete(state, &view, &grant, &peer).await;
    peer.catalog_status
        .store(503, std::sync::atomic::Ordering::SeqCst);
    let revision = state.route_config.snapshot().revision().id().to_owned();
    let response = action(state, &view, Some(&grant), "import", None, &peer).await;
    assert_eq!(response.status(), StatusCode::OK);
    wait_status(&session, SessionStatus::SaveUnconfirmed).await;
    assert_eq!(session.view().error_code, Some("dim_catalog_read_failed"));
    assert_eq!(state.route_config.snapshot().revision().id(), revision);
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    assert!(peer.inference.lock().unwrap().is_empty());
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_save_checks_token_expiry_after_waiting_for_publication() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(10, Duration::ZERO).await;
    let view = create(state, &grant, create_body(state), &peer).await;
    let session = session(state, &view);
    complete(state, &view, &grant, &peer).await;
    let publication = state.lifecycle.health_control.publication.lock().await;
    let response = action(state, &view, Some(&grant), "import", None, &peer).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(session.view().status, SessionStatus::Importing);
    // The new directory read performs real loopback I/O before waiting on publication.
    // Keep a real clock here; advancing virtual time could expire the network read instead.
    tokio::time::sleep(Duration::from_secs(11)).await;
    drop(publication);
    wait_status(&session, SessionStatus::SaveUnconfirmed).await;
    assert_eq!(
        session.view().error_code,
        Some("dim_login_material_expired")
    );
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    assert_eq!(
        action(state, &view, Some(&grant), "import", None, &peer)
            .await
            .status(),
        StatusCode::OK
    );
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    assert_eq!(peer.count(), 1);
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_admitted_save_loses_cas_without_rebase_or_second_exchange() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::ZERO).await;
    let view = create(state, &grant, create_body(state), &peer).await;
    let session = session(state, &view);
    complete(state, &view, &grant, &peer).await;
    let publication = state.lifecycle.health_control.publication.lock().await;
    assert_eq!(
        action(state, &view, Some(&grant), "import", None, &peer)
            .await
            .status(),
        StatusCode::OK
    );
    let old = state.route_config.snapshot();
    let mut changed = old.document().clone();
    changed.providers[0].vendor_name = Some("Competing accepted edit".into());
    state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .commit_document(old.revision().id(), changed, None)
        .await
        .unwrap();
    drop(publication);
    wait_status(&session, SessionStatus::SaveUnconfirmed).await;
    let response = action(state, &view, Some(&grant), "import", None, &peer).await;
    assert_eq!(
        support::parse_json(response).await["session"]["status"],
        "save_unconfirmed"
    );
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    assert_eq!(peer.count(), 1);
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_reauthorization_preserves_id_operator_fields_and_other_secrets() {
    let mut doc = document();
    let context = crate::protocol::dim_oauth::test_support::context();
    doc.providers[0].credentials.push(serde_json::from_value(json!({"id":"stable","api_key":"synthetic-old-access",
        "refresh_token":"synthetic-old-refresh","dim_oauth":context,"enabled":false,"account_name":"Operator label",
        "supported_models":["operator-model"],"expires_at":"2030-01-01T00:00:00Z"})).unwrap());
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::ZERO).await;
    let mut body = create_body(state);
    body["credentialId"] = json!("stable");
    let view = create(state, &grant, body, &peer).await;
    let session = session(state, &view);
    complete(state, &view, &grant, &peer).await;
    assert_eq!(
        action(state, &view, Some(&grant), "import", None, &peer)
            .await
            .status(),
        StatusCode::OK
    );
    wait_status(&session, SessionStatus::Succeeded).await;
    let snapshot = state.route_config.snapshot();
    let saved = &snapshot.document().providers[0].credentials[0];
    assert_eq!(saved.id.as_deref(), Some("stable"));
    assert_eq!(saved.enabled, Some(false));
    assert_eq!(saved.account_name.as_deref(), Some("Operator label"));
    assert_eq!(
        saved.supported_models,
        [
            "operator-model",
            "opaque-chat",
            "opaque-responses",
            "opaque-messages",
            "opaque-gemini"
        ]
    );
    assert_eq!(saved.dim_catalog.as_ref().unwrap().models().len(), 4);
    assert_eq!(saved.api_key.as_deref(), Some("synthetic-dim-access"));
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("synthetic-dim-refresh")
    );
    assert_eq!(
        snapshot.document().providers[1].api_key,
        "synthetic-unrelated-secret"
    );
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_reauthorization_cannot_replace_a_different_subject() {
    let mut doc = document();
    let mut context = crate::protocol::dim_oauth::test_support::context();
    context["subject"] = json!("other-subject");
    doc.providers[0].credentials.push(
        serde_json::from_value(json!({"id":"stable","api_key":"synthetic-old-access",
        "refresh_token":"synthetic-old-refresh","dim_oauth":context}))
        .unwrap(),
    );
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::ZERO).await;
    let mut body = create_body(state);
    body["credentialId"] = json!("stable");
    let view = create(state, &grant, body, &peer).await;
    complete(state, &view, &grant, &peer).await;
    let response = action(state, &view, Some(&grant), "import", None, &peer).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        support::parse_json(response).await["error"]["code"],
        "dim_login_identity_mismatch"
    );
    assert_eq!(
        state.route_config.snapshot().document().providers[0].credentials[0]
            .api_key
            .as_deref(),
        Some("synthetic-old-access")
    );
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}
