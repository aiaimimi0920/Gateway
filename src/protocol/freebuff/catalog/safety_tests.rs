use super::{
    fixture::*,
    wire_tests::{fetch, key, session},
    *,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
const BUDGET: Duration = Duration::from_secs(3);

#[tokio::test]
async fn redirects_never_forward_bearer_or_device_material_for_either_operation() {
    let trap = Peer::start(vec![]).await;
    for status in [301, 302, 303, 307, 308] {
        for post in [false, true] {
            let mut reply = Spec::json(status, json!({}));
            reply
                .headers
                .insert("location", format!("{}/trap", trap.origin).parse().unwrap());
            let replies = if post {
                vec![Spec::catalog(), reply]
            } else {
                vec![reply]
            };
            let peer = Peer::start(replies).await;
            let mut session = session(ClientSurface::Cli);
            let error = if post {
                fetch(&peer, &mut session, None, BUDGET).await.unwrap();
                TEST_ORIGIN
                    .scope(
                        peer.origin.clone(),
                        session.register_device(key().0, BUDGET),
                    )
                    .await
                    .err()
            } else {
                fetch(&peer, &mut session, None, BUDGET).await.err()
            };
            assert_eq!(error, Some(CatalogError::RedirectRejected));
            assert_eq!(peer.count(), if post { 2 } else { 1 });
            assert_eq!(trap.count(), 0);
            peer.close().await;
        }
    }
    trap.close().await;
}

#[tokio::test]
async fn catalog_decoded_and_declared_body_caps_are_enforced() {
    for declared in [false, true] {
        let mut reply = Spec::catalog();
        reply.body = vec![b'x'; 1_048_577];
        if declared {
            reply
                .headers
                .insert("content-length", "1048577".parse().unwrap());
        }
        let peer = Peer::start(vec![reply]).await;
        assert_eq!(
            fetch(&peer, &mut session(ClientSurface::Cli), None, BUDGET).await,
            Err(CatalogError::ResponseTooLarge)
        );
        assert_eq!(peer.count(), 1);
        peer.close().await;
    }
}

#[tokio::test]
async fn registration_cap_is_decoded_not_only_compressed_content_length() {
    let mut reply = Spec::json(200, json!({}));
    // gzip of 65,537 ASCII x bytes, far smaller on the wire than the registration response cap.
    reply.body = hex::decode("1f8b080000000000020aedc1010100000082a0dbceaf2140010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00d81d82a9c01000100").unwrap();
    reply
        .headers
        .insert("content-encoding", "gzip".parse().unwrap());
    let peer = Peer::start(vec![Spec::catalog(), reply]).await;
    let mut session = session(ClientSurface::Cli);
    fetch(&peer, &mut session, None, BUDGET).await.unwrap();
    assert_eq!(
        TEST_ORIGIN
            .scope(
                peer.origin.clone(),
                session.register_device(key().0, BUDGET)
            )
            .await
            .err(),
        Some(CatalogError::ResponseTooLarge)
    );
    assert_eq!(peer.count(), 2);
    peer.close().await;
}

#[tokio::test]
async fn send_and_read_have_one_deadline_and_timeout_retains_only_previous_snapshot() {
    for (header_ms, body_ms) in [(300, 0), (0, 300), (120, 180)] {
        let mut delayed = Spec::catalog();
        delayed.header_delay = Duration::from_millis(header_ms);
        delayed.body_delay = Duration::from_millis(body_ms);
        let peer = Peer::start(vec![Spec::catalog(), delayed]).await;
        let mut session = session(ClientSurface::Cli);
        fetch(&peer, &mut session, None, BUDGET).await.unwrap();
        assert_eq!(
            fetch(&peer, &mut session, None, Duration::from_millis(200)).await,
            Err(CatalogError::Timeout)
        );
        assert!(session.catalog().unwrap().select("open").is_ok());
        assert_eq!(peer.count(), 2);
        peer.close().await;
    }
}

#[tokio::test]
async fn cancellation_drops_inflight_body_and_does_not_publish_partial_catalog() {
    let mut delayed = Spec::catalog();
    delayed.body_delay = Duration::from_secs(60);
    let peer = Peer::start(vec![delayed]).await;
    let mut session = session(ClientSurface::Cli);
    let reading = Arc::new(tokio::sync::Notify::new());
    let mut pending = Box::pin(
        TEST_BODY_STARTED.scope(reading.clone(), fetch(&peer, &mut session, None, BUDGET)),
    );
    tokio::time::timeout(BUDGET, async {
        tokio::select! { _ = reading.notified() => {}, result = &mut pending => panic!("unexpected: {result:?}") }
    }).await.unwrap();
    drop(pending);
    peer.wait_drops(1).await;
    assert!(session.catalog().is_none());
    assert_eq!(peer.count(), 1);
    peer.close().await;
}

#[tokio::test]
async fn unknown_registration_post_result_is_not_replayed() {
    let mut delayed = Spec::json(200, json!({"keyId":"synthetic-key"}));
    delayed.body_delay = Duration::from_secs(60);
    let peer = Peer::start(vec![Spec::catalog(), delayed]).await;
    let mut session = session(ClientSurface::Cli);
    fetch(&peer, &mut session, None, BUDGET).await.unwrap();
    let error = TEST_ORIGIN
        .scope(
            peer.origin.clone(),
            session.register_device(key().0, Duration::from_millis(100)),
        )
        .await
        .err();
    assert_eq!(error, Some(CatalogError::Timeout));
    peer.wait_drops(2).await;
    assert_eq!(peer.count(), 2, "one bootstrap GET and exactly one POST");
    peer.close().await;
}

#[tokio::test]
async fn zero_budget_is_rejected_locally_without_touching_cached_catalog() {
    let peer = Peer::start(vec![Spec::catalog()]).await;
    let mut session = session(ClientSurface::Cli);
    fetch(&peer, &mut session, None, BUDGET).await.unwrap();
    assert_eq!(
        fetch(&peer, &mut session, None, Duration::ZERO).await,
        Err(CatalogError::InvalidInput)
    );
    assert!(session.catalog().is_some());
    assert_eq!(peer.count(), 1);
    peer.close().await;
}
