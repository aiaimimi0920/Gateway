//! Diagnostic HTML refusals consume bounded fallback slots, not same-provider retries.
use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY, SSE_REPLY,
};
use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_route, stage_send, PipelineContext, PipelineOutput};
use neuro_gateway::protocol::canonical::{CanonicalTool, EndpointKind};
use tokio::time::timeout;

const PAGE: &str =
    "<!DOCTYPE html><html><head><title>系统终端 - 异常状态诊断</title></head></html>";
const RESPONSES_SSE: &str = "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[]}}\n\n";

fn with_tools(ctx: &mut PipelineContext) {
    ctx.canonical_req.tools.push(CanonicalTool {
        tool_type: "function".into(),
        name: Some("fixture_tool".into()),
        description: None,
        input_schema: None,
        raw: Default::default(),
    });
}

#[tokio::test]
async fn html_403_tool_conversation_falls_back_in_both_protocols_and_modes() {
    for endpoint in [EndpointKind::ChatCompletions, EndpointKind::Responses] {
        for streaming in [true, false] {
            let rejected = Upstream::start(StatusCode::FORBIDDEN, false, PAGE).await;
            let reply = match (&endpoint, streaming) {
                // Responses uses a forced upstream SSE wire even for buffered callers.
                (EndpointKind::Responses, _) => RESPONSES_SSE,
                (_, true) => SSE_REPLY,
                (_, false) => JSON_REPLY,
            };
            let next = Upstream::start(
                StatusCode::OK,
                streaming || endpoint == EndpointKind::Responses,
                reply,
            )
            .await;
            let fixture = TestState::new();
            let mut ctx = context(
                streaming,
                vec![candidate(&rejected, "rejected"), candidate(&next, "next")],
            );
            ctx.canonical_req.endpoint_kind = endpoint.clone();
            if endpoint == EndpointKind::Responses {
                for candidate in &mut ctx.candidates {
                    candidate.payload.responses_path = Some("/v1/responses".into());
                    candidate.payload.chat_completions_path = None;
                }
            }
            with_tools(&mut ctx);
            authorize_candidates(&mut ctx);
            let result = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
                .await
                .unwrap();
            assert!(
                result.is_ok(),
                "HTML 403 must advance with tools: {:?}",
                result.as_ref().err()
            );
            let output = result.unwrap();
            if let PipelineOutput::Sse(stream) = output {
                let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
                assert!(chunks.iter().all(Result::is_ok));
            } else {
                assert!(!streaming, "streaming fallback must return SSE");
            }
            assert_eq!((rejected.requests().len(), next.requests().len()), (1, 1));
            assert_eq!(ctx.route_attempt_count(), 2);
            rejected.finish().await;
            next.finish().await;
            fixture.finish().await;
        }
    }
}

#[tokio::test]
async fn html_403_does_not_bypass_cross_provider_authorization() {
    let rejected = Upstream::start(StatusCode::FORBIDDEN, false, PAGE).await;
    let next = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(
        false,
        vec![candidate(&rejected, "first"), candidate(&next, "next")],
    );
    with_tools(&mut ctx);
    let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_fallback_not_authorized")
    );
    assert_eq!(rejected.requests().len(), 1);
    assert!(next.requests().is_empty());
    rejected.finish().await;
    next.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn consecutive_html_403_refusals_share_six_attempt_limit() {
    let rejected = Upstream::start(StatusCode::FORBIDDEN, false, PAGE).await;
    let next = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let mut candidates: Vec<_> = (0..6)
        .map(|index| candidate(&rejected, &format!("rejected-{index}")))
        .collect();
    candidates.push(candidate(&next, "beyond-budget"));
    let mut ctx = context(false, candidates);
    with_tools(&mut ctx);
    authorize_candidates(&mut ctx);
    let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(error.request_budget_stop_reason(), Some("attempt_limit"));
    assert_eq!(error.code.as_deref(), Some("upstream_html_diagnostic"));
    assert_eq!(ctx.route_attempt_count(), 6);
    assert_eq!(rejected.requests().len(), 6);
    assert!(next.requests().is_empty());
    rejected.finish().await;
    next.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn html_403_uses_smart_sorted_authorized_fallback_queue() {
    let rejected = Upstream::start(StatusCode::FORBIDDEN, false, PAGE).await;
    let lower_score = Upstream::start(StatusCode::FORBIDDEN, false, PAGE).await;
    let healthy = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    fixture.state.route_config.replace_document(serde_json::from_value(serde_json::json!({
        "providers":[
            {"id":"first","preset":"openai","base_url":rejected.base_url,"api_key":"send-test-token","supported_models":["caller-alias"]},
            {"id":"lower-score","preset":"openai","base_url":lower_score.base_url,"api_key":"send-test-token","supported_models":["caller-alias"]},
            {"id":"healthy","preset":"openai","base_url":healthy.base_url,"api_key":"send-test-token","supported_models":["caller-alias"]}
        ],
        "model_routes":[
            {"pattern":"caller-alias","provider_ids":["first"],"priority":100},
            {"pattern":"caller-alias","provider_ids":["lower-score"],"priority":90},
            {"pattern":"caller-alias","provider_ids":["healthy"],"priority":10}
        ]
    })).unwrap()).unwrap();
    // Feed two real failures through the send stage: 100 -> 85 -> 70 (lower tier).
    for _ in 0..2 {
        let mut failed = candidate(&lower_score, "lower-score");
        failed.provider_account_id = "lower-score".into();
        let mut probe = context(false, vec![failed]);
        with_tools(&mut probe);
        assert!(stage_send::run(&mut probe, &fixture.state).await.is_err());
    }
    let mut ctx = context(false, Vec::new());
    with_tools(&mut ctx);
    ctx.request_headers.insert(
        "x-gateway-routing-order".into(),
        "availability,intelligence,price".into(),
    );
    // Keep the large routing future off this multi-step debug test's state frame.
    Box::pin(stage_route::run(&mut ctx, &fixture.state))
        .await
        .unwrap();
    assert_eq!(
        ctx.candidates
            .iter()
            .map(|c| c.provider_account_id.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "healthy", "lower-score"]
    );
    assert!(timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .is_ok());
    assert_eq!(rejected.requests().len(), 1);
    assert_eq!(healthy.requests().len(), 1);
    assert_eq!(
        lower_score.requests().len(),
        2,
        "no additional call to lower tier"
    );
    rejected.finish().await;
    lower_score.finish().await;
    healthy.finish().await;
    fixture.finish().await;
}
