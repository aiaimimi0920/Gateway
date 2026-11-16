//! Purchase authority and account-slot races on the real public route.
use super::*;
#[tokio::test]
async fn freebuff_public_chat_requires_explicit_existing_device_registration() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    let (status, body) = host.public(request()).await.unwrap();
    assert_eq!(status, 409, "{body}");
    assert_eq!(peer.hits.lock().unwrap().len(), 0);
    assert_eq!(claims(&host).await, 0);
    settled(&host, "failed", 10).await;
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_chat_approval_rechecks_grant_after_concurrency_queue() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let controller = host.state.concurrency_registry.get_or_create("pool");
    let mut held = Vec::new();
    for _ in 0..controller.snapshot().current_limit {
        held.push(controller.acquire().await);
    }
    let grant = host.grant().await;
    let task = host.public(request());
    let view = host.pending(&grant, &task).await;
    assert_eq!(host.decision(&grant, &view, "approve").await, 200);
    host.state
        .console_auth
        .logout(
            &crate::console::ConsoleRequestContext::loopback(),
            support::MANAGEMENT_TOKEN,
        )
        .unwrap();
    drop(held);
    let (status, body) = task.await.unwrap();
    assert_eq!(status, 403, "{body}");
    assert_eq!(claims(&host).await, 0);
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 0);
    settled(&host, "failed", 10).await;
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_chat_concurrent_approvals_cannot_reuse_first_claim() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let grant = host.grant().await;
    let first = host.public(request());
    let second = host.public(request());
    let views = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = host.list(&grant).await;
            if rows.as_array().unwrap().len() == 2 {
                break rows;
            }
            assert!(!first.is_finished() && !second.is_finished());
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    for view in views.as_array().unwrap() {
        assert_eq!(host.decision(&grant, view, "approve").await, 200);
    }
    let (a, b) = tokio::join!(first, second);
    let mut statuses = [a.unwrap().0, b.unwrap().0];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
    assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
    assert_eq!(claims(&host).await, 1);
    tokio::time::timeout(Duration::from_secs(3), async {
        while host.balance().await != 9 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(host.list(&grant).await.as_array().unwrap().is_empty());
    assert_eq!(send_fences::attempts(&host, 2).await, [0, 1]);
    host.close().await;
    peer.server.close().await;
}
