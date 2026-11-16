//! Joined shutdown and post-acceptance expiry retain consumption evidence without publishing it.
use super::{fixture::*, *};
#[tokio::test]
async fn nous_management_shutdown_waits_for_sent_response_and_keeps_cancelled_receipt() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(true, 600).await;
    let view = create(state, &grant, body(state), &peer).await;
    let session = session(state, &view);
    action(state, &view, Some(&grant), "poll", &peer).await;
    peer.wait_called().await;
    let registry = state.lifecycle.nous_oauth.clone();
    let join = tokio::spawn(async move {
        registry.join().await;
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!join.is_finished());
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    peer.release.notify_one();
    tokio::time::timeout(Duration::from_secs(3), join)
        .await
        .unwrap()
        .unwrap();
    assert!(session.view().receipt_received);
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert_eq!(peer.count(), 1);
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    peer.stop().await;
}
#[tokio::test]
async fn nous_management_expiry_before_save_or_during_publication_retains_receipt() {
    for accepted in [false, true] {
        let fixture = support::ConsoleStateFixture::new(document(), true);
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let peer = Peer::new(false, 3).await;
        let view = create(state, &grant, body(state), &peer).await;
        ready(state, &view, &grant, &peer).await;
        let session = session(state, &view);
        let publication = state.lifecycle.health_control.publication.lock().await;
        if accepted {
            assert_eq!(
                action(state, &view, Some(&grant), "import", &peer)
                    .await
                    .status(),
                StatusCode::OK
            );
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
        drop(publication);
        wait_status(
            &session,
            if accepted {
                SessionStatus::SaveUnconfirmed
            } else {
                SessionStatus::Expired
            },
        )
        .await;
        assert!(session.view().receipt_received);
        if accepted {
            assert_eq!(
                action(state, &view, Some(&grant), "import", &peer)
                    .await
                    .status(),
                StatusCode::OK
            );
        } else {
            assert_ne!(
                action(state, &view, Some(&grant), "import", &peer)
                    .await
                    .status(),
                StatusCode::OK
            );
        }
        assert!(state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty());
        assert_eq!(peer.count(), 1);
        state.lifecycle.nous_oauth.join().await;
        peer.stop().await;
    }
}
