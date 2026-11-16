use super::*;
#[tokio::test]
async fn freebuff_pending_cancel_drop_expiry_and_late_decision_are_one_use() {
    let f = fixture();
    let peer = wire::Peer::new().await;
    let request = TestRequest::new(&f, &peer).await;
    let grant = grant_console_secret_access(&f.state).await;
    let waiting = request
        .begin(&f, &peer, Duration::from_secs(3))
        .await
        .unwrap();
    let view = list(&f, &grant).await[0].clone();
    let hits = peer.hits();
    assert_eq!(view["requestId"], request.ctx.req_id.to_string());
    assert_eq!(view["quote"]["meter"]["sessionPrice"], 10.0);
    let displayed = view.to_string();
    for secret in [
        "synthetic-b",
        "private prompt",
        "caller-key-id",
        "access-key-id",
        "untrusted-body-user",
    ] {
        assert!(!displayed.contains(secret));
    }
    assert_eq!(
        decide(&f, &grant, &view, "cancel").await.status(),
        StatusCode::OK
    );
    assert_eq!(
        waiting.wait().await.err().unwrap().code.as_deref(),
        Some("freebuff_purchase_cancelled")
    );
    assert_eq!(
        decide(&f, &grant, &view, "approve").await.status(),
        StatusCode::CONFLICT
    );
    assert!(list(&f, &grant).await.as_array().unwrap().is_empty());
    assert_eq!(
        peer.hits(),
        hits,
        "management read/decisions are not upstream operations"
    );
    let waiting = request
        .begin(&f, &peer, Duration::from_secs(3))
        .await
        .unwrap();
    let dropped = list(&f, &grant).await[0].clone();
    drop(waiting);
    assert!(list(&f, &grant).await.as_array().unwrap().is_empty());
    assert_eq!(
        decide(&f, &grant, &dropped, "approve").await.status(),
        StatusCode::CONFLICT
    );
    let waiting = request
        .begin(&f, &peer, Duration::from_millis(10))
        .await
        .unwrap();
    assert_eq!(
        waiting.wait().await.err().unwrap().code.as_deref(),
        Some("freebuff_purchase_quote_expired")
    );
    assert!(list(&f, &grant).await.as_array().unwrap().is_empty());
    assert_eq!(peer.purchases(), 0);
    peer.close().await;
}
#[tokio::test]
async fn freebuff_pending_approved_but_unconsumed_still_owns_a_bounded_slot() {
    let f = fixture();
    let peer = wire::Peer::new().await;
    let mut request = TestRequest::new(&f, &peer).await;
    let grant = grant_console_secret_access(&f.state).await;
    let mut waiting = Vec::new();
    for _ in 0..16 {
        request.ctx.req_id = uuid::Uuid::new_v4();
        waiting.push(
            request
                .begin(&f, &peer, Duration::from_secs(30))
                .await
                .unwrap(),
        );
    }
    let views = list(&f, &grant).await;
    assert_eq!(views.as_array().unwrap().len(), 16);
    let view = views
        .as_array()
        .unwrap()
        .iter()
        .find(|view| view["requestId"] == request.ctx.req_id.to_string())
        .unwrap();
    assert_eq!(
        decide(&f, &grant, view, "approve").await.status(),
        StatusCode::OK
    );
    assert_eq!(
        decide(&f, &grant, view, "cancel").await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(list(&f, &grant).await.as_array().unwrap().len(), 15);
    request.ctx.req_id = uuid::Uuid::new_v4();
    assert_eq!(
        request
            .begin(&f, &peer, Duration::from_secs(3))
            .await
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("freebuff_purchase_queue_full")
    );
    // An approved sender has not freed the slot. The ORIGINAL caller dropping it does.
    drop(waiting.pop().unwrap());
    let replacement = request
        .begin(&f, &peer, Duration::from_secs(3))
        .await
        .unwrap();
    drop(replacement);
    drop(waiting);
    assert!(list(&f, &grant).await.as_array().unwrap().is_empty());
    assert_eq!(peer.purchases(), 0);
    peer.close().await;
}
#[tokio::test]
async fn freebuff_pending_drain_closes_waiters_and_invalidates_delivered_approval() {
    let f = fixture();
    let peer = wire::Peer::new().await;
    let mut request = TestRequest::new(&f, &peer).await;
    let grant = grant_console_secret_access(&f.state).await;
    let waiting = request
        .begin(&f, &peer, Duration::from_secs(3))
        .await
        .unwrap();
    let view = list(&f, &grant).await[0].clone();
    assert_eq!(
        decide(&f, &grant, &view, "approve").await.status(),
        StatusCode::OK
    );
    let approved = waiting.wait().await.unwrap();
    request.ctx.req_id = uuid::Uuid::new_v4();
    let second = request
        .begin(&f, &peer, Duration::from_secs(3))
        .await
        .unwrap();
    assert!(f.state.lifecycle.begin_drain("synthetic shutdown"));
    assert!(second.wait().await.is_err());
    drop(approved);
    assert_eq!(
        request
            .begin(&f, &peer, Duration::from_secs(3))
            .await
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("freebuff_purchase_queue_closed")
    );
    assert_eq!(peer.purchases(), 0);
    peer.close().await;
}
#[tokio::test]
async fn freebuff_pending_only_one_quote_can_wait_for_an_original_request() {
    let f = fixture();
    let peer = wire::Peer::new().await;
    let request = TestRequest::new(&f, &peer).await;
    let first = request
        .begin(&f, &peer, Duration::from_secs(3))
        .await
        .unwrap();
    assert_eq!(
        request
            .begin(&f, &peer, Duration::from_secs(3))
            .await
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("freebuff_purchase_request_already_pending")
    );
    drop(first);
    peer.close().await;
}
