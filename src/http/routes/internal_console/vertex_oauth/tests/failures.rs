//! Once accepted, save/exchange outcomes cannot be turned into a second send or a fake receipt.
use super::{fixture::*, *};
use crate::{
    console::vertex_oauth::SessionStatus,
    protocol::vertex_oauth::transport_test_support::test_endpoint,
};
use std::{sync::atomic::Ordering, time::Duration};
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn vertex_oauth_management_admitted_save_loses_cas_without_rebase_or_retry() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    complete(state, &view, &grant, &mock).await;
    let lock = state.lifecycle.health_control.publication.lock().await;
    let response = action(state, &view, Some(&grant), "import", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    drop(response);
    let id = session.view().target_credential_id.unwrap();
    let mut winner = document();
    winner.providers[1].api_key = "synthetic-winner".into();
    let newer = fixture.commit_route_document(winner, None).await;
    drop(lock);
    wait_status(&session, SessionStatus::SaveUnconfirmed).await;
    let body = support::parse_json(action(state, &view, Some(&grant), "import", None).await).await;
    assert_eq!(body["session"]["status"], "save_unconfirmed");
    assert_eq!(body["session"]["errorCode"], "console_revision_conflict");
    assert_eq!(body["session"]["targetCredentialId"], id);
    assert!(body["session"]["credentialId"].is_null());
    assert!(body["session"]["savedRevision"].is_null());
    assert_eq!(
        state.route_config.snapshot().revision().id(),
        newer.revision().id()
    );
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    assert_eq!(mock.count(), 1);
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_activation_failure_is_pollable_but_cannot_reimport() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    complete(state, &view, &grant, &mock).await;
    fixture.fail_next_activation.store(true, Ordering::SeqCst);
    assert_eq!(
        action(state, &view, Some(&grant), "import", None)
            .await
            .status(),
        StatusCode::OK
    );
    wait_status(&session, SessionStatus::SaveUnconfirmed).await;
    assert!(fixture.activation_failure_seen.load(Ordering::SeqCst));
    let path = format!("{ROOT}/{}", session.id());
    let response =
        support::send_console_json(state, Method::GET, &path, json!(null), None, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        support::parse_json(response).await["session"]["status"],
        "save_unconfirmed"
    );
    for _ in 0..2 {
        let response = action(state, &view, Some(&grant), "import", None).await;
        assert_eq!(
            support::parse_json(response).await["session"]["status"],
            "save_unconfirmed"
        );
    }
    assert_eq!(
        state.route_config.snapshot().revision().id(),
        view["expectedRevision"].as_str().unwrap()
    );
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_save_budget_includes_publication_lock_and_keeps_unconfirmed_receipt(
) {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    complete(state, &view, &grant, &mock).await;
    tokio::time::pause();
    let lock = state.lifecycle.health_control.publication.lock().await;
    assert_eq!(
        action(state, &view, Some(&grant), "import", None)
            .await
            .status(),
        StatusCode::OK
    );
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(21)).await;
    wait_status(&session, SessionStatus::SaveUnconfirmed).await;
    assert_eq!(session.view().error_code, Some("vertex_login_save_timeout"));
    drop(lock);
    assert_eq!(
        support::parse_json(action(state, &view, Some(&grant), "import", None).await).await
            ["session"]["status"],
        "save_unconfirmed"
    );
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_callback_disconnect_does_not_cancel_joined_exchange() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(Reply {
        body: tokens().to_string(),
        header_delay: Duration::from_millis(100),
        ..Reply::default()
    })
    .await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    let mut stream = connect(&view).await;
    stream
        .write_all(callback_request(&view).await.as_bytes())
        .await
        .unwrap();
    mock.wait_called().await;
    drop(stream);
    wait_status(&session, SessionStatus::Ready).await;
    assert_eq!(mock.count(), 1);
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_cancel_during_callback_exchange_never_restores_ready() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(Reply {
        body: tokens().to_string(),
        header_delay: Duration::from_secs(30),
        ..Reply::default()
    })
    .await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    let mut stream = connect(&view).await;
    stream
        .write_all(callback_request(&view).await.as_bytes())
        .await
        .unwrap();
    mock.wait_called().await;
    assert_eq!(
        action(state, &view, Some(&grant), "cancel", None)
            .await
            .status(),
        StatusCode::OK
    );
    tokio::time::timeout(Duration::from_secs(2), state.lifecycle.vertex_oauth.join())
        .await
        .unwrap();
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert_eq!(mock.count(), 1);
    assert!(session.view().authorization_url.is_none());
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_dropped_manual_completion_consumes_code_and_does_not_retry() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(Reply {
        body: tokens().to_string(),
        header_delay: Duration::from_secs(30),
        ..Reply::default()
    })
    .await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    let cb = callback(&view);
    let mut request = Box::pin(test_endpoint(
        mock.endpoint.clone(),
        action(state, &view, Some(&grant), "complete", Some(&cb)),
    ));
    tokio::select! {
        _=mock.wait_called()=>(),
        _=&mut request=>panic!("exchange completed before synthetic slow peer"),
    }
    drop(request);
    assert_eq!(session.view().status, SessionStatus::Failed);
    assert_eq!(
        session.view().error_code,
        Some("vertex_login_exchange_unconfirmed")
    );
    assert_eq!(
        action(state, &view, Some(&grant), "complete", Some(&cb))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(mock.count(), 1);
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}
