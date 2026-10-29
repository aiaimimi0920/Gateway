//! A temporary circuit may be retained for waiting, but never bypasses real send admission.
use super::fixture::{candidate, context, TestState, Upstream};
use axum::http::StatusCode;
use neuro_gateway::pipeline::{stage_route, stage_send};
use serde_json::json;

#[tokio::test]
async fn recovery_keeps_an_authorized_open_circuit_without_sending_before_recovery() {
    let failed = Upstream::start(
        StatusCode::FORBIDDEN,
        false,
        "<html><title>系统终端 - 异常状态诊断</title></html>",
    )
    .await;
    let fixture = TestState::new();
    let mut doc = json!({"providers":[{"id":"site","preset":"openai","base_url":failed.base_url,
        "api_key":"send-test-token", "supported_models":["caller-alias"],
        "model_map":{"caller-alias":"upstream-model"}, "request_wait":{"mode":"disabled"}}]});
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(doc.clone()).unwrap())
        .unwrap();
    for _ in 0..3 {
        let mut ctx = context(false, vec![]);
        Box::pin(stage_route::run(&mut ctx, &fixture.state))
            .await
            .unwrap();
        assert!(Box::pin(stage_send::run(&mut ctx, &fixture.state))
            .await
            .is_err());
    }
    doc["providers"][0]["request_wait"] = json!({"mode":"bounded","total_timeout_seconds":1});
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(doc).unwrap())
        .unwrap();
    let mut ctx = context(false, vec![]);
    Box::pin(stage_route::run(&mut ctx, &fixture.state))
        .await
        .unwrap();
    assert_eq!(
        ctx.candidates.len(),
        1,
        "temporary circuit is not a zero-candidate 503"
    );
    let error = Box::pin(stage_send::run(&mut ctx, &fixture.state))
        .await
        .err()
        .unwrap();
    assert_eq!(error.request_budget_stop_reason(), Some("deadline"));
    assert_eq!(failed.requests().len(), 3);
    assert_eq!(ctx.route_attempt_count(), 0);
    failed.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn recovery_mode_never_replays_after_partial_stream_handoff() {
    use futures::StreamExt;
    use neuro_gateway::pipeline::PipelineOutput;
    const PARTIAL: &str = "data: {\"id\":\"s\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n";
    let upstream = Upstream::start_broken_stream(PARTIAL).await;
    let selected = candidate(&upstream, "managed-partial");
    let fixture = TestState::new();
    fixture.state.route_config.replace_document(serde_json::from_value(json!({"providers":[{
        "id":selected.provider_account_id,"preset":"openai","base_url":upstream.base_url,"api_key":"send-test-token"
    }]})).unwrap()).unwrap();
    let mut ctx = context(true, vec![selected]);
    let output = Box::pin(stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap();
    let PipelineOutput::Sse(mut stream) = output else {
        panic!("expected SSE")
    };
    assert!(stream.next().await.unwrap().is_ok());
    upstream.release_stream();
    let _ = stream.collect::<Vec<_>>().await;
    assert_eq!(ctx.route_attempt_count(), 1);
    assert_eq!(upstream.requests().len(), 1);
    upstream.finish().await;
    fixture.finish().await;
}
