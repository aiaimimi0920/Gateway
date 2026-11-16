//! Paid-stage boundaries keep unknown sends explicit and never invent run IDs or settlements.
use super::*;
#[tokio::test]
async fn freebuff_public_paid_unknown_cost_cancel_never_sends_zero_finish() {
    let peer = peer::Peer::new().await;
    let pause = peer.pause("/api/v1/chat/completions", 1);
    *peer.mode.lock().unwrap() = "stream_unknown".into();
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let observed = Arc::new(Notify::new());
    let (cancel, task) = caller(&host, &peer, "caller", observed.clone());
    approve(&host, &task).await;
    body_ready(&pause, &observed).await;
    let _keep = signal(&host, "caller", cancel);
    let error = result(task).await;
    assert_eq!(
        error.code.as_deref(),
        Some("freebuff_business_accounting_unknown")
    );
    assert_eq!(error.request_budget_stop_reason(), Some("caller_cancelled"));
    assert!(peer.finishes().is_empty());
    invariant(&host, &peer, 1, "caller").await;
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_paid_cancel_during_start_waits_for_original_id_then_finishes_zero_steps() {
    let peer = peer::Peer::new().await;
    let pause = peer.pause("/api/v1/agent-runs", 1);
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let (cancel, task) = caller(&host, &peer, "caller", Arc::new(Notify::new()));
    approve(&host, &task).await;
    pause.reached().await;
    let _keep = signal(&host, "caller", cancel);
    assert!(!task.is_finished());
    assert!(peer.finishes().is_empty(), "no guessed START receipt");
    pause.resume();
    let error = result(task).await;
    assert_eq!(error.request_budget_stop_reason(), Some("caller_cancelled"));
    let finishes = peer.finishes();
    assert_eq!(finishes.len(), 1);
    assert_eq!(finishes[0]["runId"], "synthetic-run");
    assert_eq!(finishes[0]["status"], "cancelled");
    assert_eq!(finishes[0]["totalSteps"], 0);
    assert_eq!(finishes[0]["totalCredits"], 0);
    invariant(&host, &peer, 0, "caller").await;
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_paid_cancel_during_finish_awaits_same_ack_without_retry() {
    let peer = peer::Peer::new().await;
    let pause = peer.pause("/api/v1/agent-runs", 2);
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let (cancel, task) = caller(&host, &peer, "caller", Arc::new(Notify::new()));
    approve(&host, &task).await;
    pause.reached().await;
    let _keep = signal(&host, "caller", cancel);
    tokio::task::yield_now().await;
    assert!(
        !task.is_finished(),
        "FINISH acknowledgement must not be dropped"
    );
    pause.resume();
    assert_eq!(
        result(task).await.request_budget_stop_reason(),
        Some("caller_cancelled")
    );
    assert_eq!(peer.finishes().len(), 1);
    assert_eq!(
        peer.finishes()[0]["status"],
        "completed",
        "model completed before cancel"
    );
    invariant(&host, &peer, 1, "caller").await;
    host.close().await;
    peer.server.close().await;
}
