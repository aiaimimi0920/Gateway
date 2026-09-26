use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_send, PipelineOutput};
use tokio::time::timeout;

use super::fixture::{candidate, context, TestState, Upstream, DEADLINE, SSE_REPLY};

#[tokio::test]
async fn clean_stream_defers_success_until_eof_and_reports_once() {
    let upstream = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "stream");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = context(true, vec![selected]);
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(mut stream) = output else {
        panic!("expected SSE")
    };
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    let received = timeout(DEADLINE, async {
        let mut received = Vec::new();
        while let Some(chunk) = stream.next().await {
            received.extend_from_slice(&chunk.unwrap());
            assert!(received.len() <= 64 * 1024, "unbounded fixture response");
        }
        received
    })
    .await
    .unwrap();
    let text = String::from_utf8(received).unwrap();
    assert!(text.contains("local stream reply"));
    assert!(text.contains("[DONE]"));
    assert!(text.contains("\"total_tokens\":10"));
    assert_eq!(controller.snapshot().consecutive_successes, 1);
    drop(stream);
    assert_eq!(controller.snapshot().consecutive_successes, 1);
    assert_eq!(controller.snapshot().current_limit, 10);
    let requests = upstream.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].body["stream"], true);
    assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 1);
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn dropping_stream_reports_failure_without_early_success_or_held_permit() {
    let upstream = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "drop");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = context(true, vec![selected]);
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(stream) = output else {
        panic!("expected SSE")
    };
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    drop(stream);
    assert_eq!(controller.snapshot().current_limit, 7);
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    assert_eq!(controller.snapshot().active_count, 0);
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn stream_start_failure_falls_back_before_returning_any_sse() {
    let failed = Upstream::start(
        StatusCode::SERVICE_UNAVAILABLE,
        false,
        r#"{"error":{"message":"fixture unavailable"}}"#,
    )
    .await;
    let success = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let first = candidate(&failed, "stream-failed");
    let second = candidate(&success, "stream-fallback");
    let first_ctrl = fixture
        .state
        .concurrency_registry
        .get_or_create(&first.provider_account_id);
    let mut ctx = context(true, vec![first.clone(), second.clone()]);
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(mut stream) = output else {
        panic!("expected fallback SSE")
    };
    assert_eq!(failed.requests().len(), 1);
    assert_eq!(success.requests().len(), 1);
    assert_eq!(
        ctx.selected_provider_id,
        Some(second.provider_account_id.clone())
    );
    assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 2);
    assert_eq!(
        *ctx.attempted_provider_ids.lock(),
        vec![first.provider_account_id, second.provider_account_id]
    );
    assert_eq!(first_ctrl.snapshot().current_limit, 7);
    assert_eq!(first_ctrl.snapshot().active_count, 0);
    timeout(DEADLINE, async {
        while let Some(chunk) = stream.next().await {
            chunk.unwrap();
        }
    })
    .await
    .unwrap();
    drop(stream);
    failed.finish().await;
    success.finish().await;
    fixture.finish().await;
}
