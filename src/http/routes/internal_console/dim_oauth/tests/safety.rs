use super::{fixture::*, *};
use crate::console::dim_oauth::SessionStatus;
use std::time::Duration;

#[tokio::test]
async fn dim_oauth_management_site_unknown_fields_and_stale_revision_reject_before_exchange() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::ZERO).await;
    for (key, value, status) in [
        ("site", json!("global"), StatusCode::BAD_REQUEST),
        (
            "clientSecret",
            json!("not-accepted"),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "tokenEndpoint",
            json!("https://untrusted.invalid/token"),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        ("expectedRevision", json!("stale"), StatusCode::CONFLICT),
        ("scopes", json!(["profile"]), StatusCode::BAD_REQUEST),
    ] {
        let mut body = create_body(state);
        body[key] = value;
        let response = origin(peer.origin.clone(), call(state, ROOT, body, Some(&grant))).await;
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    assert_eq!(peer.count(), 0);
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_original_revision_is_required_before_exchange_and_save() {
    for ready in [false, true] {
        let fixture = support::ConsoleStateFixture::new(document(), true);
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let peer = Peer::new(3600, Duration::ZERO).await;
        let view = create(state, &grant, create_body(state), &peer).await;
        if ready {
            complete(state, &view, &grant, &peer).await;
        }
        let old = state.route_config.snapshot();
        let mut changed = old.document().clone();
        changed.providers[0].vendor_name = Some("Changed while authorizing".into());
        state
            .route_config_runtime
            .as_ref()
            .unwrap()
            .commit_document(old.revision().id(), changed, None)
            .await
            .unwrap();
        let callback = callback(&view);
        let response = action(
            state,
            &view,
            Some(&grant),
            if ready { "import" } else { "complete" },
            if ready { None } else { Some(&callback) },
            &peer,
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(peer.count(), usize::from(ready));
        assert!(state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty());
        state.lifecycle.dim_oauth.join().await;
        peer.stop().await;
    }
}

#[tokio::test]
async fn dim_oauth_management_cancel_interrupts_inflight_token_body_and_never_becomes_ready() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::from_secs(120)).await;
    let view = create(state, &grant, create_body(state), &peer).await;
    let session = session(state, &view);
    let callback = callback(&view);
    let http = rquest::Client::new();
    let complete = origin(peer.origin.clone(), session.complete(&http, &callback));
    tokio::pin!(complete);
    tokio::select! { result=&mut complete=>panic!("unexpected completion: {result:?}"), _=peer.wait_called()=>{} }
    session.cancel();
    assert!(complete.await.is_err());
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    tokio::time::timeout(Duration::from_secs(2), state.lifecycle.dim_oauth.join())
        .await
        .unwrap();
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    assert_eq!(peer.count(), 1);
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_dropped_manual_exchange_never_reuses_the_code() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::from_secs(120)).await;
    let view = create(state, &grant, create_body(state), &peer).await;
    let session = session(state, &view);
    let callback = callback(&view);
    let http = rquest::Client::new();
    let mut complete = Box::pin(origin(
        peer.origin.clone(),
        session.complete(&http, &callback),
    ));
    tokio::select! { result=&mut complete=>panic!("unexpected completion: {result:?}"), _=peer.wait_called()=>{} }
    drop(complete);
    assert_eq!(session.view().status, SessionStatus::Failed);
    assert_eq!(
        session.view().error_code,
        Some("dim_login_exchange_unconfirmed")
    );
    assert!(session.complete(&http, &callback).await.is_err());
    assert_eq!(peer.count(), 1);
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_matching_state_denial_is_terminal_but_bad_manual_state_is_not() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::ZERO).await;
    let view = create(state, &grant, create_body(state), &peer).await;
    let session = session(state, &view);
    let bad = format!(
        "{}?code=c&state=wrong",
        view["redirectUri"].as_str().unwrap()
    );
    assert_eq!(
        action(state, &view, Some(&grant), "complete", Some(&bad), &peer)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(session.view().status, SessionStatus::WaitingUser);
    let denial = callback(&view).replace("code=synthetic-code", "error=access_denied");
    assert_eq!(
        action(state, &view, Some(&grant), "complete", Some(&denial), &peer)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(session.view().status, SessionStatus::Failed);
    assert_eq!(peer.count(), 0);
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}
