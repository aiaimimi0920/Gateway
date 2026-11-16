use super::{fixture::*, *};
#[tokio::test]
async fn nous_management_received_is_retained_on_invalid_access_scope_target_and_cancel() {
    for scenario in ["invalid", "scope", "target", "cancel"] {
        let fixture = support::ConsoleStateFixture::new(document(), true);
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let peer = Peer::new(true, 600).await;
        if scenario == "invalid" {
            peer.reply.lock().unwrap()["access_token"] = json!("invalid-jwt");
        }
        if scenario == "scope" {
            peer.reply.lock().unwrap()["scope"] = json!(7);
        }
        if scenario == "target" {
            peer.reply.lock().unwrap()["inference_base_url"] = json!("https://example.invalid/v1");
        }
        let view = create(state, &grant, body(state), &peer).await;
        let session = session(state, &view);
        // Returning/dropping the HTTP response does not drop the registry's token receive future.
        assert_eq!(
            action(state, &view, Some(&grant), "poll", &peer)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(session.view().status, SessionStatus::Exchanging);
        assert_eq!(
            action(state, &view, Some(&grant), "poll", &peer)
                .await
                .status(),
            StatusCode::OK
        );
        peer.wait_called().await;
        if scenario == "cancel" {
            action(state, &view, Some(&grant), "cancel", &peer).await;
        }
        peer.release.notify_one();
        tokio::time::timeout(Duration::from_secs(3), async {
            while !session.view().receipt_received {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_ne!(session.view().status, SessionStatus::Ready);
        assert_ne!(
            action(state, &view, Some(&grant), "import", &peer)
                .await
                .status(),
            StatusCode::OK
        );
        assert_ne!(
            action(state, &view, Some(&grant), "poll", &peer)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(peer.count(), 1);
        assert!(state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty());
        assert_redacted(&serde_json::to_value(session.view()).unwrap());
        state.lifecycle.nous_oauth.join().await;
        peer.stop().await;
    }
}
#[tokio::test]
async fn nous_management_unknown_fields_stale_target_and_cancel_during_pacing_do_not_send() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(false, 600).await;
    let mut invalid = body(state);
    invalid["redirectUri"] = json!("https://example.invalid");
    let response = origin(
        peer.origin.clone(),
        call(state, ROOT, invalid, Some(&grant)),
    )
    .await;
    assert_ne!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let view = create(state, &grant, body(state), &peer).await;
    action(state, &view, Some(&grant), "poll", &peer).await;
    action(state, &view, Some(&grant), "cancel", &peer).await;
    tokio::time::timeout(Duration::from_secs(2), state.lifecycle.nous_oauth.join())
        .await
        .unwrap();
    assert_eq!(peer.count(), 0);
    peer.stop().await;
}
#[tokio::test]
async fn nous_management_accepted_save_loses_cas_without_rebase_or_second_grant() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(false, 600).await;
    let view = create(state, &grant, body(state), &peer).await;
    ready(state, &view, &grant, &peer).await;
    let session = session(state, &view);
    let publication = state.lifecycle.health_control.publication.lock().await;
    assert_eq!(
        action(state, &view, Some(&grant), "import", &peer)
            .await
            .status(),
        StatusCode::OK
    );
    tokio::task::yield_now().await;
    assert_eq!(session.view().status, SessionStatus::Importing);
    let before = state.route_config.snapshot();
    let mut changed = before.document().clone();
    changed.providers[1]
        .supported_models
        .push("concurrent-model".into());
    state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .commit_document(before.revision().id(), changed, None)
        .await
        .unwrap();
    drop(publication);
    wait_status(&session, SessionStatus::SaveUnconfirmed).await;
    assert!(session.view().receipt_received);
    assert_eq!(
        action(state, &view, Some(&grant), "import", &peer)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(peer.count(), 1);
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    state.lifecycle.nous_oauth.join().await;
    peer.stop().await;
}
