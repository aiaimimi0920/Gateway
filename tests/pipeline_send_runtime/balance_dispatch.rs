//! Real loopback refusal: a balance failure is account-wide, and a funded sibling remains usable.
use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY,
};
use axum::http::StatusCode;
use neuro_gateway::pipeline::{stage_route, stage_send, PipelineContext, PipelineOutput};
use neuro_gateway::protocol::canonical::CanonicalTool;
use neuro_gateway::{error::GatewayError, state::AppState};
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::time::timeout;

#[inline(never)]
fn send<'a>(
    ctx: &'a mut PipelineContext,
    state: &'a Arc<AppState>,
) -> Pin<Box<dyn Future<Output = Result<PipelineOutput, GatewayError>> + 'a>> {
    Box::pin(stage_send::run(ctx, state))
}
#[inline(never)]
fn route<'a>(
    ctx: &'a mut PipelineContext,
    state: &'a Arc<AppState>,
) -> Pin<Box<dyn Future<Output = Result<(), GatewayError>> + 'a>> {
    Box::pin(stage_route::run(ctx, state))
}
const BALANCE: &str = r#"{"error":{"message":"预扣费额度失败, 用户剩余额度: BTC10.895248, 需要预扣费额度: BTC32.560000 (request id: key-fixture)"}}"#;

#[tokio::test]
async fn balance_rejection_switches_sibling_in_same_request_and_excludes_it_across_models() {
    let rejected = Upstream::start(StatusCode::FORBIDDEN, false, BALANCE).await;
    let healthy = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::with_local_limits().await;
    fixture.state.route_config.replace_document(serde_json::from_value(serde_json::json!({
        "providers":[{"id":"site","preset":"openai","base_url":rejected.base_url,
            "supported_models":["caller-alias","other"],"model_map":{"caller-alias":"upstream-model"},
            "credentials":[{"id":"empty","api_key":"send-test-token"},
                {"id":"funded","base_url":healthy.base_url,"api_key":"send-test-token"}]}],
        "account_groups":[{"id":"shared","name":"Shared","provider_credential_ids":["empty","funded"]}]
    })).unwrap()).unwrap();
    let mut ctx = context(false, vec![]);
    ctx.account_group_id = Some("shared".into());
    route(&mut ctx, &fixture.state).await.unwrap();
    assert_eq!(
        ctx.candidates.len(),
        2,
        "both eligible credentials must enter the fallback queue"
    );
    ctx.candidates
        .sort_by_key(|c| c.payload.credential_id.as_deref() != Some("empty"));
    let stale = ctx.candidates.clone();
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
        .is_ok());
    assert_eq!(ctx.route_attempt_count(), 2);
    assert_eq!(
        (rejected.requests().len(), healthy.requests().len()),
        (1, 1)
    );
    for model in ["caller-alias", "other"] {
        let mut next = context(false, vec![]);
        next.account_group_id = Some("shared".into());
        next.canonical_req.requested_model = Some(model.into());
        route(&mut next, &fixture.state).await.unwrap();
        assert_eq!(next.candidates.len(), 1);
        assert_eq!(
            next.candidates[0].payload.credential_id.as_deref(),
            Some("funded")
        );
    }
    let mut cached = context(false, stale);
    assert!(send(&mut cached, &fixture.state).await.is_ok());
    assert_eq!(cached.route_attempt_count(), 1);
    assert_eq!(
        rejected.requests().len(),
        1,
        "stale queues must not bypass balance cooling"
    );
    rejected.finish().await;
    healthy.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn balance_rejection_falls_back_to_authorized_provider_without_repeated_send() {
    let rejected = Upstream::start(StatusCode::FORBIDDEN, false, BALANCE).await;
    let healthy = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let first = candidate(&rejected, "empty");
    for _ in 0..2 {
        let mut ctx = context(false, vec![first.clone(), candidate(&healthy, "funded")]);
        authorize_candidates(&mut ctx);
        assert!(send(&mut ctx, &fixture.state).await.is_ok());
    }
    assert_eq!(
        (rejected.requests().len(), healthy.requests().len()),
        (1, 2)
    );
    rejected.finish().await;
    healthy.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn preoutput_stream_balance_refusal_allows_tool_fallback_but_output_never_replays() {
    use futures::StreamExt;
    for started in [false, true] {
        let body = if started {
            "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\nevent: error\ndata: {\"error\":{\"message\":\"预扣费额度失败, 用户剩余额度: BTC1, 需要预扣费额度: BTC32\"}}\n\n"
        } else {
            "event: error\ndata: {\"error\":{\"message\":\"预扣费额度失败, 用户剩余额度: BTC1, 需要预扣费额度: BTC32\"}}\n\n"
        };
        let rejected = Upstream::start(StatusCode::OK, true, body).await;
        let healthy = Upstream::start(StatusCode::OK, true, super::fixture::SSE_REPLY).await;
        let fixture = TestState::new();
        let mut ctx = context(
            true,
            vec![candidate(&rejected, "empty"), candidate(&healthy, "funded")],
        );
        authorize_candidates(&mut ctx);
        ctx.canonical_req.tools.push(CanonicalTool {
            tool_type: "function".into(),
            name: Some("fixture".into()),
            description: None,
            input_schema: None,
            raw: Default::default(),
        });
        let PipelineOutput::Sse(stream) = send(&mut ctx, &fixture.state).await.unwrap() else {
            panic!("expected stream")
        };
        let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
        if !started {
            assert!(chunks.iter().all(Result::is_ok));
        }
        assert_eq!(healthy.requests().len(), usize::from(!started));
        assert_eq!(rejected.requests().len(), 1);
        rejected.finish().await;
        healthy.finish().await;
        fixture.finish().await;
    }
}
