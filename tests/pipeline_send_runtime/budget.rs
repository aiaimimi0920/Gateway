//! Request-wide limits must constrain the real transport, not candidate iteration.
use axum::http::StatusCode;
use neuro_gateway::pipeline::stage_send;
use tokio::time::timeout;

use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY,
};

#[tokio::test]
async fn http_callers_hold_only_a_boxed_pipeline_future() {
    let fixture = TestState::new();
    let future = neuro_gateway::pipeline::run_pipeline(context(false, Vec::new()), &fixture.state);
    let caller_frame = std::mem::size_of_val(&future);
    // HTTP handlers must not inline the pipeline's large dispatch state machine.
    assert!(
        caller_frame <= std::mem::size_of::<usize>(),
        "pipeline caller frame is {caller_frame} bytes instead of a boxed future"
    );
    drop(future);
    fixture.finish().await;
}

#[tokio::test]
async fn logical_request_default_budget_prevents_candidate_multiplication() {
    let body = r#"{"error":{"message":"fixture unavailable"}}"#;
    let first = Upstream::start(StatusCode::SERVICE_UNAVAILABLE, false, body).await;
    let second = Upstream::start(StatusCode::SERVICE_UNAVAILABLE, false, body).await;
    let untouched = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(
        false,
        vec![
            candidate(&first, "budget-first"),
            candidate(&second, "budget-second"),
            candidate(&untouched, "budget-untouched"),
        ],
    );
    authorize_candidates(&mut ctx);
    let result = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap();
    let counts = (
        first.requests().len(),
        second.requests().len(),
        untouched.requests().len(),
    );
    let attempts = ctx.route_attempt_count();
    first.finish().await;
    second.finish().await;
    untouched.finish().await;
    fixture.finish().await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("request budget must stop before the third provider"),
    };
    assert_eq!(counts, (3, 3, 0));
    assert_eq!(attempts, 6);
    assert_eq!(error.request_budget_stop_reason(), Some("attempt_limit"));
    assert!(!error.retryable);
    assert!(error.message.contains("fixture unavailable"));
}
