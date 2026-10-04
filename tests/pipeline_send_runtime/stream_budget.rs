//! Once stream ownership is returned, timeout/error/drop cannot replay a supplier.
use std::time::Duration;

use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{request_budget::RequestBudget, stage_send, PipelineOutput};
use tokio::time::timeout;

use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, SSE_REPLY,
};

const FIRST_CHUNK: &str = "data: {\"id\":\"fixture\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"unique-text\",\"tool_calls\":[{\"index\":0,\"id\":\"unique-tool\",\"type\":\"function\",\"function\":{\"name\":\"fixture_tool\",\"arguments\":\"{}\"}}]},\"finish_reason\":null}]}\n\n";

#[tokio::test]
async fn post_handoff_deadline_has_no_duplicate_text_tools_or_provider_penalty() {
    let upstream = Upstream::start_held_stream(FIRST_CHUNK).await;
    let untouched = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "post-handoff");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = context(
        true,
        vec![selected, candidate(&untouched, "no-post-replay")],
    );
    authorize_candidates(&mut ctx);
    ctx.request_budget = RequestBudget::new(6, Duration::from_millis(250));
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(mut stream) = output else {
        panic!("expected SSE")
    };
    let first = timeout(DEADLINE, stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(first.to_vec()).unwrap();
    assert_eq!(text.matches("unique-text").count(), 1);
    assert_eq!(text.matches("unique-tool").count(), 1);
    let error = timeout(DEADLINE, stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(error.to_string().contains("budget_exhausted:deadline"));
    assert!(stream.next().await.is_none());
    assert_eq!(controller.snapshot().current_limit, 10);
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    assert_eq!(ctx.route_attempt_count(), 1);
    assert_eq!(upstream.requests().len(), 1);
    assert!(untouched.requests().is_empty());
    assert_eq!(
        stage_send::run(&mut ctx, &fixture.state)
            .await
            .err()
            .unwrap()
            .request_budget_stop_reason(),
        Some("deadline")
    );
    assert_eq!(upstream.requests().len(), 1);
    drop(stream);
    upstream.finish().await;
    untouched.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn post_handoff_drop_does_not_replay_on_an_authorized_second_account() {
    let upstream = Upstream::start_held_stream(FIRST_CHUNK).await;
    let untouched = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(
        true,
        vec![
            candidate(&upstream, "drop-handoff"),
            candidate(&untouched, "no-drop-replay"),
        ],
    );
    authorize_candidates(&mut ctx);
    let PipelineOutput::Sse(mut stream) =
        timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("expected SSE")
    };
    assert!(stream.next().await.unwrap().is_ok());
    drop(stream);
    assert_eq!(ctx.request_budget.stopped_reason(), Some("cancelled"));
    assert!(stage_send::run(&mut ctx, &fixture.state).await.is_err());
    assert_eq!(upstream.requests().len(), 1);
    assert!(untouched.requests().is_empty());
    upstream.finish().await;
    untouched.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn post_handoff_transport_failure_never_replays_text_or_tools() {
    let upstream = Upstream::start_broken_stream(FIRST_CHUNK).await;
    let untouched = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "transport-handoff");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = context(
        true,
        vec![selected, candidate(&untouched, "no-error-replay")],
    );
    authorize_candidates(&mut ctx);
    let PipelineOutput::Sse(mut stream) =
        timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("expected SSE")
    };
    let first = timeout(DEADLINE, stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(first.to_vec()).unwrap();
    assert_eq!(text.matches("unique-text").count(), 1);
    assert_eq!(text.matches("unique-tool").count(), 1);
    upstream.release_stream();
    assert!(timeout(DEADLINE, stream.next())
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(stream.next().await.is_none());
    assert_eq!(controller.snapshot().current_limit, 7);
    assert_eq!(upstream.requests().len(), 1);
    assert!(untouched.requests().is_empty());
    assert!(stage_send::run(&mut ctx, &fixture.state).await.is_err());
    drop(stream);
    upstream.finish().await;
    untouched.finish().await;
    fixture.finish().await;
}
