use std::sync::Arc;

use axum::http::StatusCode;
use neuro_gateway::pipeline::{stage_route, stage_send, PipelineOutput};
use neuro_gateway::routing::config::RouteConfigStore;
use serde_json::json;
use tokio::time::timeout;

use super::fixture::{context, TestState, Upstream, DEADLINE, JSON_REPLY};

#[tokio::test]
async fn successful_buffered_sends_teach_the_real_route_stage() {
    let slow = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fast = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let mut fixture = TestState::with_local_limits().await;
    let document = serde_json::from_value(json!({
        "providers": [
            {"id":"slow", "preset":"openai", "base_url":slow.base_url, "api_key":"send-test-token"},
            {"id":"fast", "preset":"openai", "base_url":fast.base_url, "api_key":"send-test-token"}
        ],
        "model_routes":[{"pattern":"caller-alias", "provider_ids":["slow", "fast"], "priority":10}]
    }))
    .unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().route_config =
        Arc::new(RouteConfigStore::from_document(document).unwrap());
    let mut initial = context(false, vec![]);
    stage_route::run(&mut initial, &fixture.state)
        .await
        .unwrap();
    assert_eq!(initial.candidates.len(), 2);
    assert_eq!(initial.candidates[0].provider_account_id, "slow");
    let mut fast_ctx = context(false, vec![initial.candidates[1].clone()]);
    let output = timeout(DEADLINE, stage_send::run(&mut fast_ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(output, PipelineOutput::Json(_)));
    assert!(slow.requests().is_empty());
    let mut routed = context(false, vec![]);
    stage_route::run(&mut routed, &fixture.state).await.unwrap();
    let first = routed.candidates[0].provider_account_id.clone();
    slow.finish().await;
    fast.finish().await;
    fixture.finish().await;
    assert_eq!(
        first, "fast",
        "a successful fast sample should outrank an unmeasured equal-tier candidate"
    );
}
