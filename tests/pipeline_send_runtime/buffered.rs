use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::http::StatusCode;
use neuro_gateway::pipeline::{stage_send, PipelineOutput};
use tokio::time::timeout;

use super::fixture::{candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY};

#[tokio::test]
async fn buffered_success_preserves_wire_model_usage_and_releases_permit() {
    let upstream = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "success");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = context(false, vec![selected.clone()]);
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Json(reply) = output else {
        panic!("expected JSON response")
    };
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "local upstream reply"
    );
    assert_eq!(reply["model"], "caller-alias");
    assert_eq!(reply["usage"]["total_tokens"], 10);
    assert_eq!(
        ctx.selected_provider_id.as_deref(),
        Some(selected.provider_account_id.as_str())
    );
    assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 1);
    let requests = upstream.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/chat/completions");
    assert!(requests[0].fixture_auth);
    assert_eq!(requests[0].body["model"], "upstream-model");
    assert_eq!(requests[0].body["messages"][0]["content"], "local request");
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(controller.snapshot().consecutive_successes, 1);
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn transient_failure_retries_then_falls_back_to_next_provider() {
    let failed = Upstream::start(
        StatusCode::SERVICE_UNAVAILABLE,
        false,
        r#"{"error":{"message":"fixture unavailable"}}"#,
    )
    .await;
    let success = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let first = candidate(&failed, "failed");
    let second = candidate(&success, "fallback");
    let first_ctrl = fixture
        .state
        .concurrency_registry
        .get_or_create(&first.provider_account_id);
    let second_ctrl = fixture
        .state
        .concurrency_registry
        .get_or_create(&second.provider_account_id);
    let mut ctx = context(false, vec![first.clone(), second.clone()]);
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Json(reply) = output else {
        panic!("expected fallback JSON")
    };
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "local upstream reply"
    );
    assert_eq!(failed.requests().len(), 3);
    assert_eq!(success.requests().len(), 1);
    assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 4);
    assert_eq!(
        *ctx.attempted_provider_ids.lock(),
        vec![
            first.provider_account_id,
            second.provider_account_id.clone()
        ]
    );
    assert_eq!(ctx.selected_provider_id, Some(second.provider_account_id));
    assert_eq!(first_ctrl.snapshot().current_limit, 7);
    assert_eq!(first_ctrl.snapshot().active_count, 0);
    assert_eq!(second_ctrl.snapshot().consecutive_successes, 1);
    assert_eq!(second_ctrl.snapshot().active_count, 0);
    failed.finish().await;
    success.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn permanent_client_error_does_not_retry_or_contact_next_candidate() {
    let failed = Upstream::start(
        StatusCode::BAD_REQUEST,
        false,
        r#"{"error":{"message":"invalid fixture input"}}"#,
    )
    .await;
    let untouched = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let first = candidate(&failed, "bad-request");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&first.provider_account_id);
    let mut ctx = context(false, vec![first, candidate(&untouched, "untouched")]);
    let result = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap();
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("expected terminal client error"),
    };
    assert_eq!(error.http_status, Some(400));
    assert_eq!(failed.requests().len(), 1);
    assert!(untouched.requests().is_empty());
    assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 1);
    assert_eq!(controller.snapshot().active_count, 0);
    failed.finish().await;
    untouched.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn cancelling_while_queued_consumes_no_admission_or_outbound_attempt() {
    let upstream = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "queued");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut permits = Vec::new();
    for _ in 0..10 {
        permits.push(controller.acquire().await);
    }
    let mut ctx = context(false, vec![selected]);
    {
        let future = stage_send::run(&mut ctx, &fixture.state);
        tokio::pin!(future);
        assert!(timeout(Duration::from_millis(50), &mut future)
            .await
            .is_err());
    }
    assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 0);
    assert!(ctx.attempted_provider_ids.lock().is_empty());
    assert!(upstream.requests().is_empty());
    assert_eq!(controller.snapshot().active_count, 10);
    drop(permits);
    assert_eq!(controller.snapshot().active_count, 0);
    upstream.finish().await;
    fixture.finish().await;
}
