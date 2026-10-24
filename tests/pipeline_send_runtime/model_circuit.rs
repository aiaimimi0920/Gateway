//! Real HTTP failures must stop future sends, independently of advisory routing scores.
use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY,
};
use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::stage_send;
use neuro_gateway::pipeline::{stage_route, PipelineOutput};
use neuro_gateway::protocol::canonical::CanonicalTool;
use tokio::time::timeout;

// Keep debug-only constructors for the large route/send futures off the test's own stack frame.
#[inline(never)]
fn send<'a>(
    ctx: &'a mut neuro_gateway::pipeline::PipelineContext,
    state: &'a std::sync::Arc<neuro_gateway::state::AppState>,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<PipelineOutput, neuro_gateway::error::GatewayError>>
            + 'a,
    >,
> {
    Box::pin(stage_send::run(ctx, state))
}
#[inline(never)]
fn route<'a>(
    ctx: &'a mut neuro_gateway::pipeline::PipelineContext,
    state: &'a std::sync::Arc<neuro_gateway::state::AppState>,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(), neuro_gateway::error::GatewayError>> + 'a>,
> {
    Box::pin(stage_route::run(ctx, state))
}

#[tokio::test]
async fn third_html_failure_hard_blocks_the_fourth_send_and_keeps_fallback() {
    let rejected = Upstream::start(
        StatusCode::FORBIDDEN,
        false,
        "<html><title>系统终端 - 异常状态诊断</title></html>",
    )
    .await;
    let healthy = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let failed_candidate = candidate(&rejected, "rejected");
    for _ in 0..3 {
        let mut ctx = context(false, vec![failed_candidate.clone()]);
        ctx.canonical_req.tools.push(CanonicalTool {
            tool_type: "function".into(),
            name: Some("fixture".into()),
            description: None,
            input_schema: None,
            raw: Default::default(),
        });
        assert!(timeout(DEADLINE, send(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .is_err());
    }
    assert_eq!(rejected.requests().len(), 3);
    let mut ctx = context(
        false,
        vec![failed_candidate, candidate(&healthy, "healthy")],
    );
    authorize_candidates(&mut ctx);
    assert!(timeout(DEADLINE, send(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .is_ok());
    assert_eq!(
        rejected.requests().len(),
        3,
        "open circuit must not send even when a candidate was cached"
    );
    assert_eq!(healthy.requests().len(), 1);
    assert_eq!(
        ctx.route_attempt_count(),
        1,
        "hard rejection does not consume an outbound slot"
    );
    rejected.finish().await;
    healthy.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn pool_filters_failed_plus_before_round_robin_without_disabling_pro() {
    let rejected = Upstream::start(
        StatusCode::FORBIDDEN,
        false,
        "<html><title>系统终端 - 异常状态诊断</title></html>",
    )
    .await;
    let healthy = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    fixture.state.route_config.replace_document(serde_json::from_value(serde_json::json!({
        "providers":[{"id":"site","preset":"openai","base_url":rejected.base_url,
            "supported_models":["caller-alias"], "model_map":{"caller-alias":"upstream-model"},
            "credentials":[{"id":"plus-token","api_key":"send-test-token"},
                {"id":"pro-token","base_url":healthy.base_url,"api_key":"send-test-token"}]}],
        "account_groups":[{"id":"plus","name":"Plus","provider_credential_ids":["plus-token"]},
            {"id":"pro","name":"Pro","provider_credential_ids":["pro-token"]}]
    })).unwrap()).unwrap();
    let plus = fixture
        .state
        .route_config
        .snapshot()
        .resolve_candidates_for_account_group(Some("caller-alias"), Some("plus"))
        .unwrap()
        .remove(0);
    for _ in 0..3 {
        let mut ctx = context(false, vec![plus.clone()]);
        assert!(timeout(DEADLINE, send(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .is_err());
    }
    for _ in 0..2 {
        let mut ctx = context(false, Vec::new());
        route(&mut ctx, &fixture.state).await.unwrap();
        assert_eq!(ctx.candidates.len(), 1);
        assert_eq!(
            ctx.candidates[0].payload.credential_id.as_deref(),
            Some("pro-token")
        );
    }
    let mut pro = context(false, Vec::new());
    pro.account_group_id = Some("pro".into());
    route(&mut pro, &fixture.state).await.unwrap();
    assert!(send(&mut pro, &fixture.state).await.is_ok());
    let mut plus_only = context(false, Vec::new());
    plus_only.account_group_id = Some("plus".into());
    assert!(route(&mut plus_only, &fixture.state).await.is_err());
    assert_eq!(rejected.requests().len(), 3);
    assert_eq!(healthy.requests().len(), 1);
    rejected.finish().await;
    healthy.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn stream_transport_failures_after_handoff_are_not_header_successes() {
    const PARTIAL: &str = "data: {\"id\":\"s\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hello\"},\"finish_reason\":null}]}\n\n";
    let fixture = TestState::new();
    let mut last = None;
    for _ in 0..3 {
        let broken = Upstream::start_broken_stream(PARTIAL).await;
        let mut selected = candidate(&broken, "broken");
        selected.provider_account_id = "broken-site".into();
        let mut ctx = context(true, vec![selected.clone()]);
        let output = timeout(DEADLINE, send(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap();
        let PipelineOutput::Sse(mut stream) = output else {
            panic!("expected stream")
        };
        assert!(stream.next().await.is_some());
        broken.release_stream();
        let _ = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
        assert_eq!(broken.requests().len(), 1);
        last = Some(selected);
        broken.finish().await;
    }
    let mut ctx = context(true, vec![last.unwrap()]);
    assert!(timeout(DEADLINE, send(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .is_err());
    assert_eq!(
        ctx.route_attempt_count(),
        0,
        "stream header success must not reset the failure streak"
    );
    fixture.finish().await;
}
