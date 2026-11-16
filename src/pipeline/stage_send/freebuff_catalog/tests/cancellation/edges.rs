//! Public TCP shutdown, unknown START/header/FINISH and actual Drop retain their distinct meanings.
use super::*;
#[tokio::test]
async fn freebuff_public_paid_real_http_shutdown_finishes_reported_cost_before_response() {
    let peer = peer::Peer::new().await;
    let pause = peer.pause("/api/v1/chat/completions", 1);
    *peer.mode.lock().unwrap() = "stream_known".into();
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let grant = host.grant().await;
    let task = host.public(request());
    let view = host.pending(&grant, &task).await;
    assert_eq!(host.decision(&grant, &view, "approve").await, 200);
    body_ready(&pause, &host.model_observed).await;
    host.state.shutdown.request("synthetic_http_paid_shutdown");
    let (status, body) = task.await.unwrap();
    assert_eq!(status, 503, "{body}");
    assert_eq!(peer.finishes().len(), 1);
    assert_eq!(peer.finishes()[0]["status"], "cancelled");
    assert_eq!(peer.finishes()[0]["totalCredits"], 13);
    invariant(&host, &peer, 1, "shutdown").await;
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_paid_drop_after_report_has_no_detached_finish_or_end() {
    let peer = peer::Peer::new().await;
    let pause = peer.pause("/api/v1/chat/completions", 1);
    *peer.mode.lock().unwrap() = "stream_known".into();
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let observed = Arc::new(Notify::new());
    let (_cancel, task) = caller(&host, &peer, "caller", observed.clone());
    approve(&host, &task).await;
    body_ready(&pause, &observed).await;
    task.abort();
    match task.await {
        Err(error) => assert!(error.is_cancelled()),
        Ok(_) => panic!("aborted owner must not return a completion"),
    }
    settled(&host, "cancelled", 10).await;
    assert_eq!(claims(&host).await, 1);
    assert!(peer.finishes().is_empty());
    assert_eq!(peer.calls("/api/v1/freebuff/session/attempt"), 0);
    assert_eq!(send_fences::attempts(&host, 1).await, [1]);
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_paid_admission_or_headers_cancel_never_guesses_run_or_cost() {
    for phase in ["admission", "headers"] {
        let peer = peer::Peer::new().await;
        let pause = peer.pause(
            if phase == "admission" {
                "/api/v1/freebuff/session/admission"
            } else {
                "/api/v1/chat/completions"
            },
            1,
        );
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let (cancel, task) = caller(&host, &peer, "caller", Arc::new(Notify::new()));
        approve(&host, &task).await;
        pause.reached().await;
        let _keep = signal(&host, "caller", cancel);
        pause.resume();
        let error = result(task).await;
        assert_eq!(error.request_budget_stop_reason(), Some("caller_cancelled"));
        if phase == "headers" {
            assert_eq!(
                error.code.as_deref(),
                Some("freebuff_business_accounting_unknown")
            );
        }
        assert!(peer.finishes().is_empty());
        assert_eq!(
            peer.calls("/api/v1/agent-runs"),
            usize::from(phase == "headers")
        );
        invariant(&host, &peer, usize::from(phase == "headers"), "caller").await;
        host.close().await;
        peer.server.close().await;
    }
}
#[tokio::test]
async fn freebuff_public_paid_unknown_start_and_rejected_finish_are_not_retried_or_replaced() {
    for phase in ["start_unknown", "finish_rejected"] {
        let peer = peer::Peer::new().await;
        *peer.mode.lock().unwrap() = phase.into();
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let grant = host.grant().await;
        let task = host.public(request());
        let view = host.pending(&grant, &task).await;
        assert_eq!(host.decision(&grant, &view, "approve").await, 200);
        let (status, body) = task.await.unwrap();
        assert_eq!(status, 502, "{body}");
        assert_eq!(body["error"]["code"], "freebuff_business_run_unconfirmed");
        assert_eq!(
            peer.finishes().len(),
            usize::from(phase == "finish_rejected")
        );
        invariant(
            &host,
            &peer,
            usize::from(phase == "finish_rejected"),
            "upstream",
        )
        .await;
        host.close().await;
        peer.server.close().await;
    }
}
#[tokio::test]
async fn freebuff_public_paid_function_tools_keep_single_attempt_through_finish() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let mut body = request();
    body["tools"] = json!([{"type":"function","function":{"name":"caller_only","parameters":{"type":"object"}}}]);
    let grant = host.grant().await;
    let task = host.public(body.clone());
    let view = host.pending(&grant, &task).await;
    assert_eq!(host.decision(&grant, &view, "approve").await, 200);
    let (status, reply) = task.await.unwrap();
    assert_eq!(status, 200, "{reply}");
    assert_eq!(reply, peer::completion());
    let sent = peer
        .hits
        .lock()
        .unwrap()
        .iter()
        .find(|(_, path, _, _)| path == "/api/v1/chat/completions")
        .unwrap()
        .3
        .clone();
    assert_eq!(
        sent["tools"], body["tools"],
        "no host tools or CLI harness injected"
    );
    assert_eq!(peer.finishes().len(), 1);
    assert_eq!(peer.finishes()[0]["totalCredits"], 13);
    settled(&host, "completed", 9).await;
    assert_eq!(send_fences::attempts(&host, 1).await, [1]);
    host.close().await;
    peer.server.close().await;
}
